//! CurseForge Core API v1 với fixture dựng theo tài liệu chính thức, qua `wiremock`.

#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::time::Duration;

use mh_core::{ErrorCode, LoaderKind, McVersion};
use mh_net::{HttpClient, NetConfig, USER_AGENT};
use mh_providers::CurseForge;
use mh_providers::curseforge::{ClassId, RelationType, SearchFilters, SortField, mod_loader_type};
use wiremock::matchers::{body_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TEST_KEY: &str = "cf-secret-test-key-987654";

fn fixture(name: &str) -> String {
    let p = format!("{}/fixtures/curseforge/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(p).unwrap()
}

async fn setup() -> (MockServer, tempfile::TempDir, CurseForge) {
    setup_with_key(TEST_KEY).await
}

async fn setup_with_key(key: &str) -> (MockServer, tempfile::TempDir, CurseForge) {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();
    let mut config = NetConfig::new(tmp.path());
    config.retry_delays = vec![Duration::from_millis(1)];
    let http = Arc::new(HttpClient::new(config).unwrap());
    let cf = CurseForge::with_base_url(http, key, format!("{}/v1", server.uri()));
    (server, tmp, cf)
}

fn mc(v: &str) -> McVersion {
    McVersion::parse(v).unwrap()
}

#[tokio::test]
async fn search_sends_params_key_header_and_caches() {
    let (s, _t, cf) = setup().await;
    Mock::given(method("GET"))
        .and(path("/v1/mods/search"))
        .and(header("user-agent", USER_AGENT))
        .and(header("x-api-key", TEST_KEY))
        .and(query_param("gameId", "432"))
        .and(query_param("classId", "6"))
        .and(query_param("modLoaderType", "4"))
        .and(query_param("gameVersion", "1.21.1"))
        .and(query_param("searchFilter", "jei"))
        .and(query_param("sortField", "6"))
        .and(query_param("index", "20"))
        .and(query_param("pageSize", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture("search_mods.json")))
        .expect(1)
        .mount(&s)
        .await;

    let filters = SearchFilters {
        class_id: Some(ClassId::Mod),
        loader: Some(LoaderKind::Fabric),
        minecraft: Some(mc("1.21.1")),
        sort_field: SortField::TotalDownloads,
        index: 20,
        page_size: 100, // kẹp về tối đa 50 của CurseForge
    };
    let first = cf.search("jei", &filters).await.unwrap();
    let second = cf.search("jei", &filters).await.unwrap();
    assert_eq!(first, second);
    assert_eq!(first.hits.len(), 2);
    assert_eq!(first.hits[0].id, 238222);
    assert_eq!(first.hits[0].download_count, 250_000_000);
    assert_eq!(first.hits[1].id, 306612);
    assert!(first.hits[1].links.wiki_url.is_none());
    assert_eq!(first.pagination.total_count, 2);
}

#[tokio::test]
async fn search_modpack_and_ignores_unsupported_loaders() {
    let (s, _t, cf) = setup().await;
    Mock::given(method("GET"))
        .and(path("/v1/mods/search"))
        .and(query_param("gameId", "432"))
        .and(query_param("classId", "4471"))
        .and(query_param("modLoaderType", "6"))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture("search_mods.json")))
        .expect(1)
        .mount(&s)
        .await;

    let pack_filters = SearchFilters {
        class_id: Some(ClassId::Modpack),
        loader: Some(LoaderKind::NeoForge),
        ..Default::default()
    };
    assert_eq!(cf.search("", &pack_filters).await.unwrap().hits.len(), 2);

    // Paper, Purpur, Vanilla không dùng CurseForge trong P3 -> không gọi mạng.
    for loader in [LoaderKind::Paper, LoaderKind::Purpur, LoaderKind::Vanilla] {
        assert_eq!(mod_loader_type(loader), None);
        let filters = SearchFilters {
            loader: Some(loader),
            ..Default::default()
        };
        assert!(
            cf.search("worldedit", &filters)
                .await
                .unwrap()
                .hits
                .is_empty()
        );
        assert!(
            cf.files(238222, loader, &mc("1.21.1"))
                .await
                .unwrap()
                .is_empty()
        );
    }
}

