//! Het portaal: de wereld gezien door één aanvrager.
//!
//! De rest van deze crate laat de wereld zien zoals een meetinstrument haar ziet:
//! alle cellen, alle kronieken, het journaal. Een aanvrager ziet dat niet. Zij
//! ziet wat zíj kan doen, en wat de cellen over háár publiceren. Het portaal is
//! de beschrijving van die blik, en het staat in het wereldbestand:
//!
//! - **`actor`**: de cel namens wie het portaal werkt. Alleen haar acties staan
//!   op de pagina.
//! - **`personas`**: fictieve aanvragers om uit te kiezen. Kiezen is een
//!   mock-login zonder authenticatie. Een persona draagt waarden voor de velden
//!   van de formulieren van de actor, en die winnen van de gewone voorinvulling
//!   (zie [`crate::World::choose_persona`]).
//! - **`inzicht`**: welke lexostatussen de pagina "inzicht in je aanvraag"
//!   bevraagt, met parameters als sjabloon over de waarden van de persona.
//!
//! **Waarom in het wereldbestand en niet in een regeling.** Wie er aanvraagt en
//! welke vragen een portaal stelt, is inrichting van deze opstelling en geen
//! recht. Een regeling die een partijnaam of een bsn zou dragen, zou een casus in
//! de wet leggen. En het platform blijft casus-agnostisch: er staat hier geen
//! enkele naam van een aanvrager, alleen de vorm.
//!
//! **Waarom het inzicht bij de consument combineert.** Een cel publiceert over
//! haar eigen feiten, en geen cel kent het totaalbeeld (RFC-022 §2). Een pagina
//! die drie antwoorden van drie cellen naast elkaar zet, doet wat RFC-022 §4.1 een
//! consument toestaat: zij vraagt elke cel apart, langs de publieke ingang, en
//! voegt pas in de weergave samen. Daarom levert deze module de *vragen* en niet
//! de antwoorden: het antwoord hoort van de cel te komen en niet uit een
//! samenvoeging hier.
//!
//! **Niets hiervan legt iets vast.** Een persona kiezen of wisselen is een stand
//! van de sessie en geen gebeurtenis: er komt geen gram, geen journaalregel en
//! geen contact over een celgrens van (zie `tests/invarianten.rs`).

use crate::cell::{
    check_parameter_value, closing_braces_match, Cell, CellConfig, DocumentedParameter, Template,
};
use crate::error::{Result, SimulatorError, Subject};
use regelrecht_engine::Value;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Het `portaal`-blok van een wereldbestand.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortaalDefinition {
    /// De cel namens wie het portaal werkt; haar acties staan op de pagina.
    pub actor: String,
    /// De titel van de pagina. Casusdata.
    pub label: String,
    /// De fictieve aanvragers waaruit gekozen kan worden.
    #[serde(default)]
    pub personas: Vec<Persona>,
    /// Wat de pagina "inzicht in je aanvraag" bevraagt, in volgorde.
    #[serde(default)]
    pub inzicht: Vec<InzichtRegel>,
}

/// Eén fictieve aanvrager.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Persona {
    /// Waarmee zij gekozen wordt; uniek in het portaal.
    pub id: String,
    /// Wat er in de keuzelijst staat.
    pub label: String,
    /// Veldnaam → waarde, voor de formulieren van de acties van de actor.
    ///
    /// Elk veld moet in minstens één van die formulieren voorkomen en de
    /// typetoets van dat veld halen; dat wordt bij het optuigen getoetst.
    #[serde(default)]
    pub values: BTreeMap<String, Value>,
}

/// Eén vraag van de pagina "inzicht in je aanvraag".
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InzichtRegel {
    /// De titel van de kaart.
    pub label: String,
    /// De cel die bevraagd wordt.
    pub cell: String,
    /// De gepubliceerde lexostatus van die cel.
    pub lexostatus: String,
    /// Parameternaam → sjabloon over de waarden van de persona, met
    /// `{veld}`-verwijzingen.
    ///
    /// Precies de gedocumenteerde inputs van de lexostatus. Hetzelfde
    /// sjabloonformaat als het zaakkenmerk van een besluit, en ook dezelfde
    /// lezer: `zorgtoeslag/{bsn}` betekent op beide plekken hetzelfde.
    #[serde(default)]
    pub params: BTreeMap<String, String>,
}

