//! Het beeld van de wereld: één doorsnede van alles wat er staat.
//!
//! Dit is het contract naar buiten. Een frontend — of wat er ook aan de andere
//! kant van een HTTP-laag zit — hoeft precies dit te kennen: waar de klok staat,
//! welke instellingen gelden en welke daarvan al vastliggen, wat er per cel in
//! welke kroniek ligt, welke acties er nu te doen zijn en met welk formulier, wat
//! er over een celgrens ging, en welke termijnen verstreken zonder dat het feit
//! er lag.
//!
//! **Geen casusnamen.** Er staat geen enkele naam in dit bestand die bij één
//! casus hoort. Elk label komt uit het wereldbestand, en daarmee is een andere
//! casus een ander bestand en geen andere frontend.
//!
//! **Een inspectiebeeld, geen weg naar een cel.** De wereld bezit de cellen en
//! zij maakt dit beeld; een cel kan het niet opvragen en kan er dus ook niet de
//! kroniek van een ander mee lezen. Dat is dezelfde keuze als bij het
//! meetinstrument naast de opstelling: het beeld ziet veel, en juist daarom is er
//! geen productiepad in een cel dat eraan kan komen. Het is passief — opvragen
//! verandert niets — en het rekent niets uit: alles wat hier staat, ligt ergens in
//! een cel, en niets bestaat alleen hier.
//!
//! **Het receipt gaat niet mee.** Een decretogram draagt het volledige RFC-013
//! Execution Receipt, en dat draagt wandkloktijd. Een beeld dat per run verschilt
//! is geen contract, dus het receipt blijft in de kroniek waar het hoort. Wat een
//! lezer van het receipt nodig heeft — de herkomst van elke waarde waarop besloten
//! is — staat er wel, per veld.

use crate::cell::{
    fixed_fields, Cell, ChronicleEvent, DocumentedParameter, Intake, Lexostatus, INPUTS, RECEIPT,
    REGULATION,
};
use crate::security::SignedAnswer;
use crate::world::{ActionDefinition, ActionEffect, Warning};
use chrono::NaiveDate;
use regelrecht_engine::Value;
use serde::Serialize;
use std::collections::BTreeMap;

/// Alles wat er nu in de wereld staat.
#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    /// Waar de logische klok staat.
    pub clock: NaiveDate,
    /// De instellingen van deze wereld.
    pub settings: BTreeMap<String, Value>,
    /// De instellingen die al door een besluit gebruikt zijn en daarmee vast
    /// staan, met het besluit dat ze gebruikte.
    ///
    /// Zodat een lezer kan zien welke knop nog om kan en welke niet, zonder het te
    /// hoeven proberen. Zie [`crate::World::update_settings`].
    pub locked_settings: BTreeMap<String, LockedSetting>,
    /// De cellen, met hun kronieken.
    pub cells: Vec<CellSnapshot>,
    /// Elke actie uit het wereldbestand, met haar formulier en of ze nu kan.
    pub actions: Vec<ActionSnapshot>,
    /// Elk contact dat over een celgrens ging, in volgorde.
    ///
    /// Dit is het materiaal van het observatielog — het meetinstrument dat buiten
    /// de band staat. Het staat hier omdat een besluit zijn contacten aan de
    /// wereld teruggeeft en een beeld van de wereld ze hoort te kunnen tonen. Een
    /// lezer hoort het als meetinstrument te labelen en niet als onderdeel van de
    /// opstelling: de houder van deze lijst kent de unie van wat over de grenzen
    /// ging, en dat is precies het totaalbeeld waarvan geen enkele cel er een
    /// heeft.
    pub crossings: Vec<CrossingSnapshot>,
    /// De termijnen die verstreken zonder dat het feit er lag.
    pub warnings: Vec<Warning>,
}

/// Een instelling die vast staat, en waardoor.
#[derive(Debug, Clone, Serialize)]
pub struct LockedSetting {
    /// De cel die het besluit nam.
    pub cell: String,
    /// Het besluit dat de instelling gebruikte.
    pub besluit: String,
}

