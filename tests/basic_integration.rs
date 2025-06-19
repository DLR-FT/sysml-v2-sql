const BIN: &str = "sysml-v2-sql";

#[test]
fn init_db() {
    let db_file = tempfile::NamedTempFile::new().unwrap();

    let output = test_bin::get_test_bin(BIN)
        .arg(db_file.path())
        .arg("init-db")
        .output()
        .expect("Failed to start {BIN}");

    assert!(output.status.success());

    db_file.close().unwrap();
}

#[test]
fn import() {
    let db_file = tempfile::NamedTempFile::new().unwrap();

    let output = test_bin::get_test_bin(BIN)
        .arg(db_file.path())
        .arg("init-db")
        .output()
        .expect("Failed to start {BIN}");

    assert!(output.status.success());

    let output = test_bin::get_test_bin(BIN)
        .arg(db_file.path())
        .arg("import-json")
        .arg("tests/example-dump.json")
        .output()
        .expect("Failed to start {BIN}");

    assert!(output.status.success());

    db_file.close().unwrap();
}

#[test]
fn import_twice() {
    let db_file = tempfile::NamedTempFile::new().unwrap();

    let output = test_bin::get_test_bin(BIN)
        .arg(db_file.path())
        .arg("init-db")
        .output()
        .expect("Failed to start {BIN}");

    assert!(output.status.success());

    for _ in 0..2 {
        let output = test_bin::get_test_bin(BIN)
            .arg(db_file.path())
            .arg("import-json")
            .arg("tests/example-dump.json")
            .output()
            .expect("Failed to start {BIN}");

        assert!(output.status.success());
    }

    db_file.close().unwrap();
}

#[test]
#[should_panic(expected = "assertion failed: output.status.success()")]
fn import_missing_schema() {
    let db_file = tempfile::NamedTempFile::new().unwrap();

    let output = test_bin::get_test_bin(BIN)
        .arg(db_file.path())
        .arg("import-json")
        .arg("tests/example-dump.json")
        .output()
        .expect("Failed to start {BIN}");

    assert!(output.status.success());

    db_file.close().unwrap();
}
