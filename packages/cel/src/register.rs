//! Registers als bron van het beleid dat ze bevraagt (notitie "bron en
//! gram-id", 29-09-2026).
//!
//! Een gegeven van buiten volgt vier lagen: een wet stelt het register in en
//! zegt wat erin komt (`vestigt`), een wet geeft de afnemer de grondslag om
//! op te vragen, en het beleid van de beheerder geeft het register een naam
//! en beschrijft de bevraging, als artikel met een invoer zonder bron
//! (`source: {}`). De afnemer haalt het gegeven op met een gewone `source`
//! naar dat beleid. Welk systeem het register levert (de kroniek van een
//! cel, een oude API), zegt alleen de deployment, in een koppelbestand
//! (`CEL_REGISTERS`):
//!
//! ```yaml
//! registers:
//!   <beleid>#<naam van het register>: {cel: <cel-id>, kroniek: <kroniek>}
//! ```
//!
//! De runtime registreert per regel een [`Registerbron`]: een databron met
//! `law_scope` het beleid, die de invoer zonder bron van dat beleid vult met
//! de grammen van de kroniek, zoals ze nu vastliggen. Opstartcontrole: elk
//! beleid met zo'n invoer heeft een bron, elke bron een beleid met zo'n
//! invoer, een cel en een kroniek van die cel; noemt het beleid de naam van
//! het register (uitkomst `naam_register`), dan is dat de naam in de
//! koppeling.
//!
//! Een proef (een handeling die nog niet vastligt) telt mee zoals bij een
//! reductie op proef: [`met_proef`] zet het gram van het concept erbij, voor
//! de duur van een engine-run op deze draad.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use regelrecht_engine::{DataSource, LawExecutionService, Value as EngineValue};
use serde::Deserialize;

use crate::gram::Gram;
use crate::kroniek::Kroniek;
use crate::laden;
use crate::lexostatus_engine;

/// De uitkomst waarmee een beleid de naam van zijn register noemt.
pub const NAAM_REGISTER: &str = "naam_register";

thread_local! {
    /// De grammen van een proef, voor de duur van [`met_proef`].
    static PROEF: RefCell<Vec<Gram>> = const { RefCell::new(Vec::new()) };
}

/// Voer `f` uit met `grammen` erbij in elke registerbron: het concept van
/// een handeling op proef telt mee alsof het vastlag. Alleen op deze draad,
/// en alleen tijdens `f` (een engine-run is synchroon).
/// Ook na een panic in `f` gaat de overlay eraf: de draad dient daarna een
/// ander verzoek.
pub fn met_proef<T>(grams: Vec<Gram>, f: impl FnOnce() -> T) -> T {
    struct Terug(Option<Vec<Gram>>);
    impl Drop for Terug {
        fn drop(&mut self) {
            let oud = self.0.take().unwrap_or_default();
            PROEF.with(|p| *p.borrow_mut() = oud);
        }
    }
    let _terug = Terug(Some(
        PROEF.with(|p| std::mem::replace(&mut *p.borrow_mut(), grams)),
    ));
    f()
}

/// Een regel van het koppelbestand.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Koppeling {
    pub cell: String,
    pub chronicle: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Bestand {
    registers: BTreeMap<String, Koppeling>,
}

/// Een register als databron: de kroniek van een cel, voor de invoer zonder
/// bron van een beleid.
pub struct Registerbron {
    name: String,
    beleid: String,
    field: String,
    chronicle: String,
    source: Arc<OnceLock<Arc<Kroniek>>>,
}

impl DataSource for Registerbron {
    fn name(&self) -> &str {
        &self.name
    }
    fn priority(&self) -> i32 {
        10
    }
    fn source_type(&self) -> &str {
        "register"
    }
    fn has_field(&self, field: &str) -> bool {
        field == self.field
    }
    fn get(&self, field: &str, _criteria: &BTreeMap<String, EngineValue>) -> Option<EngineValue> {
        if field != self.field {
            return None;
        }
        let chronicle = self.source.get()?;
        let vast = chronicle.lees(&self.chronicle).ok()?;
        let trial: Vec<Gram> = PROEF.with(|p| p.borrow().clone());
        let grams = vast
            .iter()
            .map(|v| &v.gram)
            .chain(trial.iter().filter(|g| g.chronicle == self.chronicle));
        let list = lexostatus_engine::als_kroniek(grams, &self.chronicle).ok()?;
        Some(EngineValue::from(&list))
    }
    fn fields(&self) -> Vec<&str> {
        vec![self.field.as_str()]
    }
    fn law_scope(&self) -> Option<&str> {
        Some(&self.beleid)
    }
    fn key_fields(&self) -> Option<&[String]> {
        Some(&[])
    }
}

/// De plek waar een registerbron haar kroniek krijgt.
type Slot = Arc<OnceLock<Arc<Kroniek>>>;

/// De registers van een deployment, nadat ze als bron in het corpus staan;
/// de kronieken komen erbij als de cellen hun kroniek openen
/// ([`Registers::open`]).
#[derive(Default)]
pub struct Registers {
    koppelingen: Vec<(String, Koppeling, Slot)>,
}

/// De invoer zonder bron (`source: {}`) van een regeling: de namen.
fn registerinvoer(service: &LawExecutionService, regulation: &str) -> Vec<String> {
    let Some(law) = service.resolver().get_law(regulation) else {
        return Vec::new();
    };
    let mut uit: Vec<String> = law
        .articles
        .iter()
        .filter_map(|a| a.get_execution_spec())
        .flat_map(|e| e.input.iter().flatten())
        .filter(|i| {
            i.source
                .as_ref()
                .is_some_and(|s| s.regulation.is_none() && s.output.is_none())
        })
        .map(|i| i.name.clone())
        .collect();
    uit.sort();
    uit.dedup();
    uit
}