impl InzichtRegel {
    /// De parameters van deze vraag, ingevuld met de waarden van één persona.
    ///
    /// Als tekst: de vraag gaat als queryparameters over de draad, en de route
    /// zet ze om naar het type dat de lexostatus documenteert. Een tweede
    /// omzetting hier zou daarvan kunnen gaan afwijken.
    pub fn params_for(&self, persona: &Persona) -> BTreeMap<String, String> {
        self.params
            .iter()
            .map(|(name, template)| {
                (
                    name.clone(),
                    Template::parse(template).fill(&persona.values),
                )
            })
            .collect()
    }
}

/// Het portaal zoals een frontend het krijgt: `GET /api/portaal`.
#[derive(Debug, Clone, Serialize)]
pub struct PortaalSnapshot {
    /// De cel namens wie het portaal werkt.
    pub actor: String,
    /// De titel van de pagina.
    pub label: String,
    /// De persona's, elk met haar ingevulde vragen.
    pub personas: Vec<PersonaSnapshot>,
    /// De vragen zoals het wereldbestand ze schrijft, met sjablonen.
    pub inzicht: Vec<InzichtRegel>,
}

/// Eén persona, met de vragen van het inzicht al voor haar ingevuld.
///
/// Ingevuld door de server en niet door de pagina: het sjabloon is bij het
/// optuigen tegen de waarden van precies deze persona getoetst, en een tweede
/// invuller in de browser zou dat werk opnieuw moeten doen.
#[derive(Debug, Clone, Serialize)]
pub struct PersonaSnapshot {
    /// Waarmee zij gekozen wordt.
    pub id: String,
    /// Wat er in de keuzelijst staat.
    pub label: String,
    /// Haar waarden voor de formulieren.
    pub values: BTreeMap<String, Value>,
    /// De vragen van het inzicht, in de volgorde van het wereldbestand.
    pub inzicht: Vec<InzichtVraag>,
}

/// Eén vraag aan één cel, klaar om te stellen.
#[derive(Debug, Clone, Serialize)]
pub struct InzichtVraag {
    /// De titel van de kaart.
    pub label: String,
    /// De cel die bevraagd wordt.
    pub cell: String,
    /// De lexostatus.
    pub lexostatus: String,
    /// De ingevulde parameters.
    pub params: BTreeMap<String, String>,
}

impl PortaalDefinition {
    /// De persona met dit id, of de fout die zegt welke er wél zijn.
    pub fn persona(&self, id: &str) -> Result<&Persona> {
        self.personas
            .iter()
            .find(|persona| persona.id == id)
            .ok_or_else(|| SimulatorError::UnknownPersona {
                persona: id.to_string(),
                known: listing(self.personas.iter().map(|persona| persona.id.as_str())),
            })
    }

    /// Het portaal zoals een frontend het krijgt.
    pub fn snapshot(&self) -> PortaalSnapshot {
        PortaalSnapshot {
            actor: self.actor.clone(),
            label: self.label.clone(),
            personas: self
                .personas
                .iter()
                .map(|persona| PersonaSnapshot {
                    id: persona.id.clone(),
                    label: persona.label.clone(),
                    values: persona.values.clone(),
                    inzicht: self
                        .inzicht
                        .iter()
                        .map(|regel| InzichtVraag {
                            label: regel.label.clone(),
                            cell: regel.cell.clone(),
                            lexostatus: regel.lexostatus.clone(),
                            params: regel.params_for(persona),
                        })
                        .collect(),
                })
                .collect(),
            inzicht: self.inzicht.clone(),
        }
    }

    /// Toets het portaal tegen de wereld waarin het staat.
    ///
    /// `forms` zijn de formulieren van de acties van de actor, zoals de wereld ze
    /// aanbiedt. Alles wat hier geweigerd wordt, zou anders stil niets doen: een
    /// persona-veld dat nergens voorkomt vult niets in, en een vraag met de
    /// verkeerde parameters levert op de pagina alleen een fout op.
    pub(crate) fn check(
        &self,
        forms: &[Vec<DocumentedParameter>],
        configs: &[CellConfig],
        cells: &BTreeMap<String, Cell>,
    ) -> Result<()> {
        if !cells.contains_key(&self.actor) {
            return Err(SimulatorError::PortaalUnknownActor {
                label: self.label.clone(),
                actor: self.actor.clone(),
                known: listing(cells.keys().map(String::as_str)),
            });
        }

        let mut seen = BTreeSet::new();
        for persona in &self.personas {
            if !seen.insert(persona.id.as_str()) {
                return Err(SimulatorError::DuplicatePersona {
                    persona: persona.id.clone(),
                });
            }
            self.check_persona(persona, forms)?;
        }

        for regel in &self.inzicht {
            self.check_inzicht(regel, configs, cells)?;
        }
        Ok(())
    }

