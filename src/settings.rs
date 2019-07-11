//! Logic for dealing with settings files.

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fs::File;
use std::io::{BufReader, Read};

use dbus::BusType;
use serde::Deserialize;
use xdg::BaseDirectories;

use crate::bus;
use crate::error::{ConfigFileNotFoundError, PathToUnicodeError};
use crate::unit::ActiveState;

/// The types of expressions that a user may use to match unit names.
#[derive(Clone, Debug)]
pub enum ExpressionType {
    Regex,
    UnitName,
    UnitType,
}

/// A D-Bus service that may be contacted when an event of interest happens.
///
/// When an event of interest occurs, killjoy will connect to `bus_type` and send a message to
/// `bus_name`.
#[derive(Clone, Debug)]
pub struct Notifier {
    pub bus_name: String,
    pub bus_type: BusType,
}

/// Units to watch, and notifiers to contact when any of them enter a state of interest.
///
/// Upon startup, killjoy will connect to `bus_type`. It will watch all units whose name matches
/// `expression` and `expression_type`. Whenever one of those units' ActiveState property
/// transitions to one of the `active_states` it will contact `notifiers`.
#[derive(Clone, Debug)]
pub struct Rule {
    pub active_states: HashSet<ActiveState>,
    pub bus_type: BusType,
    pub expression: String,
    pub expression_type: ExpressionType,
    pub notifiers: Vec<String>,
}