/// De regelingen van het corpus met een invoer zonder bron: een register dat
/// de deployment moet koppelen.
fn beleid_met_register(service: &LawExecutionService) -> Vec<String> {
    service
        .resolver()
        .list_laws()
        .into_iter()
        .filter(|id| !registerinvoer(service, id).is_empty())
        .map(str::to_string)
        .collect()
}

/// Lees het koppelbestand (`None`: geen), zet per register een bron in het
/// corpus, en controleer het tegen het corpus. `datum` is de dag waarop de
/// naam van het register uit het beleid gelezen wordt.
pub fn laad(
    path: Option<&Path>,
    service: &mut LawExecutionService,
    date: &str,
) -> Result<Registers, Vec<String>> {
    let bestand = match path {
        Some(p) => laden::laad(p, laden::yaml::<Bestand>)?,
        None => Bestand {
            registers: BTreeMap::new(),
        },
    };
    let source = path.map_or("CELL_REGISTERS".to_string(), |p| p.display().to_string());
    let mut fouten = Vec::new();
    let mut registers = Registers::default();
    let mut gekoppeld: Vec<String> = Vec::new();
    for (sleutel, k) in bestand.registers {
        let Some((beleid, name)) = sleutel.split_once('#') else {
            fouten.push(format!(
                "{source}: register '{sleutel}' is niet <beleid>#<naam>"
            ));
            continue;
        };
        if service.resolver().get_law(beleid).is_none() {
            fouten.push(format!(
                "{source}: register '{sleutel}': geen regeling '{beleid}' in het corpus"
            ));
            continue;
        }
        let input = registerinvoer(service, beleid);
        let field = match &input[..] {
            [v] => v.clone(),
            [] => {
                fouten.push(format!(
                    "{source}: register '{sleutel}': '{beleid}' heeft geen invoer zonder bron (source: {{}}); het bevraagt geen register"
                ));
                continue;
            }
            meer => {
                fouten.push(format!(
                    "{source}: register '{sleutel}': '{beleid}' heeft meer invoer zonder bron ({}); een beleid bevraagt hier een register",
                    meer.join(", ")
                ));
                continue;
            }
        };
        // Noemt het beleid de naam van het register, dan is dat de naam hier.
        if service
            .get_law_info(beleid)
            .is_some_and(|i| i.outputs.iter().any(|o| o == NAAM_REGISTER))
        {
            let e =
                crate::toets::evalueer(service, beleid, &[NAAM_REGISTER], &BTreeMap::new(), date);
            match e.waarden.get(NAAM_REGISTER).and_then(serde_json::Value::as_str) {
                Some(n) if n == name => {}
                Some(n) => fouten.push(format!(
                    "{source}: register '{sleutel}': het beleid noemt het register '{n}', niet '{name}'"
                )),
                None => fouten.push(format!(
                    "{source}: register '{sleutel}': de naam van het register is uit '{beleid}' niet te lezen ({})",
                    e.reason("geen naam")
                )),
            }
        }
        if gekoppeld.contains(&beleid.to_string()) {
            fouten.push(format!("{source}: '{beleid}' heeft meer dan een register"));
            continue;
        }
        gekoppeld.push(beleid.to_string());
        let slot = Arc::new(OnceLock::new());
        service.add_data_source(Box::new(Registerbron {
            name: format!("register:{sleutel}"),
            beleid: beleid.to_string(),
            field,
            chronicle: k.chronicle.clone(),
            source: slot.clone(),
        }));
        registers.koppelingen.push((sleutel, k, slot));
    }
    for beleid in beleid_met_register(service) {
        if !gekoppeld.contains(&beleid) {
            fouten.push(format!(
                "{source}: '{beleid}' bevraagt een register (een invoer zonder bron), maar geen register koppelt het aan een cel of adapter"
            ));
        }
    }
    if fouten.is_empty() {
        Ok(registers)
    } else {
        Err(fouten)
    }
}

impl Registers {
    /// Controleer de cellen en kronieken van de koppelingen, voor de
    /// kronieken open zijn.
    pub fn controleer(&self, cells: &[(&str, Vec<&str>)]) -> Vec<String> {
        let mut fouten = Vec::new();
        for (sleutel, k, _) in &self.koppelingen {
            match cells.iter().find(|(id, _)| *id == k.cell) {
                None => fouten.push(format!(
                    "register '{sleutel}': cel '{}' draait niet in deze runtime",
                    k.cell
                )),
                Some((_, chronicles)) if !chronicles.contains(&k.chronicle.as_str()) => fouten
                    .push(format!(
                        "register '{sleutel}': cel '{}' heeft geen kroniek '{}'",
                        k.cell, k.chronicle
                    )),
                Some(_) => {}
            }
        }
        fouten
    }

    /// Geef elke bron de kroniek van haar cel.
    pub fn open(&self, cell: &str, chronicle: &Arc<Kroniek>) {
        for (_, k, slot) in &self.koppelingen {
            if k.cell == cell {
                let _ = slot.set(chronicle.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trial() -> usize {
        PROEF.with(|p| p.borrow().len())
    }

    /// De overlay gaat eraf, ook als de engine-run in paniek raakt: het
    /// volgende verzoek op deze draad ziet het concept niet als vastgelegd.
    #[test]
    fn een_proef_gaat_eraf_ook_na_een_panic() {
        let g = crate::gram::testgram("00000000-0000-4000-8000-000000000001");
        assert_eq!(met_proef(vec![g.clone()], trial), 1);
        assert_eq!(trial(), 0);
        let uit = std::panic::catch_unwind(|| met_proef(vec![g], || panic!("engine")));
        assert!(uit.is_err());
        assert_eq!(trial(), 0);
    }
}
