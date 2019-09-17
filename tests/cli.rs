// Tests for the CLI.

use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use assert_cmd::cargo;
use assert_cmd::prelude::{CommandCargoExt, OutputAssertExt};
use tempfile::{NamedTempFile, TempDir};

// Call `killjoy settings load-path` and expect failure.
#[test]
fn test_settings_load_path_failure() {
    let (config_dir, settings_dir, _) = create_skeleton_config();
    let old_settings_file_path = settings_dir.join("settings.json");
    let new_settings_file_path = settings_dir.join("foo.json");
    std::fs::rename(&old_settings_file_path, &new_settings_file_path)
        .expect("Failed to rename settings file.");
    let config_dir_str = path_to_str(&config_dir.path());
    Command::cargo_bin("killjoy")
        .expect("Failed to find crate-local executable.")
        .env("XDG_CONFIG_HOME", config_dir_str)
        .env("XDG_CONFIG_DIRS", config_dir_str)
        .args(&["settings", "load-path"])
        .output()
        .expect("Failed to run killjoy.")
        .assert()
        .code(1);
}

// Call `killjoy settings load-path` and expect success.
#[test]
fn test_settings_load_path_success() {
    let (config_dir, _, _) = create_skeleton_config();
    let config_dir_str = path_to_str(&config_dir.path());
    Command::cargo_bin("killjoy")
        .expect("Failed to find crate-local executable.")
        .env("XDG_CONFIG_HOME", config_dir_str)
        .env("XDG_CONFIG_DIRS", config_dir_str)
        .args(&["settings", "load-path"])
        .output()
        .expect("Failed to run killjoy.")
        .assert()
        .code(0);
}

// Call `killjoy settings validate` and expect failure due to the settings file being invalid.
#[test]
fn test_settings_validate_failure_v1() {
    let (config_dir, _, _) = create_skeleton_config();
    let config_dir_str = path_to_str(&config_dir.path());
    Command::cargo_bin("killjoy")
        .expect("Failed to find crate-local executable.")
        .env("XDG_CONFIG_HOME", config_dir_str)
        .env("XDG_CONFIG_DIRS", config_dir_str)
        .args(&["settings", "validate"])
        .output()
        .expect("Failed to run killjoy.")
        .assert()
        .code(1);
}

// Call `killjoy settings validate` and expect failure due to the settings file being absent.
#[test]
fn test_settings_validate_failure_v2() {
    let (config_dir, settings_dir, _) = create_skeleton_config();
    let old_settings_file_path = settings_dir.join("settings.json");
    let new_settings_file_path = settings_dir.join("foo.json");
    std::fs::rename(&old_settings_file_path, &new_settings_file_path)
        .expect("Failed to rename settings file.");
    let config_dir_str = path_to_str(&config_dir.path());
    Command::cargo_bin("killjoy")
        .expect("Failed to find crate-local executable.")
        .env("XDG_CONFIG_HOME", config_dir_str)
        .env("XDG_CONFIG_DIRS", config_dir_str)
        .args(&["settings", "validate"])
        .output()
        .expect("Failed to run killjoy.")
        .assert()
        .code(1);
}

// Call `killjoy settings validate` and expect failure due to the settings file being unreadable.
#[test]
fn test_settings_validate_failure_v3() {
    let (config_dir, settings_dir, _) = create_skeleton_config();
    let config_dir_str = path_to_str(&config_dir.path());
    // PermissionsExt::set_mode has no effect. Maybe it's a bug in Rust?
    Command::new("chmod")
        .env("XDG_CONFIG_HOME", config_dir_str)
        .env("XDG_CONFIG_DIRS", config_dir_str)
        .args(&[
            "000",
            Path::new(&settings_dir)
                .join("settings.json")
                .to_str()
                .expect("Failed to cast file path to UTF-8."),
        ])
        .output()
        .expect("Failed to run chmod.")
        .assert()
        .code(0);
    Command::cargo_bin("killjoy")
        .expect("Failed to find crate-local executable.")
        .env("XDG_CONFIG_HOME", config_dir_str)
        .env("XDG_CONFIG_DIRS", config_dir_str)
        .args(&["settings", "validate"])
        .output()
        .expect("Failed to run killjoy.")
        .assert()
        .code(1);
}

// Call `killjoy settings validate` and expect success.
#[test]
fn test_settings_validate_success() {
    let (config_dir, _, mut settings_file) = create_skeleton_config();
    write_session_settings(&mut settings_file);
    let config_dir_str = path_to_str(&config_dir.path());
    Command::cargo_bin("killjoy")
        .expect("Failed to find crate-local executable.")
        .env("XDG_CONFIG_HOME", config_dir_str)
        .env("XDG_CONFIG_DIRS", config_dir_str)
        .args(&["settings", "validate"])
        .output()
        .expect("Failed to run killjoy.")
        .assert()
        .code(0);
}

