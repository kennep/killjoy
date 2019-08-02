// Tests for the CLI.

use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output};

use assert_cmd::prelude::{CommandCargoExt, OutputAssertExt};
use tempfile::{NamedTempFile, TempDir};

// Call `killjoy settings validate` and expect failure.
#[test]
fn test_settings_validate_failure() {
    let (config_dir, mut settings_file) = create_config_skeleton();
    write_invalid_settings(&mut settings_file);
    let config_dir_str = config_dir
        .path()
        .to_str()
        .expect("Failed to convert file path to unicode string.");
    let out: Output = Command::cargo_bin("killjoy")
        .expect("Failed to find crate-local executable.")
        .env("XDG_CONFIG_HOME", config_dir_str)
        .env("XDG_CONFIG_DIRS", config_dir_str)
        .args(&["settings", "validate"])
        .output()
        .expect("Failed to run killjoy.");
    out.assert().failure();
}

// Call `killjoy settings validate` and expect success.
#[test]
fn test_settings_validate_success() {
    let (config_dir, mut settings_file) = create_config_skeleton();
    write_valid_settings(&mut settings_file);
    let config_dir_str = config_dir
        .path()
        .to_str()
        .expect("Failed to convert file path to unicode string.");
    let out: Output = Command::cargo_bin("killjoy")
        .expect("Failed to find crate-local executable.")
        .env("XDG_CONFIG_HOME", config_dir_str)
        .env("XDG_CONFIG_DIRS", config_dir_str)
        .args(&["settings", "validate"])
        .output()
        .expect("Failed to run killjoy.");
    out.assert().success();
}

// Call `killjoy settings validate $path` and expect failure.
#[test]
fn test_settings_validate_path_failure() {
    let mut settings_file = NamedTempFile::new().expect("Failed to create a named temporary file.");
    write_invalid_settings(&mut settings_file);

    let out: Output = Command::cargo_bin("killjoy")
        .expect("Failed to find crate-local executable.")
        .args(&[
            "settings",
            "validate",
            settings_file
                .path()
                .to_str()
                .expect("Failed to convert file path to unicode string."),
        ])
        .output()
        .expect("Failed to run killjoy.");
    out.assert().failure();
}

// Call `killjoy settings validate $path` and expect success.
#[test]
fn test_settings_validate_path_success() {
    let mut settings_file = NamedTempFile::new().expect("Failed to create a named temporary file.");
    write_valid_settings(&mut settings_file);

    let out: Output = Command::cargo_bin("killjoy")
        .expect("Failed to find crate-local executable.")
        .args(&[
            "settings",
            "validate",
            settings_file
                .path()
                .to_str()
                .expect("Failed to convert file path to unicode string."),
        ])
        .output()
        .expect("Failed to run killjoy.");
    out.assert().success();
}

// Create a temporary directory containing "killjoy/settings.json".
//
// The settings file is empty. The returned tuple is of the form `(temp_dir, settings_file)`.
fn create_config_skeleton() -> (TempDir, File) {
    let xdg_config_home = TempDir::new().expect("Failed to create xdg_config_home.");

    let settings_dir = xdg_config_home.path().join(Path::new("killjoy"));
    fs::create_dir(&settings_dir).expect("Failed to create settings_dir.");

    let settings_file = File::create(settings_dir.join(Path::new("settings.json")))
        .expect("Failed to create settings_file.");

    (xdg_config_home, settings_file)
}

fn write_invalid_settings<T: Write>(handle: &mut T) {
    let settings_str = "{}";
    handle
        .write_all(settings_str.as_bytes())
        .expect("Failed to populate settings file.");
}

fn write_valid_settings<T: Write>(handle: &mut T) {
    let settings_str = r###"
    {
        "version": 1,
        "rules": [
            {
                "active_states": ["failed"],
                "bus_type": "session",
                "expression": "syncthing.service",
                "expression_type": "unit name",
                "notifiers": ["desktop popup"]
            }
        ],
        "notifiers": {
            "desktop popup": {
                "bus_type": "session",
                "bus_name": "name.jerebear.KilljoyNotifierNotification1"
            }
        }
    }
    "###;
    handle
        .write_all(settings_str.as_bytes())
        .expect("Failed to populate settings file.");
}
