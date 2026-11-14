//! Modrinth API v2 với fixture lấy từ API thật (ngày 2026-11-14, đã rút gọn), qua `wiremock`.

#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::time::Duration;

use mh_core::{ErrorCode, LoaderKind, McVersion};
use mh_net::{HttpClient, NetConfig, USER_AGENT};
use mh_providers::Modrinth;
use mh_providers::modrinth::{
    DependencyType, ProjectType, SearchFilters, SearchIndex, SideSupport,
};
use wiremock::matchers::{body_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn fixture(name: &str) -> String {
    let p = format!("{}/fixtures/modrinth/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(p).unwrap()
}

async fn setup() -> (MockServer, tempfile::TempDir, Modrinth) {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();
    let mut config = NetConfig::new(tmp.path());
    config.retry_delays = vec![Duration::from_millis(1)];
    let http = Arc::new(HttpClient::new(config).unwrap());
    let mr = Modrinth::with_base_url(http, format!("{}/v2", server.uri()));
    (server, tmp, mr)
}

fn mc(v: &str) -> McVersion {
    McVersion::parse(v).unwrap()
}

#[tokio::test]
async fn search_sends_facets_user_agent_and_caches() {
    let (s, _t, mr) = setup().await;
    let expected_facets = r#"[["project_type:mod"],["categories:fabric"],["versions:1.21.1"],["server_side!=unsupported"]]"#;
    Mock::given(method("GET"))
        .and(path("/v2/search"))
        .and(header("user-agent", USER_AGENT))
        .and(query_param("query", "sodium"))
        .and(query_param("facets", expected_facets))
        .and(query_param("index", "downloads"))
        .and(query_param("offset", "10"))
        .and(query_param("limit", "5"))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture("search_sodium.json")))
        .expect(1)
        .mount(&s)
        .await;

    let filters = SearchFilters {
        project_type: Some(ProjectType::Mod),
        loader: Some(LoaderKind::Fabric),
        minecraft: Some(mc("1.21.1")),
        server_only: true,
        index: SearchIndex::Downloads,
        offset: 10,
        limit: 5,
    };
    let first = mr.search("sodium", &filters).await.unwrap();
    let second = mr.search("sodium", &filters).await.unwrap();
    assert_eq!(first, second);
    assert_eq!(first.hits.len(), 3);
    assert_eq!(first.hits[0].project_id, "AANobbMI");
    assert_eq!(first.hits[0].server_side, SideSupport::Unsupported);
    assert!(first.hits[0].server_side.is_unsupported());
    assert_eq!(first.hits[2].project_id, "EIa1eiMm");
    assert_eq!(first.hits[2].server_side, SideSupport::Required);
}

#[tokio::test]
async fn search_plugin_maps_paper_and_purpur_to_four_loaders() {
    let (s, _t, mr) = setup().await;
    let expected_facets = r#"[["project_type:plugin"],["categories:paper","categories:purpur","categories:spigot","categories:bukkit"]]"#;
    Mock::given(method("GET"))
        .and(path("/v2/search"))
        .and(query_param("facets", expected_facets))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture("search_sodium.json")))
        .expect(2)
        .mount(&s)
        .await;

    for (loader, q) in [
        (LoaderKind::Paper, "luckperms"),
        (LoaderKind::Purpur, "chunky"),
    ] {
        let filters = SearchFilters {
            project_type: Some(ProjectType::Plugin),
            loader: Some(loader),
            ..Default::default()
        };
        mr.search(q, &filters).await.unwrap();
    }
}

#[tokio::test]
async fn project_returns_details_links_and_caches() {
    let (s, _t, mr) = setup().await;
    Mock::given(method("GET"))
        .and(path("/v2/project/AANobbMI"))
        .and(header("user-agent", USER_AGENT))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture("project_sodium.json")))
        .expect(1)
        .mount(&s)
        .await;

    let p = mr.project("AANobbMI").await.unwrap();
    let cached = mr.project("AANobbMI").await.unwrap();
    assert_eq!(p, cached);
    assert_eq!(p.slug, "sodium");
    assert_eq!(p.title, "Sodium");
    assert!(!p.description.is_empty());
    assert!(!p.body.is_empty());
    assert!(
        p.icon_url
            .as_deref()
            .unwrap()
            .starts_with("https://cdn.modrinth.com/")
    );
    assert_eq!(p.server_side, SideSupport::Unsupported);
    assert_eq!(p.client_side, SideSupport::Required);
    assert_eq!(
        p.links.source_url.as_deref(),
        Some("https://github.com/CaffeineMC/sodium")
    );
    assert_eq!(
        p.links.discord_url.as_deref(),
        Some("https://caffeinemc.net/discord")
    );
    assert_eq!(p.page_url(), "https://modrinth.com/mod/sodium");
}

