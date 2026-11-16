//! Cài nhiều mục trong một Job và sắp xếp khi tìm (spec trình duyệt mod, mục 3, 4).

use axum::http::StatusCode;
use serde_json::json;

use super::support::{content_api, create_with_loader, wait_job};

#[tokio::test]
async fn plan_and_install_many_items_share_dependencies_in_one_job() {
    let api = content_api(None);
    let fabric = create_with_loader(&api, "Many", "fabric", 25565).await;
    let body = json!({ "items": [
        { "source": "modrinth", "project_id": "sodium", "version_id": "sod-1" },
        { "source": "modrinth", "project_id": "iris" },
        { "source": "modrinth", "project_id": "fabric-api" },
    ] });

    let plan_uri = format!("/api/v1/servers/{fabric}/content/plan");
    let (status, plan) = api.post(&plan_uri, Some(body.clone())).await;
    assert_eq!(status, StatusCode::OK, "{plan}");
    let items = plan["items"].as_array().unwrap();
    let ids: Vec<_> = items
        .iter()
        .map(|i| i["project_id"].as_str().unwrap())
        .collect();
    // fabric-api vừa được chọn vừa là dependency của sodium: chỉ một dòng, tính là mục chính.
    assert_eq!(ids, ["sodium", "iris", "fabric-api"]);
    assert!(items.iter().all(|i| i["is_dependency"] == false));
    assert!(items.iter().all(|i| i["source"] == "modrinth"));
    assert_eq!(items[0]["version_id"], "sod-1");

    let (status, job) = api
        .post(&format!("/api/v1/servers/{fabric}/content"), Some(body))
        .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{job}");
    wait_job(&api, job["job_id"].as_u64().unwrap()).await;
    let (_, list) = api.get(&format!("/api/v1/servers/{fabric}/content")).await;
    let mut names: Vec<_> = list["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["file_name"].as_str().unwrap().to_owned())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "fabric-api-0.110.0.jar",
            "iris-1.8.0.jar",
            "sodium-0.6.0.jar"
        ]
    );
    assert!(
        list["files"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["needs_restart"] == false)
    );
}

#[tokio::test]
async fn install_body_and_sort_are_validated() {
    let api = content_api(None);
    let fabric = create_with_loader(&api, "Bad", "fabric", 25565).await;
    let one = json!({ "source": "modrinth", "project_id": "sodium" });
    let too_many: Vec<_> = (0..51).map(|_| one.clone()).collect();
    for body in [
        json!({ "items": [] }),
        json!({ "items": too_many }),
        json!({ "items": [{ "source": "modrinth", "project_id": " " }] }),
        json!({ "items": [{ "source": "modrinth", "project_id": "sodium", "version_id": "" }] }),
        json!({ "source": "modrinth", "project_id": "sodium" }),
    ] {
        for suffix in ["", "/plan"] {
            let uri = format!("/api/v1/servers/{fabric}/content{suffix}");
            let (status, _) = api.post(&uri, Some(body.clone())).await;
            assert!(status.is_client_error(), "{uri} {body}: {status}");
        }
    }

    for sort in ["relevance", "downloads", "updated"] {
        let uri = format!("/api/v1/content/search?kind=mod&q=&sort={sort}");
        assert_eq!(api.get(&uri).await.0, StatusCode::OK, "{sort}");
    }
    let (status, err) = api.get("/api/v1/content/search?kind=mod&sort=newest").await;
    assert_eq!(
        (status, err["code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("INVALID_INPUT"))
    );
}

#[tokio::test]
async fn browse_by_loader_and_create_server_with_content() {
    let api = content_api(None);
    let base = "/api/v1/content";
    let (status, res) = api
        .get(&format!("{base}/search?q=&loader=fabric&minecraft=1.21.4"))
        .await;
    assert_eq!(status, StatusCode::OK, "{res}");
    let (status, versions) = api
        .get(&format!(
            "{base}/modrinth/projects/sodium/versions?loader=fabric&minecraft=1.21.4"
        ))
        .await;
    assert_eq!(status, StatusCode::OK, "{versions}");
    assert_eq!(versions.as_array().unwrap().len(), 2);
    for bad in [
        "loader=fabric",
        "minecraft=1.21.4",
        "loader=nope&minecraft=1.21.4",
    ] {
        let (status, _) = api.get(&format!("{base}/search?q=&{bad}")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}");
    }
    let (status, err) = api
        .get(&format!(
            "{base}/modrinth/projects/sodium/versions?loader=vanilla&minecraft=1.21.4"
        ))
        .await;
    assert_eq!(
        (status, err["code"].as_str()),
        (StatusCode::CONFLICT, Some("CONTENT_INCOMPATIBLE"))
    );

    let body = json!({
        "name": "Kèm mod", "loader": "fabric", "minecraft": "1.21.4", "port": 25570,
        "content": [{ "source": "modrinth", "project_id": "sodium" }],
    });
    let (status, created) = api.post("/api/v1/servers", Some(body)).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{created}");
    wait_job(&api, created["job_id"].as_u64().unwrap()).await;
    let id = created["server_id"].as_str().unwrap();
    let (_, list) = api.get(&format!("/api/v1/servers/{id}/content")).await;
    assert_eq!(list["files"].as_array().unwrap().len(), 2, "{list}");

    let bad = json!({
        "name": "Sai", "loader": "fabric", "minecraft": "1.21.4",
        "content": [{ "source": "modrinth", "project_id": " " }],
    });
    assert_eq!(
        api.post("/api/v1/servers", Some(bad)).await.0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn create_server_applies_initial_properties_and_validates_keys() {
    let api = content_api(None);
    let body = json!({
        "name": "Tùy chỉnh",
        "loader": "fabric",
        "minecraft": "1.21.4",
        "port": 25571,
        "properties": [
            { "key": "online-mode", "value": "false" },
            { "key": "white-list", "value": "true" },
            { "key": "difficulty", "value": "hard" },
            { "key": "server-port", "value": "11111" },
        ],
    });
    let (status, created) = api.post("/api/v1/servers", Some(body)).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{created}");
    wait_job(&api, created["job_id"].as_u64().unwrap()).await;
    let id = created["server_id"].as_str().unwrap();

    let (status, props) = api.get(&format!("/api/v1/servers/{id}/properties")).await;
    assert_eq!(status, StatusCode::OK, "{props}");
    let entries = props["properties"].as_array().unwrap();
    assert!(entries.contains(&json!({ "key": "online-mode", "value": "false" })));
    assert!(entries.contains(&json!({ "key": "white-list", "value": "true" })));
    assert!(entries.contains(&json!({ "key": "difficulty", "value": "hard" })));
    // server-port luôn do MineHost giữ theo cổng đã chọn, không bị ghi đè.
    assert!(entries.contains(&json!({ "key": "server-port", "value": "25571" })));

    for bad in [
        json!({ "key": "bad=key", "value": "1" }),
        json!({ "key": "bad key", "value": "1" }),
        json!({ "key": "bad#key", "value": "1" }),
        json!({ "key": "ok", "value": "line1\nline2" }),
    ] {
        let invalid = json!({
            "name": "Lỗi",
            "loader": "fabric",
            "minecraft": "1.21.4",
            "properties": [bad],
        });
        assert_eq!(
            api.post("/api/v1/servers", Some(invalid)).await.0,
            StatusCode::BAD_REQUEST
        );
    }
}
