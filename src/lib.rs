//! Tools for working with the killjoy systemd unit monitoring application.

mod bus;
pub mod cli;
mod error;
mod generated;
pub mod settings;
mod unit;

use std::collections::HashSet;
use std::thread;

use bus::BusWatcher;
use dbus::BusType;
use settings::{Rule, Settings};

// How verbose should error messages be?
const VERBOSE: bool = false;

// Get a deduplicated list of D-Bus bus types in the given list of rules.
fn get_bus_types(rules: &[Rule]) -> Vec<BusType> {
    // The conversion from BusType → String → BusType is a hack. It's done because this method
    // should deduplicate BusType values, but BusType doesn't implement the traits necessary to
    // create a HashSet<BusType>.
    rules
        .iter()
        .map(|rule: &Rule| settings::encode_bus_type(rule.bus_type))
        .collect::<HashSet<String>>()
        .into_iter()
        .map(|bus_type_str: String| settings::decode_bus_type_str(&bus_type_str[..]).unwrap())
        .collect()
}

/// Connect to D-Bus buses, and maintain state machines for relevant units.
///
/// For each D-Bus bus listed in the settings argument, spawn and start a thread, and configure that
/// thread to connect to one of the buses. At a high level, a thread monitors the interesting units
/// accessible via that bus' systemd instance, and takes action when a unit enters an interesting
/// state.
///
/// Whether a unit is an "interesting unit," and whether it is entering an "interesting state," is
/// defined by the rules in the settings file. Currently, taking action consists of printing a
/// debugging message to the console. In the future, this will consist of reaching out across the
/// D-Bus and contacting the appropriate notifier.
pub fn run(settings: &Settings) {
    let handles: Vec<_> = get_bus_types(&settings.rules)
        .into_iter()
        .map(|bus_type| {
            let settings_clone = settings.clone();
            thread::spawn(move || BusWatcher::new(bus_type, settings_clone).run())
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }
}

#[cfg(test)]
mod test_utils {
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
}
