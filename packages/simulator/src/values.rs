//! Waarden vergelijken zoals een leesbaar bestand ze bedoelt.
//!
//! Eén plek, want er zijn twee plekken die dezelfde vraag stellen: het
//! kroniekfilter (voldoet dit veld aan deze voorwaarde?) en de scenario-runner
//! (kwam deze verwachting uit?). Wie ze uit elkaar laat lopen, krijgt een
//! scenario dat iets anders vergelijkt dan de cel.

use regelrecht_engine::Value;

/// Gelijkheid met één versoepeling: getallen vergelijken op waarde, niet op
/// variant. YAML kent geen verschil tussen `1` en `1.0`, de engine wel.
pub(crate) fn equivalent(left: &Value, right: &Value) -> bool {
    match (left.as_decimal(), right.as_decimal()) {
        (Some(a), Some(b)) => a == b,
        _ => left == right,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn getallen_vergelijken_op_waarde() {
        assert!(equivalent(&Value::Int(1), &Value::Int(1)));
        assert!(!equivalent(&Value::Int(1), &Value::Int(2)));
        assert!(!equivalent(&Value::Int(1), &Value::String("1".to_string())));
    }

    #[test]
    fn afwezigheid_is_gelijk_aan_afwezigheid() {
        assert!(equivalent(&Value::Null, &Value::Null));
        assert!(!equivalent(&Value::Null, &Value::Bool(false)));
    }
}
