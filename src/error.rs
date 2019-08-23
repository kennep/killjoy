// Custom exceptions.

use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::io::Error as IOError;

use crate::unit::ActiveState;

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

// Like dbus::Error, but with more granular semantics, and implements Send.
//
// TODO: Try carrying underlying dbus::Error as property.
#[derive(Debug)]
pub enum DBusError {
    AddMatch(String, String),
    CallOrgFreedesktopDBusPropertiesGetAll(String),
    CallOrgFreedesktopSystemd1ManagerGetUnit(String),
    CallOrgFreedesktopSystemd1ManagerListUnits(String),
    CallOrgFreedesktopSystemd1ManagerSubscribe(String),
    CastOrgFreedesktopSystemd1UnitTimestamp(&'static str),
    CastOrgFreedesktopSystemd1UnitActiveState,
    CastOrgFreedesktopSystemd1UnitId,
    DecodeOrgFreedesktopSystemd1UnitActiveState(ParseAsActiveStateError),
    GetOrgFreedesktopSystemd1UnitId(String),
    MessageLacksPath,
    PropertiesLacksTimestamp(ActiveState, &'static str),
    RemoveMatch(String, String),
}

impl Display for DBusError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match self {
            DBusError::AddMatch(match_str, cause) => {
                write!(f, "Failed to add match string '{}': {}", match_str, cause)
            }
            DBusError::CallOrgFreedesktopDBusPropertiesGetAll(cause) => {
                write!(f, "Failed to call org.freedesktop.DBus.Properties.GetAll: {}", cause)
            }
            DBusError::CallOrgFreedesktopSystemd1ManagerGetUnit(cause) => {
                write!(f, "Failed to call org.freedesktop.systemd1.Manager.GetUnit: {}", cause)
            }
            DBusError::CallOrgFreedesktopSystemd1ManagerListUnits(cause) => {
                write!(f, "Failed to call org.freedesktop.systemd1.Manager.ListUnits: {}", cause)
            }
            DBusError::CallOrgFreedesktopSystemd1ManagerSubscribe(cause) => {
                write!(f, "Failed to call org.freedesktop.systemd1.Manager.Subscribe: {}", cause)
            }
            DBusError::CastOrgFreedesktopSystemd1UnitTimestamp(timestamp_key) => {
                write!(f, "Failed to cast org.freedesktop.systemd1.Unit.{} to a u64.", timestamp_key)
            }
            DBusError::CastOrgFreedesktopSystemd1UnitActiveState => {
                write!(f, "Failed to cast org.freedesktop.systemd1.Unit.ActiveState to a string.")
            }
            DBusError::CastOrgFreedesktopSystemd1UnitId => {
                write!(f, "Failed to cast org.freedesktop.systemd1.Unit.Id to a string.")
            }
            DBusError::DecodeOrgFreedesktopSystemd1UnitActiveState(cause) => {
                write!(f, "Failed to decode org.freedesktop.systemd1.Unit.ActiveState: {}", cause)
            }
            DBusError::GetOrgFreedesktopSystemd1UnitId(cause) => {
                write!(f, "Failed to get org.freedesktop.systemd1.Unit.Id for: {}", cause)
            }
            DBusError::MessageLacksPath => {
                write!(f, "Failed to get path from message headers.")
            }
            DBusError::PropertiesLacksTimestamp(active_state, timestamp_key) => write!(
                f,
                "A unit has entered the {:?} state, but that unit's properties lack a timestamp named '{}'.",
                active_state, timestamp_key
            ),
            DBusError::RemoveMatch(match_str, cause) => {
                write!(f, "Failed to remove match string '{}': {}", match_str, cause)
            }
        }
    }
}

impl Error for DBusError {}
