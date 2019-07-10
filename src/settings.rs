//! Logic for dealing with settings files.

use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::BufReader;

use serde::Deserialize;
use xdg::BaseDirectories;

use crate::error::{ConfigFileNotFoundError, PathToUnicodeError};

#[derive(Clone, Debug, Deserialize)]
pub struct Notifier {
    pub bus: String,
    pub bus_name: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Rule {
    pub bus: String,
    pub expression_type: String,
    pub expression: String,
    pub notifiers: Vec<String>,
    pub active_states: Vec<String>,
}

/// A deserialized copy of a configuration file.
///
/// Here's an example of what a configuration file may look like:
///
///     use killjoy::settings::Settings;
///
///     let settings_str = r###"
///         {
///             "notifiers": {
///                 "desktop popup": {
///                     "bus": "session",
///                     "bus_name": "org.freedesktop.Notifications"
///                 }
///             },
///             "rules": [{
///                     "active_states": ["failed"],
///                     "bus": "session",
///                     "expression": "syncthing.service",
///                     "expression_type": "unit name",
///                     "notifiers": ["desktop popup"]
///             }],
///             "version": 1
///         }
///     "###;
///     let settings: Settings = serde_json::from_str(settings_str).unwrap();
///
///     let target = "org.freedesktop.Notifications";
///     let actual = &settings.notifiers.get("desktop popup").unwrap().bus_name;
///     assert_eq!(target, actual);
///
/// Beware that instances of `Settings` may have semantically invalid values. For example, the
/// notifier's `bus_name` shown in the example above might not be a valid D-Bus bus name.
#[derive(Clone, Debug, Deserialize)]
pub struct Settings {
    pub notifiers: HashMap<String, Notifier>,
    pub rules: Vec<Rule>,
    pub version: usize,
}

/// Search several paths for a settings file, in order of preference.
///
/// If a file is found, return its path. Otherwise, return an error describing why.
pub fn get_load_path() -> Result<String, Box<dyn Error>> {
    let dirs = match BaseDirectories::with_prefix("killjoy") {
        Ok(dirs) => dirs,
        Err(err) => return Err(Box::new(err)),
    };
    let path_buf = match dirs.find_config_file("settings.json") {
        Some(path_buf) => path_buf,
        None => return Err(Box::new(ConfigFileNotFoundError)),
    };
    let path = match path_buf.to_str() {
        Some(path) => path.to_string(),
        None => return Err(Box::new(PathToUnicodeError)),
    };
    Ok(path)
}

/// Read the configuration file into a [Settings](struct.Settings.html) object.
pub fn load() -> Result<Settings, Box<dyn Error>> {
    let load_path = get_load_path()?;
    let handle = match File::open(load_path) {
        Ok(handle) => handle,
        Err(err) => return Err(Box::new(err)),
    };
    let reader = BufReader::new(handle);
    let settings: Settings = match serde_json::from_reader(reader) {
        Ok(settings) => settings,
        Err(err) => return Err(Box::new(err)),
    };
    Ok(settings)
}
