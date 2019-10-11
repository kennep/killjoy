// Logic for dealing with settings files.

use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use dbus::{BusName, BusType};
use regex::Regex;
use serde::Deserialize;
use xdg::BaseDirectories;

use crate::error::Error as CrateError;
use crate::unit::ActiveState;

// The expressions that a user may use to match unit names.
#[derive(Clone, Debug)]
pub enum Expression {
    Regex(Regex),
    UnitName(String),
    UnitType(String),
}

impl Expression {
    // Check whether this expression matches the given `unit_name`.
    //
    // A `UnitName` expression matches unit names against a unit name. A `UnitType` expression
    // matches unit names against a unit type. A `Regex` expression matches unit names against a
    // regular expression.
    //
    // Regular expressions are implemented with the regex crate. See: https://docs.rs/regex/
    pub fn matches(&self, unit_name: &str) -> bool {
        match self {
            Expression::Regex(expr) => expr.is_match(unit_name),
            Expression::UnitName(expr) => unit_name == expr,
            Expression::UnitType(expr) => unit_name.ends_with(expr),
        }
    }
}

// A D-Bus service that may be contacted when an event of interest happens.
//
// When an event of interest occurs, killjoy will connect to `bus_type` and send a message to
// `bus_name`.
#[derive(Clone, Debug)]
pub struct Notifier {
    bus_name: String,
    pub bus_type: BusType,
}

impl Notifier {
    // Create a new notifier.
    //
    // Return an error if any arguments are invalid.
    pub fn new(bus_name: &str, bus_type: BusType) -> Result<Self, CrateError> {
        let new_obj = Self {
            bus_name: bus_name.to_owned(),
            bus_type,
        };
        new_obj.maybe_get_bus_name()?;
        Ok(new_obj)
    }

    // Get the `bus_name` attribute.
    pub fn get_bus_name(&self) -> BusName {
        self.maybe_get_bus_name().expect(
            "bus_name is invalid. new() should have caught this. Please contact a developer.",
        )
    }

    fn maybe_get_bus_name(&self) -> Result<BusName, CrateError> {
        BusName::new(&self.bus_name[..])
            .map_err(|_| CrateError::InvalidBusName(self.bus_name.to_owned()))
    }
}

impl TryFrom<SerdeNotifier> for Notifier {
    type Error = CrateError;

    fn try_from(value: SerdeNotifier) -> Result<Self, Self::Error> {
        let notifier = Notifier::new(&value.bus_name, decode_bus_type_str(&value.bus_type)?)?;
        Ok(notifier)
    }
}

// Units to watch, and notifiers to contact when any of them enter a state of interest.
//
// Upon startup, killjoy will connect to `bus_type`. It will watch all units whose name matches
// `expression`. Whenever one of those units' ActiveState property transitions to one of the
// `active_states` it will contact `notifiers`.
#[derive(Clone, Debug)]
pub struct Rule {
    pub active_states: HashSet<ActiveState>,
    pub bus_type: BusType,
    pub expression: Expression,
    pub notifiers: Vec<String>,
}

impl TryFrom<SerdeRule> for Rule {
    type Error = CrateError;

    fn try_from(value: SerdeRule) -> Result<Self, Self::Error> {
        let mut active_states: HashSet<ActiveState> = HashSet::new();
        for active_state_string in &value.active_states {
            let active_state = ActiveState::try_from(&active_state_string[..])
                .map_err(|_| CrateError::InvalidActiveState(active_state_string.to_owned()))?;
            active_states.insert(active_state);
        }
        let active_states = active_states;

        let bus_type = decode_bus_type_str(&value.bus_type)?;

        let expression: Expression = match &value.expression_type[..] {
            "regex" => Regex::new(&value.expression[..])
                .map(Expression::Regex)
                .map_err(CrateError::InvalidRegex),
            "unit name" => Ok(Expression::UnitName(value.expression.to_owned())),
            "unit type" => Ok(Expression::UnitType(value.expression.to_owned())),
            other => Err(CrateError::InvalidExpressionType(other.to_owned())),
        }?;

        let notifiers = value.notifiers.to_owned();

        Ok(Rule {
            active_states,
            bus_type,
            expression,
            notifiers,
        })
    }
}

