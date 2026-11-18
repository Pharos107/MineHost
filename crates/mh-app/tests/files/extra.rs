//! Test cho các tính năng nâng cấp trang Tệp: copy, size, preview, nbt, tar.gz, upload thư mục, trash, fetch.

use std::io::{Cursor, Write};

use flate2::Compression;
use flate2::write::GzEncoder;
use mh_app::{JobState, PREVIEW_MAX_BYTES};
use mh_core::ErrorCode;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::support::{Env, create};

#[tokio::test]
async fn copy_and_dir_size_work_and_guard_cycles() {
    let env = Env::new();
    let app = env.app();
    let spec = create(&app).await;
    let server = env.server(&spec);
    std::fs::create_dir_all(server.join("plugins/Sub")).unwrap();
    std::fs::write(server.join("plugins/a.txt"), b"hello").unwrap();
    std::fs::write(server.join("plugins/Sub/b.txt"), b"world!").unwrap();

    let size = app.dir_size(spec.id, "plugins").await.unwrap();
    assert_eq!(size.bytes, 11);
    assert_eq!(size.files, 2);
    assert_eq!(size.dirs, 1);
    assert!(!size.truncated);

    app.copy_file(spec.id, "plugins", "plugins-copy")
        .await
        .unwrap();
    assert_eq!(
        std::fs::read(server.join("plugins-copy/Sub/b.txt")).unwrap(),
        b"world!"
    );
    assert_eq!(
        app.copy_file(spec.id, "plugins", "plugins-copy")
            .await
            .unwrap_err()
            .code,
        ErrorCode::FileExists
    );
    assert_eq!(
        app.copy_file(spec.id, "plugins", "plugins/Sub/loop")
            .await
            .unwrap_err()
            .code,
        ErrorCode::InvalidInput
    );
}

#[tokio::test]
async fn preview_tails_large_utf8_and_nbt_decodes_gzip() {
    let env = Env::new();
    let app = env.app();
    let spec = create(&app).await;
    let server = env.server(&spec);

    let line = "2026-11-18 INFO dòng log tiếng Việt\n";
    let repeat = (PREVIEW_MAX_BYTES as usize / line.len()) + 50;
    let big = line.repeat(repeat);
    std::fs::write(server.join("big.log"), big.as_bytes()).unwrap();
    let preview = app.read_preview(spec.id, "big.log").await.unwrap();
    assert!(preview.truncated);
    assert_eq!(preview.total_bytes, big.len() as u64);
    assert!(preview.shown_bytes <= PREVIEW_MAX_BYTES);
    assert!(preview.text.starts_with("2026-11-18"));

    std::fs::write(server.join("bin.dat"), b"\x00\x01\x02").unwrap();
    assert_eq!(
        app.read_preview(spec.id, "bin.dat").await.unwrap_err().code,
        ErrorCode::FileNotText
    );

    // NBT: Compound { name: "world" }
    let mut nbt = Vec::new();
    nbt.extend_from_slice(&[0x0a, 0x00, 0x00]); // root compound ""
    nbt.extend_from_slice(&[0x08, 0x00, 0x04]); // string "name"
    nbt.extend_from_slice(b"name");
    nbt.extend_from_slice(&[0x00, 0x05]);
    nbt.extend_from_slice(b"world");
    nbt.push(0x00); // end compound

    let mut gz = GzEncoder::new(Vec::new(), Compression::default());
    gz.write_all(&nbt).unwrap();
    std::fs::write(server.join("level.dat"), gz.finish().unwrap()).unwrap();
    let decoded = app.read_nbt(spec.id, "level.dat").await.unwrap();
    assert!(decoded.compressed);
    assert!(decoded.json.contains("\"name\": \"world\""));
}

