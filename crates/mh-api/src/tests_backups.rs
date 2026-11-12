//! Route sao lưu và cài đặt lịch (spec P2, mục 5.8; kế hoạch PR-24).

use std::time::Duration;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

use super::TestApi;

async fn wait_job(api: &TestApi, job: &Value) {
    let job = job["job_id"].as_u64().unwrap();
    for _ in 0..500 {
        let (_, snapshot) = api.get(&format!("/api/v1/jobs/{job}")).await;
        if snapshot["state"] != "running" {
            assert_eq!(snapshot["state"], "succeeded", "{snapshot}");
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("Job không xong");
}

#[tokio::test]
async fn backup_routes_protect_restore_and_download() {
    let api = super::test_api();
    let id = api.create_server("A").await;
    let base = format!("/api/v1/servers/{id}/backups");

    let (status, job) = api
        .post(
            &base,
            Some(json!({ "note": " trước khi đổi ", "protected": true })),
        )
        .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{job}");
    wait_job(&api, &job).await;

    let (status, list) = api.get(&base).await;
    assert_eq!(status, StatusCode::OK);
    let item = &list[0];
    assert_eq!(
        (item["kind"].as_str(), item["note"].as_str()),
        (Some("manual"), Some("trước khi đổi"))
    );
    assert_eq!(item["protected"], true);
    let bid = item["id"].as_str().unwrap().to_owned();
    let one = format!("{base}/{bid}");

    let (status, body) = api.call(Method::DELETE, &one, None).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["code"], "BACKUP_PROTECTED");

    let restore = format!("{one}/restore");
    for body in [json!({}), json!({ "confirm": false })] {
        let (status, _) = api.post(&restore, Some(body)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
    let (_, body) = api.post(&restore, Some(json!({ "confirm": false }))).await;
    assert_eq!(body["params"]["field"], "confirm");

    let request = Request::builder()
        .uri(format!("{one}/download"))
        .header(header::HOST, super::HOST)
        .body(Body::empty())
        .unwrap();
    let response = api.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let disposition = response.headers()[header::CONTENT_DISPOSITION]
        .to_str()
        .unwrap()
        .to_owned();
    assert!(
        disposition.contains(&format!("{bid}.tar.zst")),
        "{disposition}"
    );
    assert!(
        !response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .is_empty()
    );

    let (status, item) = api
        .call(
            Method::PATCH,
            &one,
            Some(json!({ "protected": false, "note": "" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{item}");
    assert_eq!(
        (item["protected"].as_bool(), item["note"].is_null()),
        (Some(false), true)
    );

    let (status, job) = api.post(&restore, Some(json!({ "confirm": true }))).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{job}");
    wait_job(&api, &job).await;

    let (status, _) = api.call(Method::DELETE, &one, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, list) = api.get(&base).await;
    // Còn bản `before_restore` mà lần khôi phục vừa tạo.
    assert!(
        list.as_array()
            .unwrap()
            .iter()
            .all(|b| b["kind"] == "before_restore"),
        "{list}"
    );
}

#[tokio::test]
async fn bad_note_and_unknown_backup_are_rejected() {
    let api = super::test_api();
    let id = api.create_server("A").await;
    let base = format!("/api/v1/servers/{id}/backups");
    let long = "a".repeat(201);
    let (status, body) = api.post(&base, Some(json!({ "note": long }))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["params"]["field"], "note");
    let (status, _) = api
        .call(
            Method::DELETE,
            &format!("{base}/01JB8Z6Q0K3V5T2N8XWJ4R7M9C"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn backup_schedule_is_patched_on_the_server() {
    let api = super::test_api();
    let id = api.create_server("A").await;
    let uri = format!("/api/v1/servers/{id}");
    let (_, server) = api.get(&uri).await;
    assert_eq!(server["backup"], json!({ "schedule": "off", "keep": 10 }));

    let body = json!({ "backup": { "schedule": "daily@04:00", "keep": 5 } });
    let (status, server) = api.call(Method::PATCH, &uri, Some(body)).await;
    assert_eq!(status, StatusCode::OK, "{server}");
    assert_eq!(
        server["backup"],
        json!({ "schedule": "daily@04:00", "keep": 5 })
    );

    let (status, body) = api
        .call(
            Method::PATCH,
            &uri,
            Some(json!({ "backup": { "schedule": "hourly" } })),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["params"]["field"], "backup.schedule");
    let (status, _) = api
        .call(
            Method::PATCH,
            &uri,
            Some(json!({ "backup": { "keep": 0 } })),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