/// Read the configuration file into a [Settings](struct.Settings.html) object.
///
/// An error may be returned for one of two broad categories of reasons:
///
/// *   The file couldn't be opened. Maybe a settings file couldn't be found; or maybe a settings
///     file was found but could not be opened.
/// *   The file contained invalid contents. See [Settings::new](struct.Settings.html#method.new).
pub fn load() -> Result<Settings, Box<dyn Error>> {
    let load_path = get_load_path()?;
    let handle = match File::open(load_path) {
        Ok(handle) => handle,
        Err(err) => return Err(Box::new(err)),
    };
    let reader = BufReader::new(handle);
    let settings = Settings::new(reader)?;
    Ok(settings)
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

/// A deserialized copy of a configuration file.
///
/// Here's an example of what a configuration file may look like:
///
///     use dbus::BusType;
///     use killjoy::settings::Settings;
///
///     let settings_bytes = r###"
///         {
///             "notifiers": {
///                 "desktop popup": {
///                     "bus_name": "org.freedesktop.Notifications",
///                     "bus_type": "session"
///                 }
///             },
///             "rules": [{
///                     "active_states": ["failed"],
///                     "bus_type": "session",
///                     "expression": "syncthing.service",
///                     "expression_type": "unit name",
///                     "notifiers": ["desktop popup"]
///             }],
///             "version": 1
///         }
///     "###.as_bytes();
///     let settings: Settings = Settings::new(settings_bytes).unwrap();
///
///     let target = BusType::Session;
///     let actual = settings.notifiers.get("desktop popup").unwrap().bus_type;
///     assert_eq!(target, actual);
///
/// Beware that `Settings` instances may have semantically invalid values. For example, the
/// notifier's `bus_name` shown in the example above might not be a valid D-Bus bus name.
#[derive(Clone, Debug)]
pub struct Settings {
    pub notifiers: HashMap<String, Notifier>,
    pub rules: Vec<Rule>,
}

impl Settings {
    /// Create a new settings object.
    ///
    /// For a usage example, see [Settings](struct.Settings.html). An error may be returned for one
    /// of two broad categories of reasons:
    ///
    /// *   Deserialization of the `reader` failed. Maybe the reader yielded non-unicode bytes;
    ///     maybe the bytes were valid unicode but not valid JSON; maybe the unicode was valid JSON
    ///     but didn't match the settings file schema; or so on.
    /// *   The settings object contained semantically invalid data. Maybe a `"bus_type"` key was
    ///     set to a value such as `"foo"`, or so on.
    pub fn new<T: Read>(reader: T) -> Result<Self, Box<dyn Error>> {
        let serde_settings: SerdeSettings = serde_json::from_reader(reader)?;
        let settings: Self = serde_settings.to_settings()?;
        Ok(settings)
    }
}

// See SerdeSettings.
#[derive(Clone, Deserialize)]
struct SerdeNotifier {
    bus_name: String,
    bus_type: String,
}

impl SerdeNotifier {
    // See SerdeSettings.
    fn to_notifier(&self) -> Result<Notifier, Box<dyn Error>> {
        Ok(Notifier {
            bus_name: self.bus_name.to_owned(),
            bus_type: decode_bus_type_str(&self.bus_type)?,
        })
    }
}

// See SerdeSettings.
#[derive(Clone, Deserialize)]
struct SerdeRule {
    active_states: Vec<String>,
    bus_type: String,
    expression: String,
    expression_type: String,
    notifiers: Vec<String>,
}

impl SerdeRule {
    // See SerdeSettings.
    fn to_rule(&self) -> Result<Rule, Box<dyn Error>> {
        let mut active_states: HashSet<ActiveState> = HashSet::new();
        for active_state_string in &self.active_states {
            let active_state = bus::decode_active_state_str(&active_state_string[..])?;
            active_states.insert(active_state);
        }

        let bus_type = decode_bus_type_str(&self.bus_type)?;
        let expression = self.expression.to_owned();
        let expression_type = decode_expression_type_str(&self.expression_type[..])?;
        let notifiers = self.notifiers.to_owned();
        Ok(Rule {
            active_states,
            bus_type,
            expression,
            expression_type,
            notifiers,
        })
    }
}

// Like a `Settings`, but fields are simple types instead of domain-specific types.
//
// The `SerdeSettings` object is composed of types from the standard library, such as strings and
// vectors. This makes it easy to deserialize with serde, but the resulting object may contain
// values that are syntactically correct but semantically incorrect. For example,
// `settings.notifiers[i].bus_name` is a `String`, and it may be impossible to cast this string to a
// `dbus::BusName` object.
//
// This makes `SerdeSettings` a bad choice for use throughout the rest of the application. If this
// was done, killjoy could encounter semantic errors late in runtime. For example, killjoy might
// notice an event of interest, decide to notify a notifier, and *then* discover that a `bus_name`
// contains a malformed address.
//
// Calling SerdeSettings::to_settings forces values to be cast to more appropriate types. Although
// type-casting doesn't guarantee that a settings object contains valid values, it gets closer to
// the ideal.
#[derive(Clone, Deserialize)]
struct SerdeSettings {
    notifiers: HashMap<String, SerdeNotifier>,
    rules: Vec<SerdeRule>,
}

impl SerdeSettings {
    // See SerdeSettings.
    fn to_settings(&self) -> Result<Settings, Box<dyn Error>> {
        // Use :or loops instead of chaining method calls on iter() so that the ? operator may be
        // used.
        let mut notifiers: HashMap<String, Notifier> = HashMap::new();
        for (key, val) in self.notifiers.iter() {
            let new_key = key.to_owned();
            let new_val = val.to_notifier()?;
            notifiers.insert(new_key, new_val);
        }

        let mut rules: Vec<Rule> = Vec::new();
        for rule in self.rules.iter() {
            let new_rule = rule.to_rule()?;
            rules.push(new_rule);
        }

        Ok(Settings { notifiers, rules })
    }
}

pub fn decode_bus_type_str(bus_type_str: &str) -> Result<BusType, String> {
    match bus_type_str {
        "session" => Ok(BusType::Session),
        "starter" => Ok(BusType::Starter),
        "system" => Ok(BusType::System),
        _ => Err(format!(
            "Failed to decode bus type string: {}",
            bus_type_str
        )),
    }
}

pub fn decode_expression_type_str(expression_type_str: &str) -> Result<ExpressionType, String> {
    match expression_type_str {
        "regex" => Ok(ExpressionType::Regex),
        "unit name" => Ok(ExpressionType::UnitName),
        "unit type" => Ok(ExpressionType::UnitType),
        _ => Err(format!(
            "Failed to decode expression type string: {}",
            expression_type_str
        )),
    }
}

pub fn encode_bus_type(bus_type: &BusType) -> String {
    match bus_type {
        BusType::Session => "session".to_string(),
        BusType::Starter => "starter".to_string(),
        BusType::System => "system".to_string(),
    }
}
