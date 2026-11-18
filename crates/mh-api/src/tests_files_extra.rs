//! Route bổ sung của trang Tệp: nhân bản, dung lượng, xem trước, ảnh, NBT, thùng rác, upload thư mục.

use axum::body::Body;
use axum::http::{Method, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::json;
use tower::ServiceExt;

use super::super::{HOST, ORIGIN, TestApi};
use super::{seed, server_dir};

async fn delete_req(api: &TestApi, uri: &str) -> StatusCode {
    let req = super::super::raw(Method::DELETE, uri)
        .header(header::HOST, HOST)
        .header(header::ORIGIN, ORIGIN)
        .body(Body::empty())
        .unwrap();
    api.router.clone().oneshot(req).await.unwrap().status()
}

#[tokio::test]
async fn copy_size_preview_raw_and_trash_routes() {
    let api = super::super::test_api();
    let id = api.create_server("A").await;
    seed(&api, &id);
    let base = format!("/api/v1/servers/{id}/files");
    std::fs::write(server_dir(&api, &id).join("icon.png"), b"\x89PNG\r\n\x1a\n").unwrap();

    let (status, _) = api
        .post(
            &format!("{base}/copy"),
            Some(json!({ "from": "plugins", "to": "plugins-2" })),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(
        server_dir(&api, &id)
            .join("plugins-2/Essentials/config.yml")
            .is_file()
    );

    let (status, size) = api.get(&format!("{base}/size?path=plugins")).await;
    assert_eq!(status, StatusCode::OK, "{size}");
    assert_eq!(size["files"], 2);
    assert_eq!(size["dirs"], 1);

    let (status, preview) = api
        .get(&format!(
            "{base}/preview?path=plugins/Essentials/config.yml"
        ))
        .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["text"], "a: 1");
    assert_eq!(preview["truncated"], false);

    let req = super::super::raw(Method::GET, &format!("{base}/raw?path=icon.png"))
        .header(header::HOST, HOST)
        .body(Body::empty())
        .unwrap();
    let resp = api.router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers()[header::CONTENT_TYPE], "image/png");
    assert_eq!(resp.headers()[header::X_CONTENT_TYPE_OPTIONS], "nosniff");

    // SVG bị từ chối để tránh XSS inline.
    std::fs::write(server_dir(&api, &id).join("bad.svg"), b"<svg/>").unwrap();
    let (status, err) = api.get(&format!("{base}/raw?path=bad.svg")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{err}");

    // Xóa rồi liệt kê, khôi phục, xóa lại rồi dọn sạch thùng rác.
    let status = delete_req(&api, &format!("{base}?path=plugins-2")).await;
    assert_eq!(status, StatusCode::OK);
    let (status, trash) = api.get(&format!("{base}/trash")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(trash.as_array().unwrap().len(), 1);
    let batch = trash[0]["id"].as_str().unwrap().to_owned();

    let (status, restored) = api
        .post(
            &format!("{base}/trash/restore"),
            Some(json!({ "batch": batch })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{restored}");
    assert!(
        server_dir(&api, &id)
            .join("plugins-2/Essentials/config.yml")
            .is_file()
    );

    let status = delete_req(&api, &format!("{base}?path=plugins-2")).await;
    assert_eq!(status, StatusCode::OK);
    let status = delete_req(&api, &format!("{base}/trash")).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, trash_after) = api.get(&format!("{base}/trash")).await;
    assert!(trash_after.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn relative_upload_creates_subfolders() {
    let api = super::super::test_api();
    let id = api.create_server("A").await;
    let boundary = "----mhbound";
    let body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"path\"\r\n\r\nsub/deep.txt\r\n\
         --{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"deep.txt\"\r\nContent-Type: text/plain\r\n\r\nhi\r\n--{boundary}--\r\n"
    );
    let req = super::super::raw(
        Method::POST,
        &format!("/api/v1/servers/{id}/files/upload?relative=true"),
    )
    .header(header::HOST, HOST)
    .header(header::ORIGIN, ORIGIN)
    .header(
        header::CONTENT_TYPE,
        format!("multipart/form-data; boundary={boundary}"),
    )
    .body(Body::from(body))
    .unwrap();
    let resp = api.router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(std::str::from_utf8(&bytes).unwrap(), r#"["sub/deep.txt"]"#);
    assert_eq!(
        std::fs::read_to_string(server_dir(&api, &id).join("sub/deep.txt")).unwrap(),
        "hi"
    );
}

#[test]
fn openapi_lists_extra_file_routes() {
    let spec = crate::openapi();
    for path in [
        "copy",
        "size",
        "preview",
        "nbt",
        "raw",
        "fetch",
        "trash",
        "trash/restore",
    ] {
        let full = format!("/api/v1/servers/{{id}}/files/{path}");
        assert!(spec.paths.paths.contains_key(&full), "{full}");
    }
}
