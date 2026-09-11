//! Het observatielog: een meetinstrument, buiten de band.
//!
//! Registreert elk contact over een celgrens: wie vroeg, aan wie, welke
//! lexostatus, met welke parameters, op welk moment, ondertekend door wie, en
//! wat er terugkwam. Dat is wat een run nodig heeft om te kunnen bewijzen dat
//! een cel alleen de cellen bevraagt die haar eigen wetten noemen — een
//! invariant die *niet* in RFC-022 staat en die wij hier zelf toevoegen, omdat
//! de RFC autonomie wel beweert maar niet meet.
//!
//! ## Waarom dit geen runtimecomponent is
//!
//! Het log ziet álles. Daarmee kent de houder van het log de unie van wat over
//! de grenzen ging — precies het totaalbeeld waarvan de hele opstelling zegt
//! dat het nergens bestaat. Een observator die meedraait in productie zou die
//! claim ongedaan maken, hoe netjes hij ook geschreven is.
//!
//! Daarom drie regels, en de eerste twee zijn afgedwongen in plaats van
//! afgesproken:
//!
//! 1. **Geen productiepad verwijst hiernaar.** Deze module wordt door geen enkel
//!    ander bestand in `src/` geïmporteerd; een test in `tests/` grept daarop en
//!    wordt rood zodra iemand het toch doet. De crate-wortel re-exporteert deze
//!    types met opzet niet.
//! 2. **`Cell` komt hier niet voor.** Het log leest wat een vraag opleverde, het
//!    kan geen cel bevragen en geen kroniek bereiken.
//! 3. **Passief.** Een recorder beïnvloedt niets: hij levert geen waarde aan een
//!    beslissing, weigert nooit een aanroep en kan niet falen. [`ObservationLog`]
//!    heeft daarom geen methode die iets teruggeeft aan de aanroeper van
//!    [`crate::SecurityContext::query`].

use crate::{LexostatusOutcome, SignedAnswer};
use std::fmt::Write as _;

/// Een passieve recorder van cross-cel-contacten.
///
/// Het log bewaart het bewijsstuk dat de veiligheidscontext teruggeeft, en maakt
/// er geen eigen kopie-met-andere-namen van: wat er in het log staat is exact
/// wat er over de grens ging.
#[derive(Debug, Default)]
pub struct ObservationLog {
    entries: Vec<SignedAnswer>,
}

impl ObservationLog {
    /// Een leeg log.
    pub fn new() -> Self {
        Self::default()
    }

    /// Leg één cross-cel-contact vast.
    ///
    /// Geeft niets terug en kan niet falen: een meetinstrument dat een run kan
    /// laten struikelen, meet de run niet meer.
    pub fn record(&mut self, answer: &SignedAnswer) {
        self.entries.push(answer.clone());
    }

    /// Alle contacten, in de volgorde waarin ze plaatsvonden.
    pub fn entries(&self) -> &[SignedAnswer] {
        &self.entries
    }

    /// Het aantal vastgelegde contacten.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Is er geen enkel contact vastgelegd?
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Leesbaar verslag: één regel per contact, met alles wat er vastgelegd is.
    ///
    /// De ondertekening staat erbij, inclusief dat ze gesimuleerd is. Wie dat
    /// weglaat, leest later een log waarin een placeholder op bewijs lijkt.
    pub fn report(&self) -> String {
        let mut out = String::from("observatielog (meetinstrument, geen productiepad):\n");
        for entry in &self.entries {
            let params = entry
                .params
                .iter()
                .map(|(name, value)| format!("{name}={value}"))
                .collect::<Vec<_>>()
                .join(", ");
            let answer = match &entry.answer.outcome {
                LexostatusOutcome::Established(values) => values
                    .iter()
                    .map(|(name, value)| format!("{name}={value}"))
                    .collect::<Vec<_>>()
                    .join(", "),
                LexostatusOutcome::NotEstablished { reason } => {
                    format!("niets vastgesteld: {reason}")
                }
            };
            let _ = writeln!(
                out,
                "  {} vroeg {}.{} op {} ({params}) -> {answer} [{}]",
                entry.asked_by,
                entry.answer.cell,
                entry.answer.name,
                entry.answer.op_moment,
                entry.signature,
            );
        }
        out
    }
}