/// Eén cel, zoals ze erbij staat.
#[derive(Debug, Clone, Serialize)]
pub struct CellSnapshot {
    /// Het cel-id.
    pub id: String,
    /// De regelingen die deze cel laadt, bij `$id`.
    ///
    /// Leeg is een **bron-cel**: ze legt vast en reduceert, en heeft geen engine.
    /// Dat is te zien, en dat hoort ook: het is de toets dat het
    /// lexostatus-contract engine-onafhankelijk is.
    pub laws: Vec<String>,
    /// De namen die deze cel publiceert.
    pub lexostatussen: Vec<String>,
    /// De besluiten die deze cel kan nemen.
    pub besluiten: Vec<String>,
    /// De kronieken van deze cel, met elk gram dat erin ligt.
    pub chronicles: Vec<ChronicleSnapshot>,
}

/// Eén kroniekstroom met haar grammen.
#[derive(Debug, Clone, Serialize)]
pub struct ChronicleSnapshot {
    /// Naam van de stroom.
    pub stream: String,
    /// Het veld waarop de stroom groepeert.
    pub key: String,
    /// De grammen, in de volgorde waarin ze vastgelegd zijn.
    pub grams: Vec<GramSnapshot>,
}

/// De drie chronolexogrammen van RFC-022 §1.
///
/// Een kroniek draagt er twee: wat een cel overkwam
/// ([`Self::Executogram`]) en wat ze zelf besloot ([`Self::Decretogram`]). Het
/// **lexogram** is de wet zelf — generiek, compile-time, en van niemand in
/// bijzonder — en die ligt dus in geen enkele kroniek; welk recht een cel laadt,
/// staat in [`CellSnapshot::laws`]. De variant staat er zodat een lezer één
/// vocabulaire voor alle drie heeft en het derde gram niet stil ontbreekt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GramKind {
    /// Een regel: de wet, generiek en niet van een cel.
    Lexogram,
    /// Een besluit dat de cel zelf nam.
    Decretogram,
    /// Een feit dat de cel overkwam: een aanvraag, een levering, een betaling.
    Executogram,
}

/// Eén gram in een kroniek.
#[derive(Debug, Clone, Serialize)]
pub struct GramSnapshot {
    /// Welk van de drie grammen dit is.
    pub kind: GramKind,
    /// Wat er gebeurde, in de woorden van de cel.
    pub name: String,
    /// Het kanaal waarlangs het feit de cel bereikte.
    pub intake: Intake,
    /// Wie vastlegde: het cel-id.
    pub recording_actor: String,
    /// Op welke grondslag; mag leeg zijn.
    pub grondslag: String,
    /// Het moment waarop dit feit in deze cel feit werd.
    pub op_moment: NaiveDate,
    /// De velden, elk met de herkomst van hun waarde.
    pub fields: BTreeMap<String, FieldSnapshot>,
}

/// Eén veld van een gram: de waarde, en waar ze vandaan komt.
#[derive(Debug, Clone, Serialize)]
pub struct FieldSnapshot {
    /// De vastgelegde waarde.
    pub value: Value,
    /// Waar ze vandaan komt.
    pub origin: FieldOrigin,
}

/// Waar de waarde van één veld vandaan komt.
///
/// Bij een executogram is dat altijd hetzelfde: de cel legde het vast, langs een
/// kanaal en op een grondslag. Bij een decretogram valt het uiteen, en dat is het
/// hele punt van invariant I5 — een waarde die van een ander **geaccepteerd** is,
/// hoort niet op een berekende waarde te lijken.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "herkomst", rename_all = "snake_case")]
pub enum FieldOrigin {
    /// De cel legde dit feit zelf vast.
    Recorded {
        /// Het kanaal waarlangs het binnenkwam.
        intake: Intake,
        /// De grondslag van de vastlegging; mag leeg zijn.
        grondslag: String,
    },
    /// Een input waarop het besluit rekende, met de herkomst die het gram
    /// vastlegde: een eigen kroniek, een parameter, of geaccepteerd van een
    /// andere cel — met bron, naam, moment en ondertekening.
    ///
    /// Doorgegeven zoals het gram het opschreef en niet naverteld: wat een lezer
    /// hier ziet, is wat er in de kroniek staat.
    BesluitInput {
        /// De herkomst zoals het decretogram haar vastlegde.
        recorded_origin: Value,
    },
    /// Een uitkomst die de regeling van dit besluit berekende.
    Computed {
        /// De uitgevoerde regeling, bij `$id`.
        regulation: String,
    },
    /// Een vast veld van het decretogram zelf: het zaakkenmerk, de regeling, het
    /// bevoegd gezag, het schema van de verplichtingen.
    Besluit,
}

