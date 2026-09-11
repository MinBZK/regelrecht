//! Waarden vergelijken en opschrijven zoals een leesbaar bestand ze bedoelt.
//!
//! Eén plek, want er zijn twee plekken die dezelfde vraag stellen: het
//! kroniekfilter (voldoet dit veld aan deze voorwaarde?) en de scenario-runner
//! (kwam deze verwachting uit?). Wie ze uit elkaar laat lopen, krijgt een
//! scenario dat iets anders vergelijkt dan de cel.
//!
//! Hier staat ook de omgekeerde weg: een uitgerekend bedrag als vastlegbare
//! waarde ([`amount`]). Ook die hoort op één plek, want een betalingstermijn en
//! een som over betalingen moeten dezelfde waarde opleveren voor hetzelfde
//! getal — anders klopt "betaald tot nu toe" op het oog wel en op de variant niet.

use regelrecht_engine::Value;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

/// Gelijkheid met één versoepeling: getallen vergelijken op waarde, niet op
/// variant. YAML kent geen verschil tussen `1` en `1.0`, de engine wel.
pub(crate) fn equivalent(left: &Value, right: &Value) -> bool {
    match (left.as_decimal(), right.as_decimal()) {
        (Some(a), Some(b)) => a == b,
        _ => left == right,
    }
}

/// Een uitgerekend bedrag als vastlegbare waarde.
///
/// Een geheel bedrag blijft geheel: `Value::Int(25000)` en niet
/// `Value::Decimal(25000)`. Voor de vergelijking maakt dat niets uit (zie
/// [`equivalent`]), voor wat een lezer in een verslag of een gram ziet wel — en
/// een bedrag in hele eenheden hoort er niet uit te zien als een breuk.
pub(crate) fn amount(value: Decimal) -> Value {
    if value.fract() == Decimal::ZERO {
        if let Some(whole) = value.to_i64() {
            return Value::Int(whole);
        }
    }
    Value::Decimal(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn een_heel_bedrag_blijft_geheel() {
        assert_eq!(amount(Decimal::new(25000, 0)), Value::Int(25000));
    }

    #[test]
    fn een_bedrag_met_een_breuk_houdt_zijn_breuk() {
        let met_breuk = Decimal::new(4930231187, 5);
        assert_eq!(amount(met_breuk), Value::Decimal(met_breuk));
    }

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
