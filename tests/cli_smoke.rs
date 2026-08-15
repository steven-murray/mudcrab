use assert_cmd::Command;

#[test]
fn help_lists_subcommands() {
    let mut cmd = Command::cargo_bin("mudcrab").expect("binary should build");
    cmd.arg("--help");

    cmd.assert()
        .success()
        .stdout(predicates::str::contains("compile"))
        .stdout(predicates::str::contains("query"))
        .stdout(predicates::str::contains("download"))
        .stdout(predicates::str::contains("install"));
}