/// Eén actie, met haar formulier en of ze nu kan.
#[derive(Debug, Clone, Serialize)]
pub struct ActionSnapshot {
    /// Waarmee de actie aangeroepen wordt.
    pub id: String,
    /// De cel die haar doet.
    pub actor: String,
    /// Wat een lezer ziet. Casusdata.
    pub label: String,
    /// Vrije toelichting uit het wereldbestand.
    pub doc: Option<String>,
    /// Wat de actie uitwerkt.
    pub effect: ActionEffectSnapshot,
    /// Het formulier: wat de actor invult, en van welk type.
    pub form: Vec<DocumentedParameter>,
    /// Kan de actie nu?
    pub available: bool,
    /// Waarom ze nu niet kan; `None` als ze kan.
    ///
    /// Een actie die niet kan blijft in de lijst staan, met de reden erbij. Wie
    /// alleen de mogelijke acties toont, laat een keuze verdwijnen zonder te
    /// zeggen waarom — en dan is niet te zien waar het verhaal staat.
    pub unavailable_reason: Option<String>,
}

/// Wat een actie uitwerkt, zoals een lezer het ziet.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "soort", rename_all = "snake_case")]
pub enum ActionEffectSnapshot {
    /// De actor legt een feit vast, en levert het eventueel aan een ander.
    Records {
        /// De cel waarin het gram landt.
        cell: String,
        /// De kroniekstroom.
        chronicle: String,
        /// Hoe het gram heet.
        name: String,
        /// De cel die hetzelfde feit als levering krijgt, als die er is.
        delivers_to: Option<String>,
    },
    /// De actor start het besluit-pad van een cel.
    Decides {
        /// De cel die besluit.
        cell: String,
        /// De besluit-definitie.
        besluit: String,
    },
}

/// Eén contact over een celgrens, zoals het meetinstrument het ziet.
#[derive(Debug, Clone, Serialize)]
pub struct CrossingSnapshot {
    /// De identiteit die vroeg en ondertekende.
    pub asked_by: String,
    /// De (gesimuleerde) ondertekening. Dat ze gesimuleerd is, staat in de
    /// waarde: wie dat weglaat, leest later een placeholder als bewijs.
    pub signature: String,
    /// De parameters waarmee gevraagd is.
    pub params: BTreeMap<String, Value>,
    /// Het antwoord van de bevraagde cel, zoals zij het gaf.
    pub answer: Lexostatus,
}

/// Eén actie met wat de wereld erover weet.
///
/// Het formulier en de beschikbaarheid staan niet in de actie zelf: het eerste
/// kan van een besluit-definitie komen, het tweede hangt van de stand van de klok
/// af. Beide weet alleen de wereld, en ze horen bij elkaar in het beeld.
pub(crate) struct ActionState<'a> {
    /// De actie uit het wereldbestand.
    pub(crate) action: &'a ActionDefinition,
    /// Wat de actor invult, en van welk type.
    pub(crate) form: Vec<DocumentedParameter>,
    /// Waarom ze nu niet kan; `None` als ze kan.
    pub(crate) unavailable: Option<String>,
}

/// Alles wat de wereld aan het beeld meegeeft, geleend.
///
/// Eén parameter en geen zeven: wat hier binnenkomt is de stand van één wereld,
/// en die hoort als één ding door te komen.
pub(crate) struct WorldView<'a> {
    /// Waar de logische klok staat.
    pub(crate) clock: NaiveDate,
    /// De instellingen die nu gelden.
    pub(crate) settings: &'a BTreeMap<String, Value>,
    /// De instellingen die vast staan, met cel en besluit dat ze gebruikte.
    pub(crate) used_settings: &'a BTreeMap<String, (String, String)>,
    /// De cellen van deze wereld.
    pub(crate) cells: &'a BTreeMap<String, Cell>,
    /// De acties, met hun formulier en beschikbaarheid.
    pub(crate) actions: Vec<ActionState<'a>>,
    /// Wat er over een celgrens ging, in volgorde.
    pub(crate) crossings: &'a [SignedAnswer],
    /// De termijnen die verstreken zonder dat het feit er lag.
    pub(crate) warnings: &'a [Warning],
}

