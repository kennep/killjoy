// Logic for dealing with settings files.

use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use std::error::Error;
use std::fs::File;
use std::io::{BufReader, Read};

use dbus::{BusName, BusType};
use serde::Deserialize;
use xdg::BaseDirectories;

use crate::error::{FindConfigFileError, ParseConfigFileError, ParsePathError};
use crate::unit::ActiveState;

// The expressions that a user may use to match unit names.
#[derive(Clone, Debug)]
pub enum Expression {
    Regex(regex::Regex),
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
        match &self {
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
    pub fn new(bus_name: &str, bus_type: BusType) -> Result<Self, Box<dyn Error>> {
        let new_obj = Self {
            bus_name: bus_name.to_owned(),
            bus_type,
        };
        new_obj.maybe_get_bus_name()?;
        Ok(new_obj)
    }

    /// Get the `bus_name` attribute.
    pub fn get_bus_name(&self) -> BusName {
        self.maybe_get_bus_name().expect(
            "bus_name is invalid. new() should have caught this. Please contact a developer.",
        )
    }

    fn maybe_get_bus_name<'bn>(&'bn self) -> Result<BusName<'bn>, Box<dyn Error>> {
        Ok(BusName::new(&self.bus_name[..])?)
    }
}

impl TryFrom<SerdeNotifier> for Notifier {
    type Error = Box<dyn Error>;

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
    type Error = Box<dyn Error>;

    fn try_from(value: SerdeRule) -> Result<Self, Self::Error> {
        let mut active_states: HashSet<ActiveState> = HashSet::new();
        for active_state_string in &value.active_states {
            let active_state = ActiveState::try_from(&active_state_string[..])?;
            active_states.insert(active_state);
        }

        let bus_type = decode_bus_type_str(&value.bus_type)?;

        let expression: Expression = match &value.expression_type[..] {
            "regex" => Expression::Regex(regex::Regex::new(&value.expression[..])?),
            "unit name" => Expression::UnitName(value.expression.to_owned()),
            "unit type" => Expression::UnitType(value.expression.to_owned()),
            other => {
                let msg = format!("Found unknown expression type: {}", other);
                return Err(Box::new(ParseConfigFileError { msg }));
            }
        };

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
    pub fn new<T: Read>(reader: T) -> Result<Self, Box<dyn Error>> {
        let serde_settings: SerdeSettings = serde_json::from_reader(reader)?;
        let settings = Self::try_from(serde_settings)?;
        Ok(settings)
    }
}

impl TryFrom<SerdeSettings> for Settings {
    type Error = Box<dyn Error>;

    fn try_from(value: SerdeSettings) -> Result<Self, Self::Error> {
        // Use for loops instead of chaining method calls on iter() so that the ? operator may be
        // used.
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
                    let msg = format!("Rule references non-existent notifier: {}", notifier);
                    return Err(Box::new(ParseConfigFileError { msg }));
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
pub fn get_load_path() -> Result<String, Box<dyn Error>> {
    let dirs = match BaseDirectories::with_prefix("killjoy") {
        Ok(dirs) => dirs,
        Err(err) => return Err(Box::new(err)),
    };
    let path_buf = match dirs.find_config_file("settings.json") {
        Some(path_buf) => path_buf,
        None => return Err(Box::new(FindConfigFileError)),
    };
    let path = match path_buf.to_str() {
        Some(path) => path.to_string(),
        None => return Err(Box::new(ParsePathError)),
    };
    Ok(path)
}

// Read the configuration file into a Settings object.
//
// An error may be returned for one of two broad categories of reasons:
//
// *   The file couldn't be opened. Maybe a settings file couldn't be found; or maybe a settings
//     file was found but could not be opened.
// *   The file contained invalid contents. See [Settings::new](struct.Settings.html#method.new).
pub fn load(path: Option<&str>) -> Result<Settings, Box<dyn Error>> {
    let load_path = match path {
        Some(path) => path.to_owned(),
        None => get_load_path()?,
    };
    let handle = match File::open(load_path) {
        Ok(handle) => handle,
        Err(err) => return Err(Box::new(err)),
    };
    let reader = BufReader::new(handle);
    let settings = Settings::new(reader)?;
    Ok(settings)
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

    #[test]
    fn test_expression_unit_name_matches_success() {
        let unit_name = "aaa.service";
        let expression = Expression::UnitName("aaa.service".to_string());
        assert!(expression.matches(&unit_name));
    }

    #[test]
    fn test_expression_unit_name_matches_failure() {
        let unit_name = "aaa.service";
        let expression = Expression::UnitName("aa.service".to_string());
        assert!(!expression.matches(&unit_name));
    }

    #[test]
    fn test_expression_unit_type_matches_success() {
        let unit_name = "aaa.service";
        let expression = Expression::UnitType(".service".to_string());
        assert!(expression.matches(&unit_name));
    }

    #[test]
    fn test_expression_unit_type_matches_failure() {
        let unit_name = "aaa.service";
        let expression = Expression::UnitType(".mount".to_string());
        assert!(!expression.matches(&unit_name));
    }

    #[test]
    fn test_expression_regex_matches() {
        let expression = Expression::Regex(regex::Regex::new(r"a\.service").unwrap());
        assert!(!expression.matches(".service"));
        assert!(expression.matches("a.service"));
        assert!(expression.matches("aa.service"));
    }

    #[test]
    fn test_notifier_new_success() {
        let bus_name = "org.freedesktop.Notifications";
        let bus_type = BusType::Session;
        Notifier::new(bus_name, bus_type).unwrap();
    }

    #[test]
    fn test_notifier_new_failure() {
        let bus_name = "org/freedesktop/Notifications";
        let bus_type = BusType::Session;
        match Notifier::new(bus_name, bus_type) {
            Ok(_) => panic!("bus_name should have been invalid"),
            Err(_) => {}
        }
    }

    #[test]
    fn test_settings_new_success() {
        let settings_str = r###"
            {
                "notifiers": {
                    "desktop popup": {
                        "bus_name": "org.freedesktop.Notifications",
                        "bus_type": "session"
                    }
                },
                "rules": [{
                        "active_states": ["failed"],
                        "bus_type": "session",
                        "expression": "syncthing.service",
                        "expression_type": "unit name",
                        "notifiers": ["desktop popup"]
                }],
                "version": 1
            }
        "###;
        let settings: Settings = Settings::new(settings_str.as_bytes()).unwrap();

        let actual = settings.notifiers.get("desktop popup").unwrap().bus_type;
        let target = BusType::Session;
        assert_eq!(actual, target);
    }
}