#[tokio::test]
async fn versions_filters_loader_and_mc_newest_first() {
    let (s, _t, mr) = setup().await;
    Mock::given(method("GET"))
        .and(path("/v2/project/AANobbMI/version"))
        .and(query_param("loaders", r#"["fabric"]"#))
        .and(query_param("game_versions", r#"["1.21.1"]"#))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(fixture("versions_sodium_fabric_1_21_1.json")),
        )
        .expect(1)
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/v2/project/plugin-x/version"))
        .and(query_param(
            "loaders",
            r#"["paper","purpur","spigot","bukkit"]"#,
        ))
        .and(query_param("game_versions", r#"["1.21.4"]"#))
        .respond_with(ResponseTemplate::new(200).set_body_string("[]"))
        .expect(1)
        .mount(&s)
        .await;

    let list = mr
        .versions("AANobbMI", LoaderKind::Fabric, &mc("1.21.1"))
        .await
        .unwrap();
    assert_eq!(list.len(), 3);
    assert_eq!(list[0].id, "SMxNOGZ6");
    assert!(list[0].date_published > list[1].date_published);
    assert!(list[1].date_published > list[2].date_published);

    let plugins = mr
        .versions("plugin-x", LoaderKind::Paper, &mc("1.21.4"))
        .await
        .unwrap();
    assert!(plugins.is_empty());

    assert!(
        mr.versions("anything", LoaderKind::Vanilla, &mc("1.21.4"))
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn version_reads_files_and_dependencies() {
    let (s, _t, mr) = setup().await;
    Mock::given(method("GET"))
        .and(path("/v2/version/bAo1Qhte"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(fixture("version_bAo1Qhte_iris.json")),
        )
        .expect(1)
        .mount(&s)
        .await;

    let v = mr.version("bAo1Qhte").await.unwrap();
    assert_eq!(v.id, "bAo1Qhte");
    let file = v.primary_file().unwrap();
    assert!(file.primary);
    assert!(file.url.starts_with("https://cdn.modrinth.com/"));
    assert!(file.filename.ends_with(".jar"));
    assert!(file.size > 0);
    assert_eq!(file.hashes.sha1.len(), 40);
    assert_eq!(file.hashes.sha512.len(), 128);
    assert!(file.checksum().is_some());

    assert_eq!(v.dependencies.len(), 1);
    assert_eq!(v.dependencies[0].project_id.as_deref(), Some("AANobbMI"));
    assert_eq!(v.dependencies[0].version_id.as_deref(), Some("SMxNOGZ6"));
    assert_eq!(v.dependencies[0].dependency_type, DependencyType::Required);
}

#[tokio::test]
async fn versions_by_hash_posts_sha1_and_omits_missing() {
    let (s, _t, mr) = setup().await;
    let h1 = "003c114c85ca88ef3362e018deb6aca0c682d6a1";
    let h2 = "d01f2112698d08b052d042c68c1206af790e660a";
    let missing = "0000000000000000000000000000000000000000";
    let expected_body = serde_json::json!({
        "hashes": [h1, h2, missing],
        "algorithm": "sha1",
    });
    Mock::given(method("POST"))
        .and(path("/v2/version_files"))
        .and(header("user-agent", USER_AGENT))
        .and(body_json(&expected_body))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture("version_files.json")))
        .expect(1)
        .mount(&s)
        .await;

    let map = mr.versions_by_hash(&[h1, h2, missing]).await.unwrap();
    assert_eq!(map.len(), 2);
    assert_eq!(map[h1].id, "SMxNOGZ6");
    assert_eq!(map[h2].id, "QV48eyCs");
    assert!(!map.contains_key(missing));

    assert!(mr.versions_by_hash::<&str>(&[]).await.unwrap().is_empty());
}

#[tokio::test]
async fn errors_404_429_and_bad_json_map_to_expected_codes() {
    let (s, _t, mr) = setup().await;
    Mock::given(method("GET"))
        .and(path("/v2/project/missing"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/v2/version/missing-ver"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/v2/project/rate-limited"))
        .respond_with(ResponseTemplate::new(429).insert_header("X-Ratelimit-Reset", "45"))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/v2/project/bad-json"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"unexpected":true}"#))
        .mount(&s)
        .await;

    let err_404 = mr.project("missing").await.unwrap_err();
    assert_eq!(err_404.code(), ErrorCode::ContentNotFound);

    let err_ver = mr.version("missing-ver").await.unwrap_err();
    assert_eq!(err_ver.code(), ErrorCode::ContentNotFound);

    let err_429 = mr.project("rate-limited").await.unwrap_err();
    assert_eq!(err_429.code(), ErrorCode::UpstreamRateLimited);
    assert_eq!(err_429.params()["retry_after_s"], 45);

    let err_json = mr.project("bad-json").await.unwrap_err();
    assert_eq!(err_json.code(), ErrorCode::NetworkUnavailable);
}

#[tokio::test]
async fn file_url_outside_modrinth_allowlist_is_rejected() {
    let (s, _t, mr) = setup().await;
    let tampered = fixture("version_SMxNOGZ6.json")
        .replace("https://cdn.modrinth.com", "https://evil.example");
    Mock::given(method("GET"))
        .and(path("/v2/version/SMxNOGZ6"))
        .respond_with(ResponseTemplate::new(200).set_body_string(tampered))
        .mount(&s)
        .await;

    let err = mr.version("SMxNOGZ6").await.unwrap_err();
    assert_eq!(err.code(), ErrorCode::DownloadHostNotAllowed);
}