// Call `killjoy settings validate $path` and expect failure.
#[test]
fn test_settings_validate_path_failure() {
    let settings_file = NamedTempFile::new().expect("Failed to create a named temporary file.");

    Command::cargo_bin("killjoy")
        .expect("Failed to find crate-local executable.")
        .args(&["settings", "validate", path_to_str(&settings_file.path())])
        .output()
        .expect("Failed to run killjoy.")
        .assert()
        .code(1);
}

// Call `killjoy settings validate $path` and expect success.
#[test]
fn test_settings_validate_path_success() {
    let mut settings_file = NamedTempFile::new().expect("Failed to create a named temporary file.");
    write_session_settings(&mut settings_file);
    Command::cargo_bin("killjoy")
        .expect("Failed to find crate-local executable.")
        .args(&["settings", "validate", path_to_str(&settings_file.path())])
        .output()
        .expect("Failed to run killjoy.")
        .assert()
        .code(0);
}

// Prevent killjoy's worker threads from contacting systemd.
//
// This test makes that happen by starting a temporary stand-alone session D-Bus instance, where
// killjoy is the only peer.
#[test]
fn test_no_systemd() {
    let (config_dir, _, mut settings_file) = create_skeleton_config();
    write_session_settings(&mut settings_file);

    // Exit code is 101 for panic.
    let config_dir_str = path_to_str(&config_dir.path());
    Command::new("dbus-run-session")
        .env("XDG_CONFIG_HOME", config_dir_str)
        .env("XDG_CONFIG_DIRS", config_dir_str)
        .args(&["--", path_to_str(cargo::cargo_bin("killjoy").as_path())])
        .output()
        .expect("failed to run executable")
        .assert()
        .code(1);
}

// Call `killjoy`, and let the settings be invalid.
#[test]
fn test_run_settings_failure() {
    let (config_dir, _, _) = create_skeleton_config();
    let config_dir_str = path_to_str(&config_dir.path());
    Command::cargo_bin("killjoy")
        .expect("Failed to find crate-local executable.")
        .env("XDG_CONFIG_HOME", config_dir_str)
        .env("XDG_CONFIG_DIRS", config_dir_str)
        .args(&["--loop-once", "--loop-timeout", "0"])
        .output()
        .expect("Failed to run killjoy")
        .assert()
        .code(1);
}

// Call `killjoy`, and let the settings be valid.
#[test]
fn test_run_settings_success() {
    let (config_dir, _, mut settings_file) = create_skeleton_config();
    write_system_settings(&mut settings_file);
    let config_dir_str = path_to_str(&config_dir.path());
    Command::cargo_bin("killjoy")
        .expect("Failed to find crate-local executable.")
        .env("XDG_CONFIG_HOME", config_dir_str)
        .env("XDG_CONFIG_DIRS", config_dir_str)
        .args(&["--loop-once", "--loop-timeout", "0"])
        .output()
        .expect("Failed to run killjoy")
        .assert()
        .code(0);
}

// Return the string representation of the given path.
//
// Panic if unable create a unicode representation of the path.
fn path_to_str(path: &Path) -> &str {
    path.to_str()
        .expect("Failed to convert path to unicode string.")
}

// Create a temporary directory containing "killjoy/settings.json".
//
// The settings file isempty. The returned tuple is of the form `(temp_dir, settings_dir,
// settings_file)`.
fn create_skeleton_config() -> (TempDir, PathBuf, File) {
    let xdg_config_home = TempDir::new().expect("Failed to create xdg_config_home.");

    let settings_dir = xdg_config_home.path().join(Path::new("killjoy"));
    fs::create_dir(&settings_dir).expect("Failed to create settings_dir.");

    let settings_file = File::create(settings_dir.join(Path::new("settings.json")))
        .expect("Failed to create settings_file.");

    (xdg_config_home, PathBuf::from(settings_dir), settings_file)
}

// Write a valid settings file, where the monitoring rule references the session bus.
fn write_session_settings<T: Write>(handle: &mut T) {
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

// Write a valid settings file, where the monitoring rule references the system bus.
fn write_system_settings<T: Write>(handle: &mut T) {
    let settings_str = r###"
    {
        "version": 1,
        "rules": [
            {
                "active_states": ["failed"],
                "bus_type": "system",
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
