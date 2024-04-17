use std::{env, fs, path::Path};

const FIXTURES_DIR: &str = "tests/fixtures/";
const MOCK_BIN: &str = "target/mock_bin";

/// "Monkey-patch" the crontab executable.
///
/// The `fixtures` directory contains shell scripts that mimic the
/// behaviour of `crontab` in different scenarios.
///
/// This function takes the name of one of such mock scripts as input,
/// and plays with the `PATH` environment variable to make this script
/// be executed instead of the real `crontab` executable.
///
/// This enables us to test virtually anything, without touching the
/// real crontab.
pub fn mock_crontab(file: &str) {
    let fixtures_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURES_DIR);
    let bin_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(MOCK_BIN);

    let fixture = fixtures_dir.join(file).with_extension("sh");
    let test_mock = bin_dir.join("crontab");

    assert!(
        fs::create_dir_all(&bin_dir).is_ok(),
        "Error creating mock bin directory: '{}'.",
        bin_dir.display()
    );

    assert!(
        fs::copy(&fixture, test_mock).is_ok(),
        "Error setting up mock crontab: '{}'.",
        fixture.display()
    );

    env::set_var("PATH", format!("{}:/bin:/usr/bin/", bin_dir.display()));
}

/// "Monkey-patch" the crontab executable.
///
/// This works exactly like [`mock_crontab()`], but in this case it sets
/// up a fake shell.
pub fn mock_shell(file: &str) {
    let fixtures_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURES_DIR);
    let bin_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(MOCK_BIN);

    let fixture = fixtures_dir.join(file).with_extension("sh");
    let test_mock = bin_dir.join("mock_shell");

    assert!(
        fs::create_dir_all(&bin_dir).is_ok(),
        "Error creating mock bin directory: '{}'.",
        bin_dir.display()
    );

    assert!(
        fs::copy(&fixture, test_mock).is_ok(),
        "Error setting up mock shell: '{}'.",
        fixture.display()
    );

    env::set_var("PATH", format!("{}:/bin:/usr/bin/", bin_dir.display()));
}

/// Read output file created by a mock executable (crontab or shell).
pub fn read_output_file(file: &str) -> String {
    // Scripts create output files in the same directory as they're in
    // (i.e., in `target/mock_bin/`).
    let bin_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(MOCK_BIN);

    fs::read_to_string(bin_dir.join(file).with_extension("txt"))
        .expect("if file doesn't exist, the test failed")
}
