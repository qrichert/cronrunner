use std::env;
use std::fs;
use std::path::Path;

const FIXTURES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/");
const MOCK_BIN_DIR: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/mock_bin/");

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
    let fixtures_dir = Path::new(FIXTURES_DIR);
    let bin_dir = Path::new(MOCK_BIN_DIR);

    let fixture = fixtures_dir.join(file).with_extension("sh");
    let test_mock = bin_dir.join("crontab");

    assert!(
        fs::create_dir_all(bin_dir).is_ok(),
        "Error creating mock bin directory: '{}'.",
        bin_dir.display()
    );

    assert!(
        fs::copy(&fixture, test_mock).is_ok(),
        "Error setting up mock crontab: '{}'.",
        fixture.display()
    );

    unsafe {
        env::set_var("PATH", format!("{}:/bin:/usr/bin/", bin_dir.display()));
    }
}

/// "Monkey-patch" the crontab executable.
///
/// This works exactly like [`mock_crontab()`], but in this case it sets
/// up a fake shell.
pub fn mock_shell(file: &str) {
    let fixtures_dir = Path::new(FIXTURES_DIR);
    let bin_dir = Path::new(MOCK_BIN_DIR);

    let fixture = fixtures_dir.join(file).with_extension("sh");
    let test_mock = bin_dir.join("mock_shell");

    assert!(
        fs::create_dir_all(bin_dir).is_ok(),
        "Error creating mock bin directory: '{}'.",
        bin_dir.display()
    );

    assert!(
        fs::copy(&fixture, test_mock).is_ok(),
        "Error setting up mock shell: '{}'.",
        fixture.display()
    );

    unsafe {
        env::set_var("PATH", format!("{}:/bin:/usr/bin/", bin_dir.display()));
    }
}

/// Read output file created by a mock executable (crontab or shell).
pub fn read_output_file(file: &str) -> String {
    // Scripts create output files in the same directory as they're in
    // (i.e., in `target/tmp/mock_bin/`).
    let bin_dir = Path::new(MOCK_BIN_DIR);

    fs::read_to_string(bin_dir.join(file).with_extension("txt"))
        .expect("if file doesn't exist, the test failed")
}