#[tokio::test]
async fn mod_info_returns_details_and_caches() {
    let (s, _t, cf) = setup().await;
    Mock::given(method("GET"))
        .and(path("/v1/mods/238222"))
        .and(header("x-api-key", TEST_KEY))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture("mod_238222.json")))
        .expect(1)
        .mount(&s)
        .await;

    let m = cf.mod_info(238222).await.unwrap();
    let cached = cf.mod_info(238222).await.unwrap();
    assert_eq!(m, cached);
    assert_eq!(m.name, "Just Enough Items (JEI)");
    assert_eq!(m.slug, "jei");
    assert_eq!(m.summary, "View Items and Recipes");
    assert_eq!(m.download_count, 250_000_000);
    assert!(m.allows_distribution());
    assert!(m.logo.as_ref().unwrap().thumbnail_url.is_some());
    assert_eq!(
        m.links.source_url.as_deref(),
        Some("https://github.com/mezz/JustEnoughItems")
    );
}

#[tokio::test]
async fn files_filters_loader_mc_sorts_newest_and_parses_sha1_deps() {
    let (s, _t, cf) = setup().await;
    Mock::given(method("GET"))
        .and(path("/v1/mods/238222"))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture("mod_238222.json")))
        .expect(1)
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/mods/238222/files"))
        .and(header("x-api-key", TEST_KEY))
        .and(query_param("gameVersion", "1.21.1"))
        .and(query_param("modLoaderType", "4"))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture("mod_files.json")))
        .expect(1)
        .mount(&s)
        .await;

    let list = cf
        .files(238222, LoaderKind::Fabric, &mc("1.21.1"))
        .await
        .unwrap();
    assert_eq!(list.len(), 2);
    // Mới nhất đứng đầu (fixture đặt bản cũ trước bản mới).
    assert_eq!(list[0].id, 5600002);
    assert_eq!(list[1].id, 5600001);
    assert!(list[0].file_date > list[1].file_date);
    assert!(!list[0].manual_download);
    assert_eq!(
        list[0].api_download_url(),
        Some("https://mediafilez.forgecdn.net/files/5600/2/jei-1.21.1-fabric-19.16.0.220.jar")
    );
    assert_eq!(
        list[0].sha1.as_deref(),
        Some("003c114c85ca88ef3362e018deb6aca0c682d6a1")
    );
    assert!(list[0].checksum().is_some());
    assert_eq!(list[0].dependencies.len(), 2);
    assert_eq!(
        list[0].dependencies[0].relation_type,
        RelationType::RequiredDependency
    );
    assert_eq!(
        list[0].dependencies[1].relation_type,
        RelationType::OptionalDependency
    );
    let req_deps: Vec<u64> = list[0].required_dependencies().collect();
    assert_eq!(req_deps, vec![306612]);
}

#[tokio::test]
async fn files_by_id_posts_body_and_handles_null_download_url() {
    let (s, _t, cf) = setup().await;
    let ids = [5600002, 6100001];
    Mock::given(method("POST"))
        .and(path("/v1/mods/files"))
        .and(header("x-api-key", TEST_KEY))
        .and(body_json(serde_json::json!({ "fileIds": ids })))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture("files_by_id.json")))
        .expect(1)
        .mount(&s)
        .await;

    assert!(cf.files_by_id(&[]).await.unwrap().is_empty());
    let files = cf.files_by_id(&ids).await.unwrap();
    assert_eq!(files.len(), 2);
    assert!(!files[0].manual_download);
    assert!(files[0].api_download_url().is_some());
    assert!(files[1].manual_download);
    assert!(files[1].download_url.is_none());
    assert!(files[1].api_download_url().is_none());
}

#[tokio::test]
async fn marks_manual_download_when_disallowed_or_missing_url_with_page_link() {
    let (s, _t, cf) = setup().await;
    // Trường hợp 1: allowModDistribution = false dù file có downloadUrl.
    Mock::given(method("GET"))
        .and(path("/v1/mods/999001"))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture("mod_restricted.json")))
        .mount(&s)
        .await;
    let mut files_json: serde_json::Value =
        serde_json::from_str(&fixture("mod_files.json")).unwrap();
    files_json["data"][0]["modId"] = serde_json::json!(999001);
    files_json["data"][1]["modId"] = serde_json::json!(999001);
    files_json["data"][1]["downloadUrl"] = serde_json::Value::Null;
    Mock::given(method("GET"))
        .and(path("/v1/mods/999001/files"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&files_json))
        .mount(&s)
        .await;

    let restricted = cf
        .files(999001, LoaderKind::Forge, &mc("1.21.1"))
        .await
        .unwrap();
    assert_eq!(restricted.len(), 2);
    for f in &restricted {
        assert!(f.manual_download);
        assert!(f.api_download_url().is_none());
        assert_eq!(
            f.download_page_url.as_deref(),
            Some(
                format!(
                    "https://www.curseforge.com/minecraft/mc-mods/restricted-example/download/{}",
                    f.id
                )
                .as_str()
            )
        );
    }

    // Trường hợp 2: allowModDistribution = true nhưng file có downloadUrl = null, classId = 4471 (modpack).
    let mut pack_mod: serde_json::Value =
        serde_json::from_str(&fixture("mod_238222.json")).unwrap();
    pack_mod["data"]["id"] = serde_json::json!(888001);
    pack_mod["data"]["slug"] = serde_json::json!("cool-pack");
    pack_mod["data"]["classId"] = serde_json::json!(4471);
    Mock::given(method("GET"))
        .and(path("/v1/mods/888001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&pack_mod))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/mods/888001/files"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&files_json))
        .mount(&s)
        .await;

    let pack_files = cf
        .files(888001, LoaderKind::Quilt, &mc("1.21.1"))
        .await
        .unwrap();
    assert_eq!(pack_files.len(), 2);
    assert_eq!(pack_files[0].id, 5600002);
    assert!(pack_files[0].manual_download);
    assert_eq!(
        pack_files[0].download_page_url.as_deref(),
        Some("https://www.curseforge.com/minecraft/modpacks/cool-pack/download/5600002")
    );
    assert!(!pack_files[1].manual_download);
}

