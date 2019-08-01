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
