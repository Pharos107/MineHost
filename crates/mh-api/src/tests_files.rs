//! Route tệp: etag, tải xuống, upload và guard multipart.

use std::path::PathBuf;

use axum::body::Body;
use axum::http::{HeaderMap, Method, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

use super::TestApi;

fn authed(method: Method, uri: &str) -> axum::http::request::Builder {
    super::raw(method, uri)
        .header(header::HOST, super::HOST)
        .header(header::ORIGIN, super::ORIGIN)
}

async fn exchange(
    api: &TestApi,
    builder: axum::http::request::Builder,
    body: Body,
) -> (StatusCode, Value, HeaderMap) {
    let response = api
        .router
        .clone()
        .oneshot(builder.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json, headers)
}

async fn put_text(
    api: &TestApi,
    uri: &str,
    text: &str,
    if_match: Option<&str>,
) -> (StatusCode, Value, HeaderMap) {
    let mut req = authed(Method::PUT, uri).header(header::CONTENT_TYPE, "application/json");
    if let Some(tag) = if_match {
        req = req.header(header::IF_MATCH, tag);
    }
    exchange(api, req, Body::from(json!({ "text": text }).to_string())).await
}

async fn get_json(api: &TestApi, uri: &str) -> (StatusCode, Value, HeaderMap) {
    exchange(api, authed(Method::GET, uri), Body::empty()).await
}

fn server_dir(api: &TestApi, id: &str) -> PathBuf {
    api._tmp.path().join("servers").join(id).join("server")
}

fn form(files: &[(&str, &[u8])]) -> (String, Vec<u8>) {
    let boundary = "MineHostBoundary";
    let mut body = Vec::new();
    for (name, bytes) in files {
        body.extend(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{name}\"\r\n\r\n"
            )
            .into_bytes(),
        );
        body.extend_from_slice(bytes);
        body.extend(b"\r\n");
    }
    body.extend(format!("--{boundary}--\r\n").into_bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}

async fn post_form(
    router: &axum::Router,
    uri: &str,
    files: &[(&str, &[u8])],
    origin: bool,
) -> (StatusCode, Value) {
    let (content_type, body) = form(files);
    // Không có Content-Length thì guard coi là không có body và để extractor từ chối,
    // nên không thấy `params.field`. Client thật luôn gửi độ dài hoặc chunked.
    let mut req = super::raw(Method::POST, uri)
        .header(header::HOST, super::HOST)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, body.len().to_string());
    if origin {
        req = req.header(header::ORIGIN, super::ORIGIN);
    }
    let response = router
        .clone()
        .oneshot(req.body(Body::from(body)).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

async fn mkdir(api: &TestApi, id: &str, path: &str) -> (StatusCode, Value) {
    api.post(
        &format!("/api/v1/servers/{id}/files/mkdir"),
        Some(json!({ "path": path })),
    )
    .await
}

#[tokio::test]
async fn list_write_etag_and_download() {
    let api = super::test_api();
    let id = api.create_server("A").await;
    let base = format!("/api/v1/servers/{id}");

    let (status, _) = mkdir(&api, &id, "box").await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, err) = mkdir(&api, &id, "box").await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(err["code"], "FILE_EXISTS");
    assert_eq!(mkdir(&api, &id, "box/Mods").await.0, StatusCode::NO_CONTENT);
    assert_eq!(
        mkdir(&api, &id, "box/config").await.0,
        StatusCode::NO_CONTENT
    );

    let banner = format!("{base}/files/content?path=box/Banner.txt");
    let avocado = format!("{base}/files/content?path=box/avocado.txt");
    assert_eq!(put_text(&api, &banner, "b", None).await.0, StatusCode::OK);
    assert_eq!(put_text(&api, &avocado, "a", None).await.0, StatusCode::OK);

    let (status, list, _) = get_json(&api, &format!("{base}/files?path=box")).await;
    assert_eq!(status, StatusCode::OK);
    let names: Vec<_> = list
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["config", "Mods", "avocado.txt", "Banner.txt"]);
    assert_eq!(list[0]["kind"], "dir");
    assert!(list[0]["size"].is_null());
    assert_eq!(list[2]["kind"], "file");
    assert_eq!(list[2]["size"], 1);
    assert!(list[2]["modified"].as_str().unwrap().contains('T'));

    let (status, _) = api
        .post(
            &format!("{base}/files/rename"),
            Some(json!({ "from": "box/avocado.txt", "to": "box/Mods/moved.txt" })),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let moved = format!("{base}/files/content?path=box/Mods/moved.txt");
    let (_, moved_body, _) = get_json(&api, &moved).await;
    assert_eq!(moved_body["text"], "a");

    let notes = format!("{base}/files/content?path=notes.txt");
    let (status, created, _) = put_text(&api, &notes, "hello", None).await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let etag = created["etag"].as_str().unwrap().to_owned();
    assert_eq!(etag.len(), 64);

    let (status, read, headers) = get_json(&api, &notes).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(read["text"], "hello");
    assert_eq!(read["etag"], etag);
    assert_eq!(read["size"], 5);
    assert_eq!(
        headers[header::ETAG].to_str().unwrap(),
        format!("\"{etag}\"")
    );

    let (status, err, _) = put_text(&api, &notes, "nope", Some("\"deadbeef\"")).await;
    assert_eq!(status, StatusCode::PRECONDITION_FAILED);
    assert_eq!(err["code"], "FILE_CHANGED");
    let (status, err, _) = put_text(&api, &notes, "nope", None).await;
    assert_eq!(status, StatusCode::PRECONDITION_FAILED);
    assert_eq!(err["code"], "FILE_CHANGED");
    let (_, same, _) = get_json(&api, &notes).await;
    assert_eq!(same["text"], "hello");

    let quoted = format!("\"{etag}\"");
    let (status, updated, _) = put_text(&api, &notes, "changed", Some(&quoted)).await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_ne!(updated["etag"], etag);

    let spaced = format!("{base}/files/content?path=my%20file.txt");
    assert_eq!(
        put_text(&api, &spaced, "spaced", None).await.0,
        StatusCode::OK
    );
    assert_download(
        &api,
        &format!("{base}/files/download?path=notes.txt"),
        "notes.txt",
        b"changed",
        "text/plain",
    )
    .await;
    assert_download(
        &api,
        &format!("{base}/files/download?path=my%20file.txt"),
        "my%20file.txt",
        b"spaced",
        "text/plain",
    )
    .await;

    let (status, err, _) = get_json(&api, &format!("{base}/files/download?path=box")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(err["code"], "INVALID_INPUT");

    for suffix in [
        "files?path=../x",
        "files/content?path=../x",
        "files/download?path=../x",
    ] {
        let (status, err, _) = get_json(&api, &format!("{base}/{suffix}")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{suffix} {err}");
        assert_eq!(err["code"], "PATH_OUTSIDE_ROOT");
    }
    let (status, err) = api
        .call(Method::DELETE, &format!("{base}/files?path=../x"), None)
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(err["code"], "PATH_OUTSIDE_ROOT");

    let (status, trashed) = api
        .call(
            Method::DELETE,
            &format!("{base}/files?path=notes.txt"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{trashed}");
    assert!(trashed["trash_path"].as_str().unwrap().contains("trash"));
}

async fn assert_download(api: &TestApi, uri: &str, encoded_name: &str, body: &[u8], mime: &str) {
    let response = api
        .router
        .clone()
        .oneshot(authed(Method::GET, uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK, "{uri}");
    assert_eq!(
        response.headers()[header::CONTENT_DISPOSITION]
            .to_str()
            .unwrap(),
        format!("attachment; filename*=UTF-8''{encoded_name}")
    );
    let content_type = response.headers()[header::CONTENT_TYPE].to_str().unwrap();
    assert!(content_type.starts_with(mime), "{content_type}");
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(bytes.as_ref(), body);
}

#[tokio::test]
async fn upload_saves_files_and_drops_part_when_too_large() {
    let api = super::test_api();
    let id = api.create_server("A").await;
    let router = crate::router_with(
        std::sync::Arc::clone(&api.app),
        super::PORT,
        8,
        crate::LanState::new(),
    );
    let uri = format!("/api/v1/servers/{id}/files/upload?path=");

    let (status, names) = post_form(
        &router,
        &uri,
        &[("hello.txt", b"hello"), ("blob.bin", &[1, 2, 3])],
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{names}");
    assert_eq!(names, json!(["hello.txt", "blob.bin"]));
    let dir = server_dir(&api, &id);
    assert_eq!(std::fs::read(dir.join("hello.txt")).unwrap(), b"hello");
    assert_download(
        &api,
        &format!("/api/v1/servers/{id}/files/download?path=blob.bin"),
        "blob.bin",
        &[1, 2, 3],
        "application/octet-stream",
    )
    .await;

    let (status, err) = post_form(&router, &uri, &[("big.bin", &[9u8; 32])], true).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE, "{err}");
    assert_eq!(err["code"], "FILE_TOO_LARGE");
    assert!(!dir.join("big.bin").exists());
    assert!(!dir.join("big.bin.part").exists());
    let leftover = std::fs::read_dir(&dir).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".part")
    });
    assert!(!leftover);
}

#[tokio::test]
async fn upload_requires_origin_and_other_routes_reject_multipart() {
    let api = super::test_api();
    let id = api.create_server("A").await;
    let upload = format!("/api/v1/servers/{id}/files/upload?path=");
    let (status, err) = post_form(&api.router, &upload, &[("a.txt", b"hi")], false).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(err["code"], "FORBIDDEN");

    let mkdir = format!("/api/v1/servers/{id}/files/mkdir");
    let (status, err) = post_form(&api.router, &mkdir, &[("a.txt", b"hi")], true).await;
    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(err["code"], "INVALID_INPUT");
    assert_eq!(err["params"]["field"], "content-type");
}

#[tokio::test]
async fn other_routes_keep_the_default_body_limit() {
    let api = super::test_api();
    let id = api.create_server("A").await;
    let uri = format!("/api/v1/servers/{id}/files/content?path=big.txt");
    // Hơn 2 MiB: axum từ chối trước handler. Handler thì trả FILE_TOO_LARGE (văn bản tối đa 1 MiB).
    let text = "x".repeat(2_097_153);
    let (status, body, _) = put_text(&api, &uri, &text, None).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE, "{body}");
    assert_eq!(body["code"], "INVALID_INPUT");
    assert!(!server_dir(&api, &id).join("big.txt").exists());
}

#[test]
fn openapi_lists_file_routes() {
    let spec = crate::openapi();
    for path in [
        "/api/v1/servers/{id}/files",
        "/api/v1/servers/{id}/files/content",
        "/api/v1/servers/{id}/files/download",
        "/api/v1/servers/{id}/files/upload",
        "/api/v1/servers/{id}/files/mkdir",
        "/api/v1/servers/{id}/files/rename",
    ] {
        assert!(spec.paths.paths.contains_key(path), "{path}");
    }
}