#[tokio::test]
async fn skips_files_outside_curseforge_allowlist_without_failing_list() {
    let (s, _t, cf) = setup().await;
    Mock::given(method("GET"))
        .and(path("/v1/mods/238222"))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture("mod_238222.json")))
        .mount(&s)
        .await;
    let mut list: serde_json::Value = serde_json::from_str(&fixture("mod_files.json")).unwrap();
    list["data"][0]["downloadUrl"] = serde_json::json!("https://evil.example/bad.jar");
    Mock::given(method("GET"))
        .and(path("/v1/mods/238222/files"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&list))
        .mount(&s)
        .await;

    let files = cf
        .files(238222, LoaderKind::Fabric, &mc("1.21.1"))
        .await
        .unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].id, 5600002);

    let mut batch: serde_json::Value = serde_json::from_str(&fixture("files_by_id.json")).unwrap();
    batch["data"][0]["downloadUrl"] = serde_json::json!("https://evil.example/bad.jar");
    Mock::given(method("POST"))
        .and(path("/v1/mods/files"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&batch))
        .mount(&s)
        .await;

    let got = cf.files_by_id(&[5600002, 6100001]).await.unwrap();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].id, 6100001);
}

#[tokio::test]
async fn maps_missing_key_401_403_404_429_bad_json_and_hides_key() {
    // Thiếu key -> CURSEFORGE_UNAVAILABLE, không gọi mạng.
    let (s, _t, empty_cf) = setup_with_key("   ").await;
    let err_empty = empty_cf.mod_info(238222).await.unwrap_err();
    assert_eq!(err_empty.code(), ErrorCode::CurseforgeUnavailable);
    assert!(s.received_requests().await.unwrap().is_empty());

    let (s, _t, cf) = setup().await;
    assert!(!format!("{cf:?}").contains(TEST_KEY));

    for (id, status) in [(1, 401u16), (2, 403)] {
        Mock::given(method("GET"))
            .and(path(format!("/v1/mods/{id}")))
            .respond_with(ResponseTemplate::new(status))
            .mount(&s)
            .await;
        let err = cf.mod_info(id).await.unwrap_err();
        assert_eq!(err.code(), ErrorCode::CurseforgeUnavailable);
        assert!(!err.to_string().contains(TEST_KEY));
        assert!(!format!("{err:?}").contains(TEST_KEY));
    }

    Mock::given(method("GET"))
        .and(path("/v1/mods/404"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&s)
        .await;
    let err_404 = cf.mod_info(404).await.unwrap_err();
    assert_eq!(err_404.code(), ErrorCode::ContentNotFound);
    assert!(!err_404.to_string().contains(TEST_KEY));

    Mock::given(method("GET"))
        .and(path("/v1/mods/429"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "30"))
        .mount(&s)
        .await;
    let err_429 = cf.mod_info(429).await.unwrap_err();
    assert_eq!(err_429.code(), ErrorCode::UpstreamRateLimited);
    assert_eq!(err_429.params()["retry_after_s"], 30);
    assert!(!err_429.to_string().contains(TEST_KEY));

    Mock::given(method("GET"))
        .and(path("/v1/mods/999"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"unexpected":true}"#))
        .mount(&s)
        .await;
    let err_json = cf.mod_info(999).await.unwrap_err();
    assert_eq!(err_json.code(), ErrorCode::NetworkUnavailable);
    assert!(!err_json.to_string().contains(TEST_KEY));
}
