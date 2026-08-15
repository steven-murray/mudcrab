use assert_cmd::Command;
use tempfile::tempdir;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn download_uses_nexus_api_descriptor_via_cli() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path(
            "/v1/games/skyrimspecialedition/mods/1234/files/5678/download_link.json",
        ))
        .and(header("apikey", "test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            format!(r#"[{{"uri":"{}/files/archive.bin"}}]"#, server.uri()),
            "application/json",
        ))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/files/archive.bin"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"nexus-cli-payload".to_vec()))
        .mount(&server)
        .await;

    let dir = tempdir().expect("temp dir should be created");
    let modlist = dir.path().join("modlist.toml");
    let compiled = dir.path().join("compiled.json");
    let plan = dir.path().join("plan.json");
    let cache = dir.path().join("cache");
    let game_dir = dir.path().join("game");
    std::fs::create_dir_all(&game_dir).expect("game dir should be created");

    let source = "name = \"Nexus CLI Test\"\n\n[modlist.core]\ndependencies = []\n\n[[modlist.core.archives]]\npath = \"nexus:skyrimspecialedition/1234/5678\"\ndownload_handler = \"nexus\"\n";
    std::fs::write(&modlist, source).expect("fixture modlist should be written");

    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .env("GAME_DIR", &game_dir)
        .arg("compile")
        .arg(&modlist)
        .arg("--output")
        .arg(&compiled)
        .assert()
        .success();

    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .env("GAME_DIR", &game_dir)
        .arg("query")
        .arg(&compiled)
        .arg("--output")
        .arg(&plan)
        .arg("--headless")
        .assert()
        .success();

    Command::cargo_bin("mudcrab")
        .expect("binary should build")
        .env("GAME_DIR", &game_dir)
        .arg("download")
        .arg(&plan)
        .arg("--cache")
        .arg(&cache)
        .env("NEXUS_API_KEY", "test-key")
        .env("NEXUS_API_BASE", format!("{}/v1", server.uri()))
        .assert()
        .success();

    let files = std::fs::read_dir(&cache)
        .expect("cache should exist")
        .filter_map(Result::ok)
        .collect::<Vec<_>>();

    let archive = files
        .iter()
        .find(|entry| entry.path().extension().and_then(|ext| ext.to_str()) != Some("orig-name"))
        .expect("archive payload should exist");
    let metadata = files
        .iter()
        .find(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("orig-name"))
        .expect("archive metadata sidecar should exist");

    let payload = std::fs::read(archive.path()).expect("downloaded file should be readable");
    assert_eq!(payload, b"nexus-cli-payload");

    let original_name = std::fs::read_to_string(metadata.path())
        .expect("original filename metadata should be readable");
    assert_eq!(original_name, "archive.bin");
}
