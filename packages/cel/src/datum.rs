//! Tijd in de cel-runtime, op een plek: de twee tijden van een gram
//! (`op_moment` en `vastgelegd_op`, elk een moment met tijdzone, RFC 3339),
//! de peildatum waarop de engine een regeling leest (`JJJJ-MM-DD`) en het
//! [`Tijdpunt`] waarop een reductie peilt.
//!
//! De peildatum van een moment is de kalenderdatum in de tijdzone van dat
//! moment: `2025-03-12T00:30:00+01:00` is 12 maart, ook al is het dan in UTC
//! nog 11 maart.

use chrono::{DateTime, Datelike, FixedOffset, NaiveDate, SecondsFormat};

/// Een `op_moment` gelezen. Een moment zonder tijdzone of in een andere vorm
/// is een fout, met het moment erbij.
pub fn moment(tekst: &str) -> Result<DateTime<FixedOffset>, String> {
    moment_van("op_moment", tekst)
}

/// Een moment gelezen; een fout noemt het veld (`op_moment`,
/// `vastgelegd_op`, `peilmoment`) en het moment.
pub fn moment_van(veld: &str, tekst: &str) -> Result<DateTime<FixedOffset>, String> {
    DateTime::parse_from_rfc3339(tekst).map_err(|e| format!("ongeldig {veld} '{tekst}': {e}"))
}

/// Een punt op de tijdas: een datum (`JJJJ-MM-DD`) of een moment met
/// tijdzone. Een peilmoment is er een, en een `op_moment` dat een event aan
/// een ingediende waarde bindt (zoals een datumstempel) ook.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tijdpunt {
    /// Een hele kalenderdag.
    Datum(NaiveDate),
    Moment(DateTime<FixedOffset>),
}

impl Tijdpunt {
    /// Lees een datum of een moment; `veld` noemt wat het is in de fout.
    pub fn lees(veld: &str, tekst: &str) -> Result<Self, String> {
        match NaiveDate::parse_from_str(tekst, "%Y-%m-%d") {
            Ok(d) => Ok(Self::Datum(d)),
            Err(_) => DateTime::parse_from_rfc3339(tekst)
                .map(Self::Moment)
                .map_err(|_| {
                    format!("ongeldig {veld} '{tekst}': geen datum (JJJJ-MM-DD) en geen moment met tijdzone (RFC 3339)")
                }),
        }
    }

    /// Of een moment op of voor dit tijdpunt ligt. Een datum telt de hele
    /// dag, in de tijdzone van het moment (zoals [`peildatum`]).
    pub fn omvat(&self, m: &DateTime<FixedOffset>) -> bool {
        match self {
            Self::Datum(d) => m.date_naive() <= *d,
            Self::Moment(t) => m <= t,
        }
    }

    /// Het tijdpunt als moment: een datum is het begin van die dag, in de
    /// tijdzone `zone`.
    pub fn als_moment(&self, zone: FixedOffset) -> DateTime<FixedOffset> {
        match self {
            Self::Moment(m) => *m,
            Self::Datum(d) => {
                let lokaal = d.and_time(chrono::NaiveTime::MIN);
                let utc = lokaal - chrono::Duration::seconds(i64::from(zone.local_minus_utc()));
                DateTime::from_naive_utc_and_offset(utc, zone)
            }
        }
    }
}

impl std::fmt::Display for Tijdpunt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Datum(d) => write!(f, "{}", d.format("%Y-%m-%d")),
            Self::Moment(m) => f.write_str(&als_op_moment(m)),
        }
    }
}

/// Een moment zoals een gram het vastlegt: op de seconde, met tijdzone.
pub fn als_op_moment(m: &DateTime<FixedOffset>) -> String {
    m.to_rfc3339_opts(SecondsFormat::Secs, false)
}

/// De peildatum (`JJJJ-MM-DD`) van een moment, in zijn eigen tijdzone.
pub fn peildatum(m: &DateTime<FixedOffset>) -> String {
    m.date_naive().format("%Y-%m-%d").to_string()
}

/// De peildatum van een `op_moment`.
pub fn peildatum_van(op_moment: &str) -> Result<String, String> {
    moment(op_moment).map(|m| peildatum(&m))
}

/// Het jaartal van een moment.
pub fn jaar(m: &DateTime<FixedOffset>) -> i64 {
    i64::from(m.year())
}

/// Het jaartal van een datum (`JJJJ-MM-DD`) of een moment met tijdzone.
/// `None` als de tekst geen van beide is.
pub fn jaar_van(tekst: &str) -> Option<i64> {
    match NaiveDate::parse_from_str(tekst, "%Y-%m-%d") {
        Ok(d) => Some(i64::from(d.year())),
        Err(_) => moment(tekst).ok().map(|m| jaar(&m)),
    }
}

/// 1 januari van een jaar, als datum: de tegenhanger van [`jaar_van`].
pub fn eerste_dag_van_het_jaar(jaar: i64) -> String {
    format!("{jaar:04}-01-01")
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn peildatum_in_de_eigen_tijdzone() {
        let m = moment("2025-03-12T00:30:00+01:00").unwrap();
        assert_eq!(peildatum(&m), "2025-03-12");
        assert_eq!(
            peildatum_van("2024-12-31T23:59:59-05:00").unwrap(),
            "2024-12-31"
        );
        assert_eq!(als_op_moment(&m), "2025-03-12T00:30:00+01:00");
    }

    #[test]
    fn een_ongeldig_moment_is_een_fout() {
        let f = moment("12 maart 2025").unwrap_err();
        assert!(f.contains("ongeldig op_moment '12 maart 2025'"), "{f}");
        // Een datum zonder tijd is geen moment.
        assert!(moment("2025-03-12").is_err());
    }

    #[test]
    fn een_tijdpunt_is_een_datum_of_een_moment() {
        let d = Tijdpunt::lees("peilmoment", "2026-01-01").unwrap();
        assert_eq!(d.to_string(), "2026-01-01");
        // Een datum telt de hele dag, in de tijdzone van het moment.
        assert!(d.omvat(&moment("2026-01-01T23:59:59+01:00").unwrap()));
        assert!(!d.omvat(&moment("2026-01-02T00:00:00+01:00").unwrap()));
        let m = Tijdpunt::lees("peilmoment", "2026-01-01T12:00:00+01:00").unwrap();
        assert!(m.omvat(&moment("2026-01-01T11:00:00Z").unwrap()));
        assert!(!m.omvat(&moment("2026-01-01T11:00:01Z").unwrap()));
        let zone = FixedOffset::east_opt(3600).unwrap();
        assert_eq!(
            als_op_moment(&d.als_moment(zone)),
            "2026-01-01T00:00:00+01:00"
        );
        let f = Tijdpunt::lees("peilmoment", "gisteren").unwrap_err();
        assert!(f.contains("ongeldig peilmoment 'gisteren'"), "{f}");
    }

    #[test]
    fn jaar_van_een_datum_of_een_moment() {
        assert_eq!(jaar_van("2025-03-12"), Some(2025));
        assert_eq!(jaar_van("2026-01-01T00:00:00+01:00"), Some(2026));
        assert_eq!(jaar_van("geen datum"), None);
        assert_eq!(eerste_dag_van_het_jaar(2026), "2026-01-01");
    }
}