// A deserialized copy of a configuration file.
//
// Beware that `Settings` instances may have semantically invalid values. For example, a notifier's
// `bus_name` might be syntactically valid but may point to a non-existent entity.
#[derive(Clone, Debug)]
pub struct Settings {
    pub notifiers: HashMap<String, Notifier>,
    pub rules: Vec<Rule>,
}

impl Settings {
    // Create a new settings object.
    //
    // An error may be returned for one of two broad categories of reasons:
    //
    // *   Deserialization of the `reader` failed. Maybe the reader yielded non-unicode bytes; maybe
    //     the bytes were valid unicode but not valid JSON; maybe the unicode was valid JSON but
    //     didn't match the settings file schema; or so on.
    // *   The settings object contained semantically invalid data. Maybe a `"bus_type"` key was set
    //     to a value such as `"foo"`, or so on.
    pub fn new<T: Read>(reader: T) -> Result<Self, CrateError> {
        let serde_settings: SerdeSettings = serde_json::from_reader(reader)
            .map_err(CrateError::SettingsFileDeserializationFailed)?;
        Self::try_from(serde_settings)
    }
}

impl TryFrom<SerdeSettings> for Settings {
    type Error = CrateError;

    fn try_from(value: SerdeSettings) -> Result<Self, Self::Error> {
        let mut notifiers: HashMap<String, Notifier> = HashMap::new();
        for (key, serde_notifier) in value.notifiers.into_iter() {
            let notifier = Notifier::try_from(serde_notifier)?;
            notifiers.insert(key, notifier);
        }
        let notifiers = notifiers; // make immutable

        let mut rules: Vec<Rule> = Vec::new();
        for serde_rule in value.rules.into_iter() {
            let rule = Rule::try_from(serde_rule)?;
            for notifier in &rule.notifiers {
                if !notifiers.contains_key(notifier) {
                    return Err(CrateError::InvalidNotifier(notifier.to_owned()));
                }
            }
            rules.push(rule);
        }
        let rules = rules; // make immutable

        Ok(Self { notifiers, rules })
    }
}

// See SerdeSettings.
#[derive(Deserialize)]
struct SerdeNotifier {
    bus_name: String,
    bus_type: String,
}

