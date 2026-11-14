//! Kiểm thử route tạo server từ modpack (`/servers/modpack` và `/servers/modpack/upload`).

#[path = "tests_modpack_support.rs"]
mod support;

use axum::body::Body;
use axum::http::{Method, StatusCode, header};
use mh_app::ModpackSummary;
use serde_json::json;

use super::{HOST, ORIGIN, raw, send};
use support::{modpack_api, sample_mrpack, upload_pack, wait_job_json};

#[tokio::test]
async fn post_servers_modpack_creates_job_and_returns_summary() {
    let api = modpack_api();
    let (status, vers) = api
        .get("/api/v1/content/modrinth/projects/pack1/versions")
        .await;
    assert_eq!(status, StatusCode::OK, "{vers}");
    assert_eq!(vers[0]["id"], "vp1");

    let (status, created) = api
        .post(
            "/api/v1/servers/modpack",
            Some(json!({ "source": "modrinth", "project_id": "pack1", "name": "Pack API", "port": 25565 })),
        )
        .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{created}");
    let job = wait_job_json(&api, created["job_id"].as_u64().unwrap()).await;
    assert_eq!(job["kind"], "create_modpack");
    let summary: ModpackSummary = serde_json::from_value(job["result"].clone()).unwrap();
    assert_eq!(summary.installed_mods, 1);
    assert_eq!(summary.skipped_client, ["mods/cli.jar"]);
    assert!(summary.pending_curseforge.is_empty());

    let sid = created["server_id"].as_str().unwrap();
    let (status, srv) = api.get(&format!("/api/v1/servers/{sid}")).await;
    assert_eq!(status, StatusCode::OK, "{srv}");
    assert_eq!(srv["loader"], "fabric");
}

#[tokio::test]
async fn post_servers_modpack_validates_inputs() {
    let api = modpack_api();
    let (st, err) = api
        .post(
            "/api/v1/servers/modpack",
            Some(json!({ "source": "curseforge", "project_id": "x", "name": "CF" })),
        )
        .await;
    assert_eq!(
        (st, err["code"].as_str()),
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Some("CURSEFORGE_UNAVAILABLE")
        )
    );

    for body in [
        json!({ "source": "other", "project_id": "x", "name": "Bad" }),
        json!({ "source": "modrinth", "project_id": "   ", "name": "Bad" }),
    ] {
        let (st, err) = api.post("/api/v1/servers/modpack", Some(body)).await;
        assert_eq!(
            (st, err["code"].as_str()),
            (StatusCode::BAD_REQUEST, Some("INVALID_INPUT"))
        );
    }
}

#[tokio::test]
async fn post_servers_modpack_upload_mrpack_and_reject_zip() {
    let api = modpack_api();
    let mrpack = sample_mrpack(
        "Upload API",
        "https://cdn.modrinth.com/data/fab/versions/vf1/fabric-api.jar",
        38,
    );
    let (st, created) = upload_pack(
        &api.router,
        "pack.mrpack",
        &mrpack,
        &[
            ("name", "Upload API"),
            ("memory_mb", r#"{"min":2048,"max":6144}"#),
            ("port", "25566"),
        ],
    )
    .await;
    assert_eq!(st, StatusCode::ACCEPTED, "{created}");
    let job = wait_job_json(&api, created["job_id"].as_u64().unwrap()).await;
    assert_eq!(job["result"]["installed_mods"], 1);
    let (_, srv) = api
        .get(&format!(
            "/api/v1/servers/{}",
            created["server_id"].as_str().unwrap()
        ))
        .await;
    assert_eq!(srv["port"], 25566);

    let (st, err) = upload_pack(&api.router, "curse.zip", b"zipbytes", &[("name", "CF Zip")]).await;
    assert_eq!(
        (st, err["code"].as_str()),
        (StatusCode::UNPROCESSABLE_ENTITY, Some("MODPACK_INVALID"))
    );
    assert!(
        err["params"]["reason"]
            .as_str()
            .unwrap()
            .contains(".mrpack")
    );

    let (st, err) = upload_pack(&api.router, "pack.mrpack", &mrpack, &[]).await;
    assert_eq!(
        (st, err["code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("INVALID_INPUT"))
    );
}

#[tokio::test]
async fn modpack_upload_guard_and_openapi_routes() {
    let api = modpack_api();
    for bad_path in [
        "/api/v1/servers/modpack",
        "/api/v1/servers/modpack/upload/extra",
    ] {
        let req = raw(Method::POST, bad_path)
            .header(header::HOST, HOST)
            .header(header::ORIGIN, ORIGIN)
            .header(header::CONTENT_TYPE, "multipart/form-data; boundary=b")
            .header(header::CONTENT_LENGTH, "1")
            .body(Body::from("x"))
            .unwrap();
        let (st, err) = send(api.router.clone(), req).await;
        assert_eq!(
            (st, err["code"].as_str()),
            (StatusCode::UNSUPPORTED_MEDIA_TYPE, Some("INVALID_INPUT")),
            "{bad_path}"
        );
    }

    let bare = raw(Method::POST, "/api/v1/servers/modpack/upload")
        .header(header::HOST, HOST)
        .header(header::CONTENT_TYPE, "multipart/form-data; boundary=b");
    let (st, _) = send(api.router.clone(), bare.body(Body::empty()).unwrap()).await;
    assert_eq!(st, StatusCode::FORBIDDEN);

    let spec = crate::openapi();
    for path in ["/api/v1/servers/modpack", "/api/v1/servers/modpack/upload"] {
        assert!(spec.paths.paths.contains_key(path), "thiếu {path}");
    }
}
