//! Test route tải file thủ công cho mục chờ CurseForge (`POST /content/pending/upload`).

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use serde_json::{Value, json};

use super::super::{HOST, ORIGIN, send};
use super::catalog::store_zip;
use super::support::{content_api_with_cf, create_with_loader, wait_job};

async fn upload_pending_parts(
    router: &axum::Router,
    uri: &str,
    parts: &[(&str, &[u8])],
) -> (StatusCode, Value) {
    let boundary = "PendingBoundary";
    let mut body = Vec::new();
    for (name, bytes) in parts {
        body.extend(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{name}\"\r\n\r\n"
            )
            .into_bytes(),
        );
        body.extend_from_slice(bytes);
        body.extend_from_slice(b"\r\n");
    }
    body.extend(format!("--{boundary}--\r\n").into_bytes());
    let req = Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header(header::HOST, HOST)
        .header(header::ORIGIN, ORIGIN)
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .header(header::CONTENT_LENGTH, body.len().to_string())
        .body(Body::from(body))
        .unwrap();
    send(router.clone(), req).await
}

#[tokio::test]
async fn upload_pending_route_rejects_mismatch_batches_and_clears_preflight() {
    let api = content_api_with_cf(None, true);
    let fabric = create_with_loader(&api, "Fabric", "fabric", 25565).await;
    let (status, _) = api
        .post(
            &format!("/api/v1/servers/{fabric}/eula"),
            Some(json!({ "accept": true })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    // Cài mod CurseForge cấm phân phối -> sinh mục chờ kèm tên mod.
    let (status, job) = api
        .post(
            &format!("/api/v1/servers/{fabric}/content"),
            Some(json!({ "source": "curseforge", "project_id": "777001" })),
        )
        .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{job}");
    wait_job(&api, job["job_id"].as_u64().unwrap()).await;

    let (status, list) = api.get(&format!("/api/v1/servers/{fabric}/content")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list["pending"][0]["name"], "Restricted Mod");
    assert_eq!(list["pending"][0]["file_name"], "restricted.jar");

    // Chưa đủ file chờ -> start bị chặn.
    let (status, start_err) = api
        .post(&format!("/api/v1/servers/{fabric}/actions/start"), None)
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{start_err}");
    assert_eq!(start_err["code"], "CURSEFORGE_FILES_PENDING");

    let pending_uri = format!("/api/v1/servers/{fabric}/content/pending/upload");

    // Multipart rỗng -> 400 INVALID_INPUT.
    let (status, empty_err) = upload_pending_parts(&api.router, &pending_uri, &[]).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{empty_err}");
    assert_eq!(empty_err["code"], "INVALID_INPUT");

    // File sai hash -> ok=false, mã UPLOAD_HASH_MISMATCH kèm tên file đúng.
    let wrong_jar = store_zip("fabric.mod.json", br#"{"id":"wrong"}"#);
    let (status, res) =
        upload_pending_parts(&api.router, &pending_uri, &[("wrong.jar", &wrong_jar)]).await;
    assert_eq!(status, StatusCode::OK, "{res}");
    assert_eq!(res["results"].as_array().unwrap().len(), 1);
    assert_eq!(res["results"][0]["ok"], false);
    assert_eq!(res["results"][0]["error"]["code"], "UPLOAD_HASH_MISMATCH");
    assert_eq!(
        res["results"][0]["error"]["params"]["expected"],
        "restricted.jar"
    );

    // Gửi nhiều file một lần: một sai, một đúng hash của restricted.jar.
    let match_jar = store_zip("fabric.mod.json", b"{}");
    let (status, batch) = upload_pending_parts(
        &api.router,
        &pending_uri,
        &[("other.jar", &wrong_jar), ("UserPicked.jar", &match_jar)],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{batch}");
    let results = batch["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0]["file_name"], "other.jar");
    assert_eq!(results[0]["ok"], false);
    assert_eq!(results[0]["error"]["code"], "UPLOAD_HASH_MISMATCH");
    assert_eq!(results[1]["file_name"], "UserPicked.jar");
    assert_eq!(results[1]["ok"], true);
    let item = &results[1]["installed"];
    assert_eq!(item["file_name"], "restricted.jar");
    assert_eq!(item["source"], "curseforge");
    assert_eq!(item["project_id"], "777001");
    assert_eq!(item["version_id"], "7770001");

    // Mục chờ đã xóa và preflight hết chặn.
    let (status, list) = api.get(&format!("/api/v1/servers/{fabric}/content")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(list["pending"].as_array().unwrap().is_empty());
    let (status, pre) = api
        .get(&format!("/api/v1/servers/{fabric}/preflight"))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(pre["pending"].as_array().unwrap().is_empty());
    let (status, server_dto) = api.get(&format!("/api/v1/servers/{fabric}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(server_dto["preflight_blocking"], false);
}