// See SerdeSettings.
#[derive(Deserialize)]
struct SerdeRule {
    active_states: Vec<String>,
    bus_type: String,
    expression: String,
    expression_type: String,
    notifiers: Vec<String>,
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
#[derive(Deserialize)]
struct SerdeSettings {
    notifiers: HashMap<String, SerdeNotifier>,
    rules: Vec<SerdeRule>,
}

// This struct is a hack. See get_bus_types().
#[derive(PartialEq, Eq, Hash)]
enum HashableBusType {
    Session,
    Starter,
    System,
}

// This impl is a hack. See get_bus_types().
impl From<BusType> for HashableBusType {
    fn from(value: BusType) -> Self {
        match value {
            BusType::Session => HashableBusType::Session,
            BusType::Starter => HashableBusType::Starter,
            BusType::System => HashableBusType::System,
        }
    }
}

// This impl is a hack. See get_bus_types().
impl Into<BusType> for HashableBusType {
    fn into(self) -> BusType {
        match self {
            HashableBusType::Session => BusType::Session,
            HashableBusType::Starter => BusType::Starter,
            HashableBusType::System => BusType::System,
        }
    }
}

pub fn decode_bus_type_str(bus_type_str: &str) -> Result<BusType, CrateError> {
    match bus_type_str {
        "session" => Ok(BusType::Session),
        "starter" => Ok(BusType::Starter),
        "system" => Ok(BusType::System),
        other => Err(CrateError::InvalidBusType(other.to_owned())),
    }
}

// Get a deduplicated list of D-Bus bus types in the given list of rules.
pub fn get_bus_types(rules: &[Rule]) -> Vec<BusType> {
    // The conversion from BusType → HashableBusType → BusType is a hack. It's done because this
    // method should deduplicate BusType values, but BusType doesn't implement the traits necessary
    // to create a HashSet<BusType>.
    rules
        .iter()
        .map(|rule: &Rule| HashableBusType::from(rule.bus_type))
        .collect::<HashSet<HashableBusType>>()
        .into_iter()
        .map(|hashable_bus_type: HashableBusType| hashable_bus_type.into())
        .collect()
}

// Search several paths for a settings file, in order of preference.
//
// If a file is found, return its path. Otherwise, return an error describing why.
pub fn get_load_path() -> Result<PathBuf, CrateError> {
    let prefix = "killjoy";
    let suffix = "settings.json";
    BaseDirectories::with_prefix(prefix)
        .map_err(|_| CrateError::SettingsFileNotFound(format!("{}/{}", prefix, suffix)))?
        .find_config_file(suffix)
        .ok_or_else(|| CrateError::SettingsFileNotFound(format!("{}/{}", prefix, suffix)))
}

// Read the configuration file into a Settings object.
//
// An error may be returned for one of two broad categories of reasons:
//
// *   The file couldn't be opened. Maybe a settings file couldn't be found; or maybe a settings
//     file was found but could not be opened.
// *   The file contained invalid contents.
pub fn load(path_opt: Option<&Path>) -> Result<Settings, CrateError> {
    let handle_res = match path_opt {
        Some(path) => File::open(path),
        None => File::open(get_load_path()?.as_path()),
    };
    let handle = handle_res.map_err(CrateError::SettingsFileNotReadable)?;
    let reader = BufReader::new(handle);
    Settings::new(reader)
}

#[cfg(test)]
pub mod test_utils {
    use crate::settings::{Expression, Rule};
    use dbus::BusType;
    use std::collections::HashSet;

    pub fn gen_session_rule() -> Rule {
        Rule {
            active_states: HashSet::new(),
            bus_type: BusType::Session,
            expression: Expression::UnitName("".to_string()),
            notifiers: Vec::new(),
        }
    }