/// Bouw het beeld van de wereld.
///
/// `pub(crate)`: de weg hiernaartoe is [`crate::World::snapshot`]. Dit is geen
/// tweede ingang naar de cellen — de aanroeper moet ze al bezitten om ze hier te
/// kunnen lenen.
pub(crate) fn build(view: &WorldView<'_>) -> Snapshot {
    Snapshot {
        clock: view.clock,
        settings: view.settings.clone(),
        locked_settings: view
            .used_settings
            .iter()
            .map(|(setting, (cell, besluit))| {
                (
                    setting.clone(),
                    LockedSetting {
                        cell: cell.clone(),
                        besluit: besluit.clone(),
                    },
                )
            })
            .collect(),
        cells: view.cells.values().map(cell_snapshot).collect(),
        actions: view.actions.iter().map(action_snapshot).collect(),
        crossings: view.crossings.iter().map(crossing_snapshot).collect(),
        warnings: view.warnings.to_vec(),
    }
}

/// Eén cel, met haar kronieken.
fn cell_snapshot(cell: &Cell) -> CellSnapshot {
    CellSnapshot {
        id: cell.id().to_string(),
        laws: cell.laws().to_vec(),
        lexostatussen: cell
            .published_names()
            .into_iter()
            .map(str::to_string)
            .collect(),
        besluiten: cell
            .besluit_names()
            .into_iter()
            .map(str::to_string)
            .collect(),
        chronicles: cell
            .inspect()
            .into_iter()
            .map(|view| ChronicleSnapshot {
                stream: view.stream.to_string(),
                key: view.key.to_string(),
                grams: view.events.iter().map(gram_snapshot).collect(),
            })
            .collect(),
    }
}

/// Eén gram, met de herkomst van elke waarde erbij.
fn gram_snapshot(event: &ChronicleEvent) -> GramSnapshot {
    // Een decretogram ontstaat langs precies één weg — de cel besluit zelf — en
    // draagt dat in zijn kanaal. Daarmee is het kanaal ook wat de twee soorten
    // gram uit elkaar houdt, en niet een tweede veld dat ermee uit de pas kan
    // lopen.
    let kind = match event.intake {
        Intake::EigenBesluit => GramKind::Decretogram,
        Intake::Aanvraag | Intake::Levering | Intake::Betaling => GramKind::Executogram,
    };

    // Een gram dat langs het besluit-pad ontstond, draagt zijn receipt. Een cel
    // zonder engine kan óók een eigen vaststelling in haar kroniek leggen — een
    // bron-cel die opschrijft wat zij vaststelde — en dat gram draagt geen inputs,
    // geen regeling en geen receipt. Het is even goed een decretogram, maar de
    // herkomst van elke waarde erin is de vastlegging zelf en niet een uitvoering
    // die nooit gedraaid heeft.
    let from_besluit_path = kind == GramKind::Decretogram && event.fields.contains_key(RECEIPT);
    let recorded = || FieldOrigin::Recorded {
        intake: event.intake,
        grondslag: event.grondslag.clone(),
    };
    let regulation = event
        .fields
        .get(REGULATION)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    let mut fields: BTreeMap<String, FieldSnapshot> = event
        .fields
        .iter()
        .filter(|(name, _)| !(from_besluit_path && omitted(name)))
        .map(|(name, value)| {
            let origin = match from_besluit_path {
                false => recorded(),
                true if fixed_fields().contains(&name.as_str()) => FieldOrigin::Besluit,
                true => FieldOrigin::Computed {
                    regulation: regulation.clone(),
                },
            };
            (
                name.clone(),
                FieldSnapshot {
                    value: value.clone(),
                    origin,
                },
            )
        })
        .collect();

    // De inputs van een besluit staan in het gram in één veld bij elkaar. Ze
    // komen hier los te staan, elk met haar eigen herkomst, want dat is wat een
    // lezer per waarde wil zien — en het veld waarin ze samen zaten, verdwijnt
    // daarmee (zie [`omitted`]). Ze gaan er ná de eigen velden in: zou een input
    // en een uitkomst dezelfde naam dragen, dan is de rijkere herkomst de juiste.
    if from_besluit_path {
        for (name, input) in besluit_inputs(&event.fields) {
            fields.insert(
                name,
                FieldSnapshot {
                    value: input.value.unwrap_or(Value::Null),
                    origin: FieldOrigin::BesluitInput {
                        recorded_origin: input.origin,
                    },
                },
            );
        }
    }

    GramSnapshot {
        kind,
        name: event.name.clone(),
        intake: event.intake,
        recording_actor: event.recording_actor.clone(),
        grondslag: event.grondslag.clone(),
        op_moment: event.op_moment,
        fields,
    }
}

