//! Route kiểm tra và cập nhật mod (spec P3, mục 5.8; kế hoạch PR-32).

use axum::http::StatusCode;
use serde_json::json;

use super::support::{content_api, create_with_loader, wait_job};

#[tokio::test]
async fn check_runs_as_a_job_with_a_report() {
    let api = content_api(None);
    let fabric = create_with_loader(&api, "Fabric", "fabric", 25565).await;
    let (status, started) = api
        .post(&format!("/api/v1/servers/{fabric}/updates/check"), None)
        .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{started}");
    let job = started["job_id"].as_u64().unwrap();
    wait_job(&api, job).await;
    let (_, snap) = api.get(&format!("/api/v1/jobs/{job}")).await;
    assert_eq!(snap["kind"], "check_updates");
    assert_eq!(snap["result"]["updates"], json!([]), "{snap}");
}

#[tokio::test]
async fn update_body_is_validated() {
    let api = content_api(None);
    let fabric = create_with_loader(&api, "Fabric", "fabric", 25565).await;
    let vanilla = create_with_loader(&api, "Vanilla", "vanilla", 25566).await;
    let uri = format!("/api/v1/servers/{fabric}/updates");
    let item = |source: &str, file: &str| json!({ "file": file, "source": source, "project_id": "P", "version_id": "v2" });
    for (body, field) in [
        (json!({ "items": [] }), "items"),
        (json!({ "items": [item("hangar", "mods/a.jar")] }), "source"),
        (json!({ "items": [item("modrinth", " ")] }), "file"),
    ] {
        let (status, err) = api.post(&uri, Some(body)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{err}");
        assert_eq!(err["params"]["field"], field, "{err}");
    }
    let (status, err) = api
        .post(&format!("/api/v1/servers/{vanilla}/updates/check"), None)
        .await;
    assert_eq!(
        (status, err["code"].as_str()),
        (StatusCode::CONFLICT, Some("CONTENT_INCOMPATIBLE"))
    );
}

#[tokio::test]
async fn version_change_needs_confirm_and_valid_versions() {
    let api = content_api(None);
    let paper = create_with_loader(&api, "Paper", "paper", 25565).await;
    let uri = format!("/api/v1/servers/{paper}/version");
    for (body, field) in [
        (json!({ "minecraft": "1.21.1" }), "confirm"),
        (
            json!({ "minecraft": "../1.21", "confirm": true }),
            "minecraft",
        ),
        (
            json!({ "loader_version": "0.16.9", "confirm": true }),
            "loader_version",
        ),
    ] {
        let (status, err) = api.post(&uri, Some(body)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{err}");
        assert_eq!(err["params"]["field"], field, "{err}");
    }
    let (status, started) = api.post(&uri, Some(json!({ "confirm": true }))).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{started}");
    let job = started["job_id"].as_u64().unwrap();
    wait_job(&api, job).await;
    let (_, snap) = api.get(&format!("/api/v1/jobs/{job}")).await;
    assert_eq!(snap["kind"], "change_version");
    assert_eq!(snap["result"]["changed"], false, "{snap}");
}
