// Tests for the CLI.

use std::io::Write;
use std::process::{Command, Output};

use assert_cmd::prelude::{CommandCargoExt, OutputAssertExt};
use tempfile::NamedTempFile;

// Call `killjoy settings validate $path` and expect failure.
#[test]
fn test_settings_validate_path_failure() {
    let mut settings_file = NamedTempFile::new().expect("Failed to create a named temporary file.");
    settings_file
        .write_all("{}".as_bytes())
        .expect("Failed to populate temporary settings file.");

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
    let mut settings_file = NamedTempFile::new().expect("Failed to create a named temporary file.");
    settings_file
        .write_all(settings_str.as_bytes())
        .expect("Failed to populate temporary settings file.");

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
