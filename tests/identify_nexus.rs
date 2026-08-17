//! `identify` recovers a Nexus descriptor for an archive downloaded by hand.
//!
//! Nexus indexes every file it serves by MD5, which is how Mod Organizer 2
//! recognises a file you fetched yourself. Two archives in Part 9 arrived with
//! no `.meta` sidecar and had to be identified through MO2's UI before they
//! could become modlist entries; this is the same API call.

use assert_cmd::Command;
use tempfile::tempdir;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// md5("abc"), which is what the fixture archive hashes to.
const ABC_MD5: &str = "900150983cd24fb0d6963f7d28e17f72";

fn body() -> String {
    r#"[{
        "mod": {"mod_id": 24078, "name": "EVE HGEC", "domain_name": "oblivion"},
        "file_details": {"file_id": 42364, "name": "EVE BAIN", "version": "1.3"}
    }]"#
    .to_string()
}

#[tokio::test]
async fn identify_prints_a_pasteable_descriptor() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/v1/games/oblivion/mods/md5_search/{ABC_MD5}.json"
        )))
        .and(header("apikey", "test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body(), "application/json"))
        .mount(&server)
        .await;

    let dir = tempdir().expect("temp dir");
    let archive = dir.path().join("EVE BAIN-24078.7z");
    std::fs::write(&archive, b"abc").expect("fixture archive");

    let assert = Command::cargo_bin("mudcrab")
        .expect("binary builds")
        .env("NEXUS_API_KEY", "test-key")
        .args(["identify", archive.to_str().unwrap()])
        .args(["--api-base", &format!("{}/v1", server.uri())])
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf-8");
    assert!(
        stdout.contains(r#"path = "nexus:oblivion/24078/42364""#),
        "unexpected output:\n{stdout}"
    );
    assert!(
        stdout.contains(r#"file_name = "EVE BAIN-24078.7z""#),
        "the descriptor must carry the on-disk filename:\n{stdout}"
    );
    assert!(stdout.contains("# nexus version 1.3"), "{stdout}");
}

#[tokio::test]
async fn identify_can_write_the_meta_sidecar_mo2_would_have() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/v1/games/oblivion/mods/md5_search/{ABC_MD5}.json"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body(), "application/json"))
        .mount(&server)
        .await;

    let dir = tempdir().expect("temp dir");
    let archive = dir.path().join("thing.7z");
    std::fs::write(&archive, b"abc").expect("fixture archive");

    Command::cargo_bin("mudcrab")
        .expect("binary builds")
        .env("NEXUS_API_KEY", "test-key")
        .args(["identify", archive.to_str().unwrap(), "--write-meta"])
        .args(["--api-base", &format!("{}/v1", server.uri())])
        .assert()
        .success();

    let sidecar = std::fs::read_to_string(dir.path().join("thing.7z.meta")).expect("sidecar");
    assert!(sidecar.contains("modID=24078"), "{sidecar}");
    assert!(sidecar.contains("fileID=42364"), "{sidecar}");
    // The same keys `add --from-oracle` reads back out of a real MO2 meta.ini.
    assert!(sidecar.contains("version=1.3"), "{sidecar}");
}

#[tokio::test]
async fn an_archive_nexus_does_not_know_is_reported_not_guessed() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let dir = tempdir().expect("temp dir");
    let archive = dir.path().join("repacked.7z");
    std::fs::write(&archive, b"abc").expect("fixture archive");

    let assert = Command::cargo_bin("mudcrab")
        .expect("binary builds")
        .env("NEXUS_API_KEY", "test-key")
        .args(["identify", archive.to_str().unwrap()])
        .args(["--api-base", &format!("{}/v1", server.uri())])
        .assert()
        .failure();

    let stderr = String::from_utf8(assert.get_output().stderr.clone()).expect("utf-8");
    assert!(stderr.contains("does not recognise"), "{stderr}");
    assert!(
        stderr.contains("repacked after download"),
        "the likely cause is worth naming:\n{stderr}"
    );
}
