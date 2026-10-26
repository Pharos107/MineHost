//! Test route Job, catalog, thoát và chỉ số. Dùng lại tiện ích của `tests.rs`.

use super::*;

#[tokio::test]
async fn jobs_and_catalog() {
    let api = test_api();
    let (status, versions) = api.get("/api/v1/catalog/paper/versions").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(versions[0]["id"], "1.21.4");
    assert_eq!(versions[0]["release_time"], "2024-12-03T10:12:00Z");
    let (status, empty) = api.get("/api/v1/jobs").await;
    assert_eq!((status, empty), (StatusCode::OK, json!([])));
    let (status, err) = api.get("/api/v1/jobs?server_id=not-a-ulid").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(err["code"], "INVALID_INPUT");
    let (status, _) = api.get("/api/v1/jobs/999").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = api.post("/api/v1/jobs/999/cancel", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let mk = |name: &str| json!({ "name": name, "loader": "paper", "minecraft": "1.21.4" });
    let (_, first) = api.post("/api/v1/servers", Some(mk("A"))).await;
    let (_, second) = api.post("/api/v1/servers", Some(mk("B"))).await;
    let job = second["job_id"].as_u64().unwrap();
    let (status, job_dto) = api.get(&format!("/api/v1/jobs/{job}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(job_dto["kind"], "create_server");
    assert!(job_dto["started_at"].as_str().unwrap().contains('T'));
    assert!(job_dto.get("finished_at").is_some());

    let (status, listed) = api.get("/api/v1/jobs").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed[0]["id"], second["job_id"]);
    assert_eq!(listed[1]["id"], first["job_id"]);
    let id = first["server_id"].as_str().unwrap();
    let (status, only) = api.get(&format!("/api/v1/jobs?server_id={id}")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        only.as_array()
            .is_some_and(|rows| rows.len() == 1 && rows[0]["server_id"] == id)
    );

    let (status, _) = api.post(&format!("/api/v1/jobs/{job}/cancel"), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn shutdown_route_signals_the_binary() {
    let api = test_api();
    let (status, _) = api.post("/api/v1/app/shutdown", None).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let signalled = tokio::time::timeout(Duration::from_secs(1), api.app.shutdown_requested());
    assert!(signalled.await.is_ok());
}

#[tokio::test]
async fn metrics_route_checks_since_and_missing_server() {
    let api = test_api();
    let id = api.create_server("A").await;
    let base = format!("/api/v1/servers/{id}/metrics");
    let (status, body) = api.get(&base).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["samples"], json!([]));
    assert!(body["disk_bytes"].is_null() && body["running_since"].is_null());

    let since = format!("{base}?since=2020-01-01T00:00:00Z");
    let (status, body) = api.get(&since).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["samples"], json!([]));

    let (status, body) = api.get(&format!("{base}?since=khong-phai-ngay")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["code"], "INVALID_INPUT");

    let missing = "/api/v1/servers/01JB8Z6Q0K3V5T2N8XWJ4R7M9C/metrics";
    let (status, body) = api.get(missing).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert_eq!(body["code"], "NOT_FOUND");
}
