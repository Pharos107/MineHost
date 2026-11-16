//! Route nhiều mục của trang Tệp: tìm, zip tải xuống, nén, giải nén, di chuyển.

use std::path::PathBuf;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Method, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

use super::TestApi;

fn server_dir(api: &TestApi, id: &str) -> PathBuf {
    api._tmp.path().join("servers").join(id).join("server")
}

async fn wait_job(api: &TestApi, job: &Value) -> Value {
    let job = job["job_id"].as_u64().unwrap();
    for _ in 0..500 {
        let (_, snapshot) = api.get(&format!("/api/v1/jobs/{job}")).await;
        if snapshot["state"] != "running" {
            return snapshot;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("Job không xong");
}

fn seed(api: &TestApi, id: &str) {
    let dir = server_dir(api, id);
    std::fs::create_dir_all(dir.join("plugins/Essentials")).unwrap();
    std::fs::write(dir.join("plugins/Essentials/config.yml"), "a: 1").unwrap();
    std::fs::write(dir.join("plugins/Essentials.jar"), "jar").unwrap();
}

#[tokio::test]
async fn search_finds_nested_files() {
    let api = super::test_api();
    let id = api.create_server("A").await;
    seed(&api, &id);
    let (status, body) = api
        .get(&format!("/api/v1/servers/{id}/files/search?q=config"))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["truncated"], false);
    assert_eq!(body["items"][0]["path"], "plugins/Essentials/config.yml");
    assert_eq!(body["items"][0]["kind"], "file");
    assert_eq!(body["items"][0]["size"], 4);

    let (status, body) = api
        .get(&format!("/api/v1/servers/{id}/files/search?q=%20"))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], "INVALID_INPUT");
}

#[tokio::test]
async fn archive_streams_a_zip_of_the_selection() {
    let api = super::test_api();
    let id = api.create_server("A").await;
    seed(&api, &id);
    let paths = r#"["plugins/Essentials","plugins/Essentials.jar"]"#;
    let uri = format!(
        "/api/v1/servers/{id}/files/archive?paths={}",
        paths
            .replace('"', "%22")
            .replace('[', "%5B")
            .replace(']', "%5D")
            .replace(',', "%2C")
    );
    let req = super::raw(Method::GET, &uri)
        .header(header::HOST, super::HOST)
        .body(Body::empty())
        .unwrap();
    let response = api.router.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "application/zip");
    assert_eq!(
        response.headers()[header::CONTENT_DISPOSITION],
        "attachment; filename*=UTF-8''plugins.zip"
    );
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert!(bytes.starts_with(b"PK\x03\x04"));
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("Essentials/config.yml"));
    assert!(text.contains("Essentials.jar"));

    let (status, body) = api
        .get(&format!(
            "/api/v1/servers/{id}/files/archive?paths=%5B%22nope%22%5D"
        ))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (status, _) = api
        .get(&format!("/api/v1/servers/{id}/files/archive?paths=world"))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn compress_then_extract_as_jobs() {
    let api = super::test_api();
    let id = api.create_server("A").await;
    seed(&api, &id);
    let base = format!("/api/v1/servers/{id}/files");
    let body = json!({ "paths": ["plugins"], "dest": "plugins.zip" });
    let (status, job) = api
        .post(&format!("{base}/compress"), Some(body.clone()))
        .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{job}");
    let done = wait_job(&api, &job).await;
    assert_eq!(done["state"], "succeeded", "{done}");
    assert_eq!(done["kind"], "compress_files");
    assert!(server_dir(&api, &id).join("plugins.zip").is_file());

    // Đích đã có: báo ngay, không tạo Job.
    let (status, err) = api.post(&format!("{base}/compress"), Some(body)).await;
    assert_eq!(status, StatusCode::CONFLICT, "{err}");
    assert_eq!(err["code"], "FILE_EXISTS");

    // Giải nén đè lên thư mục gốc thì trùng file đã có.
    let (status, err) = api
        .post(
            &format!("{base}/extract"),
            Some(json!({ "path": "plugins.zip" })),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{err}");
    assert_eq!(err["params"]["path"], "plugins/Essentials.jar");

    let (status, job) = api
        .post(
            &format!("{base}/extract"),
            Some(json!({ "path": "plugins.zip", "dest": "copy" })),
        )
        .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{job}");
    let done = wait_job(&api, &job).await;
    assert_eq!(done["state"], "succeeded", "{done}");
    let copy = server_dir(&api, &id).join("copy/plugins/Essentials/config.yml");
    assert_eq!(std::fs::read_to_string(copy).unwrap(), "a: 1");
}

#[tokio::test]
async fn rename_refuses_moving_a_folder_into_itself() {
    let api = super::test_api();
    let id = api.create_server("A").await;
    seed(&api, &id);
    let uri = format!("/api/v1/servers/{id}/files/rename");
    let (status, err) = api
        .post(
            &uri,
            Some(json!({ "from": "plugins", "to": "plugins/Essentials/plugins" })),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{err}");
    assert_eq!(err["code"], "INVALID_INPUT");
    let (status, _) = api
        .post(&uri, Some(json!({ "from": "plugins/Essentials.jar", "to": "plugins/Essentials/Essentials.jar" })))
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(
        server_dir(&api, &id)
            .join("plugins/Essentials/Essentials.jar")
            .is_file()
    );
}

#[test]
fn openapi_lists_file_ops_routes() {
    let spec = crate::openapi();
    for path in ["search", "archive", "compress", "extract"] {
        let path = format!("/api/v1/servers/{{id}}/files/{path}");
        assert!(spec.paths.paths.contains_key(&path), "{path}");
    }
}
