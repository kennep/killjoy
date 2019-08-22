// Custom exceptions.

use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::io::Error as IOError;

use regex::Error as RegexError;
use serde_json::error::Error as SerdeJsonError;

// An error used when working with a settings file.
#[derive(Debug)]
pub enum SettingsFileError {
    DeserializationFailed(SerdeJsonError),
    FileNotFound(String),
    FileNotReadable(IOError),
    InvalidActiveState(String),
    InvalidBusName(String),
    InvalidBusType(String),
    InvalidExpressionType(String),
    InvalidNotifier(String),
    InvalidRegex(RegexError),
}

impl Display for SettingsFileError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match self {
            SettingsFileError::DeserializationFailed(err) => {
                write!(f, "Failed to deserialize the settings file: {}", err,)
            }
            SettingsFileError::FileNotFound(path) => write!(
                f,
                "Failed to find a configuration file in $XDG_CONFIG_HOME or $XDG_CONFIG_DIRS with path {}",
                path
            ),
            SettingsFileError::FileNotReadable(err) => {
                write!(f, "Failed to read settings file: {}", err)
            }
            SettingsFileError::InvalidActiveState(as_str) => {
                write!(f, "Found invalid active state: {}", as_str)
            }
            SettingsFileError::InvalidBusName(bn_str) => {
                write!(f, "Found invalid bus name: {}", bn_str)
            }
            SettingsFileError::InvalidBusType(bt_str) => {
                write!(f, "Found invalid bus type: {}", bt_str)
            }
            SettingsFileError::InvalidExpressionType(et_str) => {
                write!(f, "Found invalid expression type: {}", et_str)
            }
            SettingsFileError::InvalidRegex(err) => {
                write!(f, "Found invalid regular expression: {}", err)
            }
            SettingsFileError::InvalidNotifier(notifier) => {
                write!(f, "Rule references non-existent notifier: {}", notifier)
            }
        }
    }
}

impl Error for SettingsFileError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            SettingsFileError::DeserializationFailed(err) => Some(err),
            SettingsFileError::FileNotReadable(err) => Some(err),
            SettingsFileError::InvalidRegex(err) => Some(err),
            _ => None,
        }
    }
}

// An error indicating that parsing an object to an ActiveState failed.
#[derive(Debug)]
pub struct ParseAsActiveStateError {
    msg: String,
}

impl ParseAsActiveStateError {
    pub fn new(msg: String) -> Self {
        Self { msg }
    }
}

impl Display for ParseAsActiveStateError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "{}", &self.msg[..])
    }
}

impl Error for ParseAsActiveStateError {}

// Like dbus::Error, but implements Send.
//
// This isn't a great way to handle errors. Doing this means that we don't know what went wrong,
// except by inspecting the `msg` field. It would be nicer if e.g. we had an enum with variants like
// ManagerUnitNew, ManagerUnitRemoved, etc, so that the type system could communicate what went
// wrong.
#[derive(Debug)]
pub struct MyDBusError {
    msg: String,
}

impl MyDBusError {
    pub fn new(msg: String) -> Self {
        Self { msg }
    }
}

impl Display for MyDBusError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "{}", self.msg)
    }
}

impl Error for MyDBusError {}
