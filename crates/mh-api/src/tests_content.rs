//! Route mod/plugin và tìm kiếm (spec P3, mục 5.8; kế hoạch PR-18).

#[path = "tests_content_catalog.rs"]
mod catalog;
#[path = "tests_preflight.rs"]
mod preflight;
#[path = "tests_content_support.rs"]
mod support;

use std::sync::Arc;

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};
use tokio::sync::Notify;

use super::PORT;
use catalog::store_zip;
use support::{content_api, content_api_with_cf, create_with_loader, upload_jar, wait_job};

#[tokio::test]
async fn server_dto_reports_content_kind() {
    let api = content_api(None);
    let paper = create_with_loader(&api, "Paper", "paper", 25565).await;
    let fabric = create_with_loader(&api, "Fabric", "fabric", 25566).await;
    let vanilla = create_with_loader(&api, "Vanilla", "vanilla", 25567).await;

    for (id, expected) in [
        (&paper, json!("plugins")),
        (&fabric, json!("mods")),
        (&vanilla, Value::Null),
    ] {
        let (status, dto) = api.get(&format!("/api/v1/servers/{id}")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(dto["content_kind"], expected);
    }
}

#[tokio::test]
async fn search_project_and_versions_routes() {
    let api = content_api(None);
    let fabric = create_with_loader(&api, "Fabric", "fabric", 25565).await;
    let vanilla = create_with_loader(&api, "Vanilla", "vanilla", 25566).await;

    let (status, res) = api
        .get(&format!(
            "/api/v1/content/search?source=modrinth&kind=mod&q=&server_id={fabric}&page=1"
        ))
        .await;
    assert_eq!(status, StatusCode::OK, "{res}");
    assert_eq!(res["page"], 1);
    assert_eq!(res["page_size"], 20);
    let ids: Vec<_> = res["hits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["project_id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&"sodium") && ids.contains(&"fabric-api"));
    assert!(
        !ids.contains(&"iris"),
        "client-only phải bị lọc khi có server_id"
    );

    let (status, err) = api
        .get(&format!("/api/v1/content/search?server_id={vanilla}"))
        .await;
    assert_eq!(
        (status, err["code"].as_str()),
        (StatusCode::CONFLICT, Some("CONTENT_INCOMPATIBLE"))
    );

    for (uri, code, field) in [
        (
            "/api/v1/content/search?source=curseforge",
            "CURSEFORGE_UNAVAILABLE",
            None,
        ),
        (
            "/api/v1/content/search?source=spigot",
            "INVALID_INPUT",
            Some("source"),
        ),
        (
            "/api/v1/content/search?kind=theme",
            "INVALID_INPUT",
            Some("kind"),
        ),
        (
            "/api/v1/content/search?page=0",
            "INVALID_INPUT",
            Some("page"),
        ),
    ] {
        let (status, err) = api.get(uri).await;
        assert_eq!(err["code"], code, "{uri}: {err}");
        if let Some(f) = field {
            assert_eq!(status, StatusCode::BAD_REQUEST);
            assert_eq!(err["params"]["field"], f);
        }
    }

    let (status, proj) = api.get("/api/v1/content/modrinth/projects/sodium").await;
    assert_eq!(status, StatusCode::OK, "{proj}");
    assert_eq!(proj["title"], "Sodium");
    assert_eq!(proj["server_side"], "optional");
    assert_eq!(proj["links"]["page"], "https://modrinth.com/mod/sodium");
    let (status, err) = api.get("/api/v1/content/modrinth/projects/nope").await;
    assert_eq!(
        (status, err["code"].as_str()),
        (StatusCode::NOT_FOUND, Some("CONTENT_NOT_FOUND"))
    );
    let (status, err) = api.get("/api/v1/content/curseforge/projects/sodium").await;
    assert_eq!(
        (status, err["code"].as_str()),
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Some("CURSEFORGE_UNAVAILABLE")
        )
    );

    let (status, vers) = api
        .get(&format!(
            "/api/v1/content/modrinth/projects/sodium/versions?server_id={fabric}"
        ))
        .await;
    assert_eq!(status, StatusCode::OK, "{vers}");
    assert_eq!(vers[0]["version_number"], "0.6.1");
    assert_eq!(vers[1]["version_number"], "0.6.0");

    let (status, err) = api
        .get("/api/v1/content/modrinth/projects/sodium/versions")
        .await;
    assert_eq!(
        (status, err["code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("INVALID_INPUT"))
    );
    assert_eq!(err["params"]["field"], "server_id");
    let (status, err) = api
        .get(&format!(
            "/api/v1/content/modrinth/projects/sodium/versions?server_id={vanilla}"
        ))
        .await;
    assert_eq!(
        (status, err["code"].as_str()),
        (StatusCode::CONFLICT, Some("CONTENT_INCOMPATIBLE"))
    );
}

#[tokio::test]
async fn plan_install_list_toggle_and_remove_routes() {
    let api = content_api(None);
    let fabric = create_with_loader(&api, "Fabric", "fabric", 25565).await;
    let vanilla = create_with_loader(&api, "Vanilla", "vanilla", 25566).await;

    let (status, err) = api.get(&format!("/api/v1/servers/{vanilla}/content")).await;
    assert_eq!(
        (status, err["code"].as_str()),
        (StatusCode::CONFLICT, Some("CONTENT_INCOMPATIBLE"))
    );
    let (status, err) = api
        .post(
            &format!("/api/v1/servers/{vanilla}/content/plan"),
            Some(json!({ "source": "modrinth", "project_id": "sodium" })),
        )
        .await;
    assert_eq!(
        (status, err["code"].as_str()),
        (StatusCode::CONFLICT, Some("CONTENT_INCOMPATIBLE"))
    );

    let (status, plan) = api
        .post(
            &format!("/api/v1/servers/{fabric}/content/plan"),
            Some(json!({ "source": "modrinth", "project_id": "sodium" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{plan}");
    assert_eq!(plan["kind"], "mods");
    let items = plan["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["project_id"], "sodium");
    assert_eq!(items[0]["is_dependency"], false);
    assert_eq!(items[1]["project_id"], "fabric-api");
    assert_eq!(items[1]["is_dependency"], true);

    let (status, err) = api
        .post(
            &format!("/api/v1/servers/{fabric}/content"),
            Some(json!({ "source": "curseforge", "project_id": "sodium" })),
        )
        .await;
    assert_eq!(
        (status, err["code"].as_str()),
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Some("CURSEFORGE_UNAVAILABLE")
        )
    );

    let (status, job) = api
        .post(
            &format!("/api/v1/servers/{fabric}/content"),
            Some(json!({ "source": "modrinth", "project_id": "sodium" })),
        )
        .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{job}");
    wait_job(&api, job["job_id"].as_u64().unwrap()).await;

    let (status, list) = api.get(&format!("/api/v1/servers/{fabric}/content")).await;
    assert_eq!(status, StatusCode::OK, "{list}");
    assert_eq!(list["kind"], "mods");
    assert_eq!(list["pending"], json!([]));
    assert_eq!(list["files"].as_array().unwrap().len(), 2);

    let item_uri = format!("/api/v1/servers/{fabric}/content/sodium-0.6.1.jar");
    let (status, _) = api
        .call(Method::PATCH, &item_uri, Some(json!({ "enabled": false })))
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, list) = api.get(&format!("/api/v1/servers/{fabric}/content")).await;
    let sodium = list["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["file_name"] == "sodium-0.6.1.jar")
        .unwrap();
    assert_eq!(sodium["enabled"], false);

    let missing_uri = format!("/api/v1/servers/{fabric}/content/missing.jar");
    let (status, err) = api
        .call(
            Method::PATCH,
            &missing_uri,
            Some(json!({ "enabled": true })),
        )
        .await;
    assert_eq!(
        (status, err["code"].as_str()),
        (StatusCode::NOT_FOUND, Some("NOT_FOUND"))
    );

    let (status, _) = api.call(Method::DELETE, &item_uri, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, err) = api.call(Method::DELETE, &missing_uri, None).await;
    assert_eq!(
        (status, err["code"].as_str()),
        (StatusCode::NOT_FOUND, Some("NOT_FOUND"))
    );
}

#[tokio::test]
async fn upload_and_busy_server_errors() {
    let api = content_api(None);
    let fabric = create_with_loader(&api, "Fabric", "fabric", 25565).await;
    let vanilla = create_with_loader(&api, "Vanilla", "vanilla", 25566).await;

    let jar = store_zip(
        "fabric.mod.json",
        br#"{"schemaVersion":1,"id":"custom","version":"1.0","name":"Custom","environment":"client"}"#,
    );
    let upload_uri = format!("/api/v1/servers/{fabric}/content/upload");
    let (status, item) = upload_jar(&api.router, &upload_uri, "custom.jar", &jar).await;
    assert_eq!(status, StatusCode::OK, "{item}");
    assert_eq!(item["file_name"], "custom.jar");
    assert_eq!(item["source"], "upload");
    assert_eq!(item["client_only"], true);

    let (status, err) = upload_jar(&api.router, &upload_uri, "readme.txt", &jar).await;
    assert_eq!(
        (status, err["code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("INVALID_INPUT"))
    );
    assert_eq!(err["params"]["field"], "file");

    let (status, err) = upload_jar(&api.router, &upload_uri, "broken.jar", b"not-a-zip").await;
    assert_eq!(
        (status, err["code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("INVALID_INPUT"))
    );

    let small_router = crate::router_with(Arc::clone(&api.app), PORT, 16, crate::LanState::new());
    let (status, err) = upload_jar(&small_router, &upload_uri, "big.jar", &[1u8; 32]).await;
    assert_eq!(
        (status, err["code"].as_str()),
        (StatusCode::PAYLOAD_TOO_LARGE, Some("FILE_TOO_LARGE"))
    );

    let (status, err) = upload_jar(
        &api.router,
        &format!("/api/v1/servers/{vanilla}/content/upload"),
        "v.jar",
        &jar,
    )
    .await;
    assert_eq!(
        (status, err["code"].as_str()),
        (StatusCode::CONFLICT, Some("CONTENT_INCOMPATIBLE"))
    );

    // Server đang cài (giữ download trên Notify) phải trả 409 SERVER_BUSY ở các route sửa mod.
    let notify = Arc::new(Notify::new());
    let busy_api = content_api(Some(Arc::clone(&notify)));
    let (status, created) = busy_api
        .post(
            "/api/v1/servers",
            Some(json!({ "name": "Busy", "loader": "paper", "minecraft": "1.21.4" })),
        )
        .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let busy_id = created["server_id"].as_str().unwrap();

    for (method, suffix, body) in [
        (
            Method::POST,
            "",
            Some(json!({ "source": "modrinth", "project_id": "sodium" })),
        ),
        (Method::PATCH, "/a.jar", Some(json!({ "enabled": false }))),
        (Method::DELETE, "/a.jar", None),
    ] {
        let uri = format!("/api/v1/servers/{busy_id}/content{suffix}");
        let (status, err) = busy_api.call(method, &uri, body).await;
        assert_eq!(
            (status, err["code"].as_str()),
            (StatusCode::CONFLICT, Some("SERVER_BUSY")),
            "{uri}"
        );
    }
    let (status, err) = upload_jar(
        &busy_api.router,
        &format!("/api/v1/servers/{busy_id}/content/upload"),
        "a.jar",
        &jar,
    )
    .await;
    assert_eq!(
        (status, err["code"].as_str()),
        (StatusCode::CONFLICT, Some("SERVER_BUSY"))
    );
    notify.notify_waiters();
}

#[test]
fn openapi_lists_content_routes() {
    let spec = crate::openapi();
    for path in [
        "/api/v1/content/search",
        "/api/v1/content/{source}/projects/{pid}",
        "/api/v1/content/{source}/projects/{pid}/versions",
        "/api/v1/servers/{id}/content",
        "/api/v1/servers/{id}/content/plan",
        "/api/v1/servers/{id}/content/upload",
        "/api/v1/servers/{id}/content/{file}",
    ] {
        assert!(spec.paths.paths.contains_key(path), "thiếu {path}");
    }
}

#[tokio::test]
async fn curseforge_routes_cover_search_project_versions_plan_and_pending() {
    let api = content_api_with_cf(None, true);
    let fabric = create_with_loader(&api, "Fabric CF", "fabric", 25565).await;

    let (status, search) = api
        .get(&format!(
            "/api/v1/content/search?source=curseforge&server_id={fabric}"
        ))
        .await;
    assert_eq!(status, StatusCode::OK, "{search}");
    assert_eq!(search["hits"][0]["source"], "curseforge");
    assert_eq!(search["hits"][0]["server_side"], Value::Null);

    let (status, proj) = api.get("/api/v1/content/curseforge/projects/238222").await;
    assert_eq!(status, StatusCode::OK, "{proj}");
    assert_eq!(proj["source"], "curseforge");
    assert_eq!(proj["body"], Value::Null);
    assert_eq!(proj["server_side"], Value::Null);

    let (status, vers) = api
        .get(&format!(
            "/api/v1/content/curseforge/projects/238222/versions?server_id={fabric}"
        ))
        .await;
    assert_eq!(status, StatusCode::OK, "{vers}");
    assert_eq!(vers[0]["version_number"], Value::Null);

    let (status, plan) = api
        .post(
            &format!("/api/v1/servers/{fabric}/content/plan"),
            Some(json!({ "source": "curseforge", "project_id": "238222" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{plan}");
    assert_eq!(plan["items"].as_array().unwrap().len(), 2);
    assert_eq!(plan["items"][0]["manual_download"], false);

    let (status, job) = api
        .post(
            &format!("/api/v1/servers/{fabric}/content"),
            Some(json!({ "source": "curseforge", "project_id": "238222" })),
        )
        .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{job}");
    wait_job(&api, job["job_id"].as_u64().unwrap()).await;

    let (status, rplan) = api
        .post(
            &format!("/api/v1/servers/{fabric}/content/plan"),
            Some(json!({ "source": "curseforge", "project_id": "777001" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{rplan}");
    assert_eq!(rplan["items"][0]["manual_download"], true);
    assert!(
        rplan["items"][0]["download_page_url"]
            .as_str()
            .unwrap()
            .contains("/download/7770001")
    );

    let (status, rjob) = api
        .post(
            &format!("/api/v1/servers/{fabric}/content"),
            Some(json!({ "source": "curseforge", "project_id": "777001" })),
        )
        .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{rjob}");
    wait_job(&api, rjob["job_id"].as_u64().unwrap()).await;

    let (status, list) = api.get(&format!("/api/v1/servers/{fabric}/content")).await;
    assert_eq!(status, StatusCode::OK, "{list}");
    assert_eq!(list["files"].as_array().unwrap().len(), 2);
    assert_eq!(list["files"][0]["source"], "curseforge");
    let pending = list["pending"].as_array().unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0]["project_id"], 777001);
    assert_eq!(pending[0]["file_id"], 7770001);
}