    pub fn gen_system_rule() -> Rule {
        Rule {
            active_states: HashSet::new(),
            bus_type: BusType::System,
            expression: Expression::UnitName("".to_string()),
            notifiers: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    // get_bus_types()
    #[test]
    fn test_get_bus_types_v1() {
        let settings = Settings {
            notifiers: HashMap::new(),
            rules: Vec::new(),
        };
        let bus_types = get_bus_types(&settings.rules);
        assert!(!bus_types.contains(&BusType::Session));
        assert!(!bus_types.contains(&BusType::System));
    }

    // get_bus_types()
    #[test]
    fn test_get_bus_types_v2() {
        let settings = Settings {
            notifiers: HashMap::new(),
            rules: vec![test_utils::gen_session_rule()],
        };
        let bus_types: Vec<BusType> = get_bus_types(&settings.rules);
        assert!(bus_types.contains(&BusType::Session));
        assert!(!bus_types.contains(&BusType::System));
    }

    // get_bus_types()
    #[test]
    fn test_get_bus_types_v3() {
        let settings = Settings {
            notifiers: HashMap::new(),
            rules: vec![test_utils::gen_system_rule()],
        };
        let bus_types: Vec<BusType> = get_bus_types(&settings.rules);
        assert!(!bus_types.contains(&BusType::Session));
        assert!(bus_types.contains(&BusType::System));
    }

    // get_bus_types()
    #[test]
    fn test_get_bus_types_v4() {
        let settings = Settings {
            notifiers: HashMap::new(),
            rules: vec![
                test_utils::gen_session_rule(),
                test_utils::gen_system_rule(),
            ],
        };
        let bus_types: Vec<BusType> = get_bus_types(&settings.rules);
        assert!(bus_types.contains(&BusType::Session));
        assert!(bus_types.contains(&BusType::System));
    }

    // Expression::UnitName::matches()
    #[test]
    fn test_expression_unit_name_matches_success() {
        let unit_name = "aaa.service";
        let expression = Expression::UnitName("aaa.service".to_string());
        assert!(expression.matches(unit_name));
    }

    // Expression::UnitName::matches()
    #[test]
    fn test_expression_unit_name_matches_failure() {
        let unit_name = "aaa.service";
        let expression = Expression::UnitName("aa.service".to_string());
        assert!(!expression.matches(unit_name));
    }

    // Expression::UnitType::matches()
    #[test]
    fn test_expression_unit_type_matches_success() {
        let unit_name = "aaa.service";
        let expression = Expression::UnitType(".service".to_string());
        assert!(expression.matches(unit_name));
    }

    // Expression::UnitType::matches()
    #[test]
    fn test_expression_unit_type_matches_failure() {
        let unit_name = "aaa.service";
        let expression = Expression::UnitType(".mount".to_string());
        assert!(!expression.matches(unit_name));
    }

    // Expression::UnitRegex::matches()
    #[test]
    fn test_expression_regex_matches() {
        let expression =
            Expression::Regex(Regex::new(r"a\.service").expect("Failed to compile regex."));
        assert!(!expression.matches(".service"));
        assert!(expression.matches("a.service"));
        assert!(expression.matches("aa.service"));
    }

    // Settings::new()
    #[test]
    fn test_settings_new() {
        let settings_str = r###"
            {
                "rules": [{
                        "active_states": ["failed"],
                        "bus_type": "session",
                        "expression": "syncthing.service",
                        "expression_type": "unit name",
                        "notifiers": ["desktop popup"]
                }],
                "notifiers": {
                    "desktop popup": {
                        "bus_name": "name.jerebear.KilljoyNotifierNotification1",
                        "bus_type": "session"
                    }
                },
                "version": 1
            }
        "###;
        Settings::new(settings_str.as_bytes()).expect("valid settings parsed as invalid");
    }

    // Settings::new()
    #[test]
    fn test_settings_new_deserialization_failed() {
        let settings_str = r###"
            {
                "rules": [{
                        "active_states": ["failed"],
                        "bus_type": "session",
                        "expression": "syncthing.service",
                        "expression_type": "unit name",
                        "notifiers": ["desktop popup"]
                }],
                "notifiers": {
                    "desktop popup": {
                        "bus_name": "name.jerebear.KilljoyNotifierNotification1",
                        "bus_type": "session",
                    }
                },
                "version": 1
            }
        "###;
        match Settings::new(settings_str.as_bytes()) {
            Err(CrateError::SettingsFileDeserializationFailed(_)) => {}
            _ => panic!("expected DeserializationFailed; an extra comma has been added"),
        }
    }

    // Settings::new()
    #[test]
    fn test_settings_new_invalid_active_state() {
        let settings_str = r###"
            {
                "rules": [{
                        "active_states": ["failedd"],
                        "bus_type": "session",
                        "expression": "syncthing.service",
                        "expression_type": "unit name",
                        "notifiers": ["desktop popup"]
                }],
                "notifiers": {
                    "desktop popup": {
                        "bus_name": "name.jerebear.KilljoyNotifierNotification1",
                        "bus_type": "session"
                    }
                },
                "version": 1
            }
        "###;
        match Settings::new(settings_str.as_bytes()) {
            Err(CrateError::InvalidActiveState(_)) => {}
            _ => panic!("expected InvalidActiveState; an active state has been typo'd"),
        }
    }

    // Settings::new()
    #[test]
    fn test_settings_new_invalid_bus_name() {
        let settings_str = r###"
            {
                "rules": [{
                        "active_states": ["failed"],
                        "bus_type": "session",
                        "expression": "syncthing.service",
                        "expression_type": "unit name",
                        "notifiers": ["desktop popup"]
                }],
                "notifiers": {
                    "desktop popup": {
                        "bus_name": "name/jerebear/KilljoyNotifierNotification1",
                        "bus_type": "session"
                    }
                },
                "version": 1
            }
        "###;
        match Settings::new(settings_str.as_bytes()) {
            Err(CrateError::InvalidBusName(_)) => {}
            _ => panic!("expected InvalidBusName; a bus name has been typo'd"),
        }
    }

    // Settings::new()
    #[test]
    fn test_settings_new_invalid_bus_type_v1() {
        let settings_str = r###"
            {
                "rules": [{
                        "active_states": ["failed"],
                        "bus_type": "sessionn",
                        "expression": "syncthing.service",
                        "expression_type": "unit name",
                        "notifiers": ["desktop popup"]
                }],
                "notifiers": {
                    "desktop popup": {
                        "bus_name": "name.jerebear.KilljoyNotifierNotification1",
                        "bus_type": "session"
                    }
                },
                "version": 1
            }
        "###;
        match Settings::new(settings_str.as_bytes()) {
            Err(CrateError::InvalidBusType(_)) => {}
            _ => panic!("expected InvalidBusType; a bus type has been typo'd"),
        }
    }

    // Settings::new()
    #[test]
    fn test_settings_new_invalid_bus_type_v2() {
        let settings_str = r###"
            {
                "rules": [{
                        "active_states": ["failed"],
                        "bus_type": "session",
                        "expression": "syncthing.service",
                        "expression_type": "unit name",
                        "notifiers": ["desktop popup"]
                }],
                "notifiers": {
                    "desktop popup": {
                        "bus_name": "name.jerebear.KilljoyNotifierNotification1",
                        "bus_type": "sessionn"
                    }
                },
                "version": 1
            }
        "###;
        match Settings::new(settings_str.as_bytes()) {
            Err(CrateError::InvalidBusType(_)) => {}
            _ => panic!("expected InvalidBusType; a bus type has been typo'd"),
        }
    }

    // Settings::new()
    #[test]
    fn test_settings_new_invalid_expression_type() {
        let settings_str = r###"
            {
                "rules": [{
                        "active_states": ["failed"],
                        "bus_type": "session",
                        "expression": "syncthing.service",
                        "expression_type": "unit namee",
                        "notifiers": ["desktop popup"]
                }],
                "notifiers": {
                    "desktop popup": {
                        "bus_name": "name.jerebear.KilljoyNotifierNotification1",
                        "bus_type": "session"
                    }
                },
                "version": 1
            }
        "###;
        match Settings::new(settings_str.as_bytes()) {
            Err(CrateError::InvalidExpressionType(_)) => {}
            _ => panic!("expected InvalidExpressionType; an expression type has been typo'd"),
        }
    }

    // Settings::new()
    #[test]
    fn test_settings_new_invalid_regex() {
        let settings_str = r###"
            {
                "rules": [{
                        "active_states": ["failed"],
                        "bus_type": "session",
                        "expression": "{",
                        "expression_type": "regex",
                        "notifiers": ["desktop popup"]
                }],
                "notifiers": {
                    "desktop popup": {
                        "bus_name": "name.jerebear.KilljoyNotifierNotification1",
                        "bus_type": "session"
                    }
                },
                "version": 1
            }
        "###;
        match Settings::new(settings_str.as_bytes()) {
            Err(CrateError::InvalidRegex(_)) => {}
            _ => panic!("expected InvalidRegex; a regex has been typo'd"),
        }
    }

    // Settings::new()
    #[test]
    fn test_settings_new_invalid_notifier() {
        let settings_str = r###"
            {
                "rules": [{
                        "active_states": ["failed"],
                        "bus_type": "session",
                        "expression": "syncthing.service",
                        "expression_type": "unit name",
                        "notifiers": ["desktop popupp"]
                }],
                "notifiers": {
                    "desktop popup": {
                        "bus_name": "name.jerebear.KilljoyNotifierNotification1",
                        "bus_type": "session"
                    }
                },
                "version": 1
            }
        "###;
        match Settings::new(settings_str.as_bytes()) {
            Err(CrateError::InvalidNotifier(_)) => {}
            _ => panic!("expected InvalidNotifier; a notifier has been typo'd"),
        }
    }
}