    /// Elke waarde van een persona hoort bij een veld van de actor, en haalt de
    /// typetoets van elk veld waarin ze terechtkomt.
    fn check_persona(&self, persona: &Persona, forms: &[Vec<DocumentedParameter>]) -> Result<()> {
        for (field, value) in &persona.values {
            let matching: Vec<&DocumentedParameter> = forms
                .iter()
                .flatten()
                .filter(|param| &param.name == field)
                .collect();
            if matching.is_empty() {
                let known: BTreeSet<&str> = forms
                    .iter()
                    .flatten()
                    .map(|param| param.name.as_str())
                    .collect();
                return Err(SimulatorError::PersonaFieldNotInForm {
                    persona: persona.id.clone(),
                    field: field.clone(),
                    actor: self.actor.clone(),
                    known: listing(known.into_iter()),
                });
            }
            // Dezelfde toets als wat een invuller typt: een waarde die hier
            // doorheen komt, komt bij het versturen ook door.
            for param in matching {
                check_parameter_value(&self.actor, Subject::Persona, &persona.id, param, value)?;
            }
        }
        Ok(())
    }

    /// Een vraag van het inzicht gaat naar een cel die er is, naar een naam die
    /// zij publiceert, met precies haar parameters en sjablonen die bij elke
    /// persona uitkomen.
    fn check_inzicht(
        &self,
        regel: &InzichtRegel,
        configs: &[CellConfig],
        cells: &BTreeMap<String, Cell>,
    ) -> Result<()> {
        let (Some(config), Some(cell)) = (
            configs.iter().find(|config| config.id == regel.cell),
            cells.get(&regel.cell),
        ) else {
            return Err(SimulatorError::InzichtUnknownCell {
                label: regel.label.clone(),
                cell: regel.cell.clone(),
                known: listing(cells.keys().map(String::as_str)),
            });
        };
        let definition = config
            .lexostatus_definitions
            .iter()
            .find(|candidate| candidate.name == regel.lexostatus)
            .ok_or_else(|| SimulatorError::UnknownLexostatus {
                cell: regel.cell.clone(),
                requested: regel.lexostatus.clone(),
                published: cell.published_names().join(", "),
            })?;

        let expected: BTreeSet<&str> = definition
            .inputs
            .iter()
            .map(|input| input.name.as_str())
            .collect();
        let given: BTreeSet<&str> = regel.params.keys().map(String::as_str).collect();
        if expected != given {
            return Err(SimulatorError::InzichtParams {
                label: regel.label.clone(),
                cell: regel.cell.clone(),
                lexostatus: regel.lexostatus.clone(),
                given: listing(given.into_iter()),
                expected: listing(expected.into_iter()),
            });
        }

        for (parameter, template) in &regel.params {
            if !closing_braces_match(template) {
                return Err(SimulatorError::InzichtMalformedTemplate {
                    label: regel.label.clone(),
                    parameter: parameter.clone(),
                    template: template.clone(),
                });
            }
            for reference in Template::parse(template).references() {
                if let Some(persona) = self
                    .personas
                    .iter()
                    .find(|persona| !persona.values.contains_key(reference))
                {
                    return Err(SimulatorError::InzichtUnknownReference {
                        label: regel.label.clone(),
                        parameter: parameter.clone(),
                        reference: reference.to_string(),
                        persona: persona.id.clone(),
                    });
                }
            }
        }
        Ok(())
    }
}

/// Een komma-gescheiden opsomming voor een melding; "geen" als ze leeg is.
fn listing<'a>(items: impl Iterator<Item = &'a str>) -> String {
    let joined = items.collect::<Vec<_>>().join(", ");
    if joined.is_empty() {
        "geen".to_string()
    } else {
        joined
    }
}