/// Blijft dit veld van een gram uit het besluit-pad buiten het beeld?
///
/// Twee velden, elk om een eigen reden: het **receipt** draagt wandkloktijd en zou
/// het beeld per run laten verschillen, en het veld met de **inputs** gaat uit
/// elkaar in losse velden met hun herkomst — het twee keer meesturen zou dezelfde
/// waarden twee keer in beeld brengen.
fn omitted(name: &str) -> bool {
    name == RECEIPT || name == INPUTS
}

/// Eén input van een besluit, zoals het gram haar draagt.
struct RecordedInput {
    /// De waarde waarop gerekend is.
    value: Option<Value>,
    /// De herkomst, zoals het gram haar opschreef.
    origin: Value,
}

/// De inputs van een decretogram, uit het veld dat ze draagt.
///
/// Uit het gram gelezen en niet uit een tweede bron: wat hier uitkomt is precies
/// wat er in de kroniek staat. Een gram zonder dit veld — of met iets anders erin
/// dan verwacht — levert een lege lijst op, en dan staan de uitkomsten er nog
/// steeds; een beeld hoort niet om te vallen omdat een herkomst niet te lezen was.
fn besluit_inputs(fields: &BTreeMap<String, Value>) -> BTreeMap<String, RecordedInput> {
    let Some(Value::Object(inputs)) = fields.get(INPUTS) else {
        return BTreeMap::new();
    };
    inputs
        .iter()
        .filter_map(|(name, entry)| {
            let Value::Object(parts) = entry else {
                return None;
            };
            Some((
                name.clone(),
                RecordedInput {
                    value: parts.get("value").cloned(),
                    origin: parts.get("origin").cloned().unwrap_or(Value::Null),
                },
            ))
        })
        .collect()
}

/// Eén actie, met haar formulier en of ze nu kan.
fn action_snapshot(state: &ActionState<'_>) -> ActionSnapshot {
    let action = state.action;
    let effect = match &action.effect {
        ActionEffect::Records(records) => ActionEffectSnapshot::Records {
            cell: records.cell.clone(),
            chronicle: records.chronicle.clone(),
            name: records.name.clone(),
            delivers_to: records
                .delivers_to
                .as_ref()
                .map(|delivery| delivery.cell.clone()),
        },
        ActionEffect::Decides(decides) => ActionEffectSnapshot::Decides {
            cell: decides.cell.clone(),
            besluit: decides.besluit.clone(),
        },
    };
    ActionSnapshot {
        id: action.id.clone(),
        actor: action.actor.clone(),
        label: action.label.clone(),
        doc: action.doc.clone(),
        effect,
        form: state.form.clone(),
        available: state.unavailable.is_none(),
        unavailable_reason: state.unavailable.clone(),
    }
}

/// Eén contact over een celgrens.
fn crossing_snapshot(signed: &SignedAnswer) -> CrossingSnapshot {
    CrossingSnapshot {
        asked_by: signed.asked_by.to_string(),
        signature: signed.signature.to_string(),
        params: signed.params.clone(),
        answer: signed.answer.clone(),
    }
}