#[tokio::test]
async fn relative_upload_and_trash_restore_and_empty() {
    let env = Env::new();
    let app = env.app();
    let spec = create(&app).await;
    let server = env.server(&spec);
    app.create_dir(spec.id, "plugins").await.unwrap();

    let saved = app
        .save_upload_relative(
            spec.id,
            "plugins",
            "MyPack/config.yml",
            Cursor::new(b"x: 1".to_vec()),
            1024,
        )
        .await
        .unwrap();
    assert_eq!(saved, "MyPack/config.yml");
    assert_eq!(
        std::fs::read_to_string(server.join("plugins/MyPack/config.yml")).unwrap(),
        "x: 1"
    );
    assert_eq!(
        app.save_upload_relative(
            spec.id,
            "plugins",
            "../escape.txt",
            Cursor::new(b"x".to_vec()),
            1024
        )
        .await
        .unwrap_err()
        .code,
        ErrorCode::InvalidInput
    );

    app.delete(spec.id, "plugins/MyPack").await.unwrap();
    assert!(!server.join("plugins/MyPack").exists());
    let batches = app.list_trash(spec.id).await.unwrap();
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].items, vec!["plugins/MyPack/config.yml"]);

    let restored = app.restore_trash(spec.id, &batches[0].id).await.unwrap();
    assert_eq!(restored, vec!["plugins/MyPack"]);
    assert!(server.join("plugins/MyPack/config.yml").is_file());

    app.delete(spec.id, "plugins/MyPack").await.unwrap();
    assert_eq!(app.list_trash(spec.id).await.unwrap().len(), 1);
    app.empty_trash(spec.id, None).await.unwrap();
    assert!(app.list_trash(spec.id).await.unwrap().is_empty());
}

#[tokio::test]
async fn tar_gz_extract_and_fetch_file_job() {
    let env = Env::new();
    let app = env.app();
    let spec = create(&app).await;
    let server = env.server(&spec);

    // Tạo .tar.gz hợp lệ kèm tiền tố ./ (giống `tar -czf ... .`)
    let buf = Vec::new();
    let gz = GzEncoder::new(buf, Compression::default());
    let mut tar = tar::Builder::new(gz);
    let mut dir_hdr = tar::Header::new_gnu();
    dir_hdr.set_entry_type(tar::EntryType::Directory);
    dir_hdr.set_size(0);
    dir_hdr.set_mode(0o755);
    dir_hdr.set_cksum();
    tar.append_data(&mut dir_hdr, "./", std::io::empty())
        .unwrap();
    let data = b"hello tar";
    let mut header = tar::Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    tar.append_data(&mut header, "./pack/readme.txt", &data[..])
        .unwrap();
    let gz_done = tar.into_inner().unwrap();
    std::fs::write(server.join("bundle.tar.gz"), gz_done.finish().unwrap()).unwrap();

    let job_id = app
        .extract_files(spec.id, "bundle.tar.gz", "out")
        .await
        .unwrap();
    wait_job_ok(&app, job_id).await;
    assert_eq!(
        std::fs::read_to_string(server.join("out/pack/readme.txt")).unwrap(),
        "hello tar"
    );

    // Tải từ URL giả lập bằng wiremock
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/dl/plugin.jar"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"jar-bytes"))
        .mount(&mock)
        .await;
    let url = format!("{}/dl/plugin.jar", mock.uri());
    let fetch_id = app.fetch_file(spec.id, "", &url, None, 1024).await.unwrap();
    wait_job_ok(&app, fetch_id).await;
    assert_eq!(
        std::fs::read(server.join("plugin.jar")).unwrap(),
        b"jar-bytes"
    );
    assert_eq!(
        app.fetch_file(spec.id, "", "ftp://example.com/a", None, 1024)
            .await
            .unwrap_err()
            .code,
        ErrorCode::InvalidInput
    );
}

async fn wait_job_ok(app: &mh_app::App, id: mh_app::JobId) {
    for _ in 0..500 {
        let snap = app.jobs().get(id).unwrap();
        if snap.state != JobState::Running {
            assert_eq!(snap.state, JobState::Succeeded, "{snap:?}");
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert_eq!(
        app.jobs().get(id).unwrap().state,
        JobState::Succeeded,
        "Job không xong"
    );
}
