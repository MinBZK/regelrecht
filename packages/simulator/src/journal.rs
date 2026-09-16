//! Het journaal: één verhaal in tijdsvolgorde, van alles wat er gebeurde.
//!
//! De kronieken vertellen wat er per cel *ligt*; het journaal vertelt wie er
//! iets deed en wat dat aan de stand veranderde. Eén regel per gebeurtenis, in
//! de volgorde waarin ze ontstond: een actie van een actor, een trigger van de
//! klok (een startstand die passeert, een vervallen verplichting, een verstreken
//! termijn), en elke vraag die over een celgrens ging.
//!
//! **Geen tweede administratie.** Elke regel wijst naar de grammen die erdoor
//! ontstonden — cel, kroniek, gram-id — en draagt zelf geen kopie van hun
//! inhoud. Wat er in een regel staat dat niet in een gram ligt, is precies één
//! ding: het **verschil** in de stand van de zaak, en dat is een meting en geen
//! feit (zie [`StatusChange`]).
//!
//! **Geen casusnamen.** Welke lexostatussen "de stand van de zaak" vormen, staat
//! per cel in het wereldbestand ([`StatusIndicator`]). Een andere casus is een
//! ander bestand en dezelfde Rust.
//!
//! ## De vóór/ná-meting is geen celgrens-verkeer
//!
//! Om een verschil te kunnen tonen reduceert de wereld de betrokken indicatoren
//! vlak vóór en vlak ná de gebeurtenis, op het moment van die gebeurtenis. Dat
//! gebeurt langs [`crate::Cell::reduce`] — de publieke ingang van de cel —
//! buiten de veiligheidscontext en het transport om, precies zoals de
//! voorinvulling van een formulier dat doet (zie [`crate::cell::Prefill`]). Er
//! wordt dus geen vraag over een grens gesteld: er komt geen `crossing` van, er
//! komt geen regel in het observatielog van, en de invarianten-gate ziet er
//! niets van. Dat moet ook: de wereld meet hier haar eigen opstelling, zoals ze
//! ook het beeld van alle kronieken maakt, en een meting die zichzelf als
//! verkeer laat tellen zou het vraaggraf vervuilen met vragen die het recht niet
//! stelt.
//!
//! Alleen de cellen die de gebeurtenis **raakte** worden gereduceerd. Een
//! indicator van een cel waar niets gebeurde zou hetzelfde antwoord twee keer
//! opleveren, en dat is geen verschil; hem toch bevragen zou de opstelling laten
//! rekenen voor een regel die er niet komt.

use crate::cell::{
    Afwijzingsgrond, ExecutedRegulation, InputOrigin, Lexostatus, LexostatusOutcome,
};
use crate::snapshot::{CrossingSnapshot, GramKind};
use chrono::NaiveDate;
use regelrecht_engine::Value;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// Wie of wat een gebeurtenis in gang zette.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "soort", rename_all = "snake_case")]
pub enum JournalActor {
    /// Een actor uit het wereldbestand die een actie deed.
    Actor {
        /// Het actor-id; in deze opstelling altijd een cel-id.
        id: String,
    },
    /// Een cel die uit zichzelf besloot of vastlegde.
    Cell {
        /// Het cel-id.
        id: String,
    },
    /// De logische klok: een startstand die passeert, een termijn die vervalt.
    Klok,
}

impl JournalActor {
    /// Hoe deze actor in een verslag heet.
    pub fn describe(&self) -> &str {
        match self {
            Self::Actor { id } | Self::Cell { id } => id,
            Self::Klok => "klok",
        }
    }
}

/// Wat voor soort gebeurtenis een journaalregel beschrijft.
///
/// De soort staat in de JSON, zodat een lezer ze uit elkaar kan houden zonder
/// ze op hun velden te moeten herkennen — dezelfde keuze als bij
/// [`crate::Warning`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JournalKind {
    /// Er is een feit vastgelegd: een actie van een actor, of een startstand die
    /// de klok passeerde.
    Vastlegging,
    /// Een cel nam een besluit.
    Besluit,
    /// Een termijn van een verplichting verviel en werd nagekomen.
    Betaling,
    /// Een termijn verstreek zonder dat het feit er lag.
    Termijn,
    /// Een vraag die over een celgrens ging.
    Vraag,
}

/// De verwijzing naar één gram dat door een gebeurtenis ontstond.
///
/// Een verwijzing en geen kopie: wat er in het gram staat, staat in de kroniek
/// van de cel die het vastlegde, en het beeld van de wereld draagt het al.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GramRef {
    /// De cel in wiens kroniek het gram landde.
    pub cell: String,
    /// De kroniekstroom.
    pub chronicle: String,
    /// Waaronder dit gram in het beeld terug te vinden is:
    /// `<cel>|<kroniek>|<plek in de kroniek>`.
    ///
    /// De plek in de kroniek en niet een verzonnen sleutel: een kroniek groeit
    /// achteraan en wijzigt nooit, dus die plek is stabiel, en het beeld geeft
    /// de grammen in precies die volgorde.
    pub id: String,
    /// Welk van de drie grammen dit is.
    pub kind: GramKind,
    /// Hoe het gram heet, in de woorden van de cel.
    pub name: String,
}

impl GramRef {
    /// Hoe dit gram in een verslag heet.
    pub fn describe(&self) -> String {
        format!("{}.{} · {}", self.cell, self.chronicle, self.name)
    }
}

/// Eén waarde die een besluit van een andere cel accepteerde.
///
/// Hier en niet alleen in het gram, omdat het journaal het verhaal vertelt: dat
/// een besluit een waarde niet zelf vaststelde maar ophaalde bij degene die dat
/// wel deed, is het verhaal en niet een detail van de herkomst (invariant I5).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AcceptedValue {
    /// De naam waaronder het besluit de waarde gebruikte.
    pub name: String,
    /// De waarde zelf; afwezig als het gram haar niet als input draagt.
    pub value: Option<Value>,
    /// De cel die haar vaststelde.
    pub cell: String,
    /// De lexostatus waarmee ze opgehaald is; afwezig bij een waarde die de
    /// *wet* via een cel-bron haalde (tier 3), want die noemt geen naam.
    pub lexostatus: Option<String>,
    /// Het moment waarop de vaststelling gold.
    pub op_moment: Option<NaiveDate>,
}

impl AcceptedValue {
    /// Leesbare regel voor een verslag.
    pub fn describe(&self) -> String {
        let value = match &self.value {
            Some(value) => format!(" {value}"),
            None => String::new(),
        };
        let moment = match self.op_moment {
            Some(moment) => format!(" (vastgesteld {moment})"),
            None => String::new(),
        };
        format!("accepteerde {}{value} van {}{moment}", self.name, self.cell)
    }
}

/// Wat een besluit uitvoerde: welk recht, waarop, en wat eruit kwam.
///
/// De journaalregel van een besluit noemde tot nu toe wat het besluit *is* — de
/// actie, het gram, de geaccepteerde waarden — en niet wat er gebeurd is: welke
/// regelingen er uitgevoerd zijn, op welke gegevens, en wat ze opleverden. Dat
/// staat wel in het gram, maar dan moet een lezer eerst het gram opzoeken en
/// vervolgens zijn velden uit elkaar halen. Het journaal is de hoofdweergave, en
/// het verhaal van een besluit is de uitvoering ervan.
///
/// **Geen tweede administratie.** Alles hier komt uit het decretogram dat deze
/// regel in [`JournalEntry::grams`] aanwijst, in dezelfde woorden als het gram
/// het opschreef (zie [`ExecutedInput::origin`]) — ook de uitgevoerde
/// regelingen, die het gram onder `executed_regulations` vastlegt. Er wordt
/// niets uitgerekend en er staat niets in wat niet in een cel ligt.
///
/// **Geen wandkloktijd.** Het receipt draagt een tijdstempel van de machine; wat
/// hieruit komt hangt alleen van de kronieken en van het moment van het besluit
/// af, zoals de rest van het beeld (zie [`crate::snapshot`]).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Execution {
    /// Het besluittype van dit besluit: wat het gram onder `decision_type`
    /// draagt, en `None` als de regeling er niets over zegt.
    ///
    /// In het verhaal en niet alleen in het gram, om dezelfde reden als de
    /// geaccepteerde waarden: dat een besluit **afwees** is het verhaal van dat
    /// besluit en geen detail van een veld. Een lezer die alleen de uitkomsten
    /// ziet, leest een weigering als een bedrag dat toevallig nul bleef.
    pub decision_type: Option<String>,
    /// De afwijzingsvoorwaarden die vervuld waren; leeg bij elk ander besluit.
    ///
    /// De motivering van de weigering, in dezelfde woorden als het gram haar
    /// vastlegde: welke uitkomst, welke waarde, welk artikel.
    pub afwijzingsgronden: Vec<Afwijzingsgrond>,
    /// De regelingen die uitgevoerd zijn, met de versie die toen gold. De
    /// regeling van het besluit staat vooraan.
    pub regulations: Vec<ExecutedRegulation>,
    /// De inputs waarop besloten is, elk met haar herkomst.
    pub inputs: Vec<ExecutedInput>,
    /// De uitkomsten die het besluit vastlegde.
    pub outputs: Vec<ExecutedOutput>,
}

impl Execution {
    /// Wees dit besluit af?
    ///
    /// Op de gronden en niet op het type, net als [`Decretogram::is_afwijzing`]:
    /// de reden gaat voor het etiket.
    pub fn is_afwijzing(&self) -> bool {
        !self.afwijzingsgronden.is_empty()
    }

    /// Waarop dit besluit afketste, in één regel; leeg als het niet afwees.
    pub fn describe_afwijzing(&self) -> String {
        self.afwijzingsgronden
            .iter()
            .map(Afwijzingsgrond::describe)
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Welke regelingen er uitgevoerd zijn, in één regel.
    pub fn describe_regulations(&self) -> String {
        self.regulations
            .iter()
            .map(ExecutedRegulation::describe)
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Eén input waarop een besluit rekende, met haar herkomst.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExecutedInput {
    /// De naam waaronder het besluit haar gebruikte.
    pub name: String,
    /// De waarde zoals ze meedeed.
    pub value: Value,
    /// Waar ze vandaan kwam: een parameter, een eigen kroniek, een eerder
    /// besluit, of geaccepteerd van een andere cel — met bron, naam, moment en
    /// ondertekening.
    ///
    /// De herkomst zelf en niet een navertelling ervan, zodat het verslag haar
    /// in woorden kan zeggen ([`InputOrigin::describe`]) en een lezer buiten
    /// deze crate haar per soort kan uitvragen. Naar buiten gaat ze in precies
    /// de vorm waarin het gram haar opschreef (`InputOrigin::as_value`), onder
    /// dezelfde naam als in het beeld van de wereld
    /// ([`crate::snapshot::FieldOrigin::BesluitInput`]): één vocabulaire voor
    /// herkomst in plaats van twee.
    #[serde(rename = "herkomst", serialize_with = "serialize_origin")]
    pub origin: InputOrigin,
}

/// De herkomst naar buiten in de vorm van het gram, en niet in die van `serde`.
///
/// Een afgeleide `Serialize` op [`InputOrigin`] zou een tweede schrijfwijze
/// opleveren naast de ene die het gram en het beeld van de wereld al gebruiken.
fn serialize_origin<S: serde::Serializer>(
    origin: &InputOrigin,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error> {
    origin.as_value().serialize(serializer)
}

impl ExecutedInput {
    /// Leesbare regel voor een verslag.
    ///
    /// Mét de herkomst: een bedrag zonder de plek waar het vandaan komt leest
    /// als iets wat dit besluit zelf vaststelde, en juist bij een waarde die van
    /// een andere organisatie geaccepteerd is, is dat het verschil dat telt
    /// (invariant I5).
    pub fn describe(&self) -> String {
        format!(
            "input {} = {} ({})",
            self.name,
            self.value,
            self.origin.describe()
        )
    }
}

/// Eén uitkomst die een besluit vastlegde.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExecutedOutput {
    /// De naam van de uitkomst, zoals de regeling haar noemt.
    pub name: String,
    /// De waarde waarop de uitvoering uitkwam.
    pub value: Value,
}

impl ExecutedOutput {
    /// Leesbare regel voor een verslag.
    pub fn describe(&self) -> String {
        format!("uitkomst {} = {}", self.name, self.value)
    }
}

/// Wat een gebeurtenis aan de stand van de zaak veranderde, bij één cel.
///
/// `voor` en `na` zijn twee reducties over dezelfde indicator op hetzelfde
/// moment, met de gebeurtenis ertussen. `None` is "niets vastgesteld" en niet
/// "leeg": dat onderscheid is wat deze opstelling maakt, en het hoort hier niet
/// weg te vallen (zie [`LexostatusOutcome`]).
///
/// Een indicator die niet veranderde, komt hier niet in voor: een regel die
/// zegt dat er niets veranderde, is geen verandering.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StatusChange {
    /// De cel wiens stand veranderde.
    pub cell: String,
    /// De gepubliceerde lexostatus die de stand draagt.
    pub lexostatus: String,
    /// Wat een lezer ziet. Casusdata uit het wereldbestand.
    pub label: String,
    /// De stand vóór de gebeurtenis; `None` is "niets vastgesteld".
    pub voor: Option<BTreeMap<String, Value>>,
    /// De stand ná de gebeurtenis; `None` is "niets vastgesteld".
    pub na: Option<BTreeMap<String, Value>>,
}

impl StatusChange {
    /// Leesbare verandering: `label: was → is`.
    pub fn describe(&self) -> String {
        format!(
            "{} · {}: {} → {}",
            self.cell,
            self.label,
            describe_values(self.voor.as_ref()),
            describe_values(self.na.as_ref())
        )
    }
}

/// Hoe de stand van één indicator in woorden heet.
///
/// Eén uitkomst staat er kaal ("25000"), meer uitkomsten staan met hun naam
/// erbij: bij één waarde zou de naam de regel alleen langer maken, en bij twee
/// zou het weglaten ervan niet meer te lezen zijn.
fn describe_values(values: Option<&BTreeMap<String, Value>>) -> String {
    // Een reeks zonder uitkomsten leest als "niets vastgesteld" en niet als een
    // lege regel: er valt niets te tonen, en `voor → ` zou een verandering
    // suggereren waarvan de ene kant wegviel. Dezelfde regel als in de weergave
    // (`describeStand`), want het is dezelfde zin.
    let Some(values) = values.filter(|values| !values.is_empty()) else {
        return "niets vastgesteld".to_string();
    };
    if let Some((_, only)) = values.iter().next().filter(|_| values.len() == 1) {
        return only.to_string();
    }
    values
        .iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join("; ")
}

/// Het antwoord van een andere cel in woorden: wat zij gaf, en van wanneer.
///
/// Hoort bij de vraag-regel: die noemde tot nu toe alleen wát er gevraagd is, en
/// een vraag zonder antwoord is een half verhaal. Wáárop dat antwoord berust
/// staat in de uitleg die het antwoord zelf draagt ([`Lexostatus::reductie`]);
/// die wordt hier niet uitgeschreven — het verslag zou er een uitdraai van
/// worden, en het beeld draagt haar voor wie haar wil nalopen.
fn describe_answer(answer: &Lexostatus) -> String {
    let stand = match &answer.outcome {
        LexostatusOutcome::Established(values) => describe_values(Some(values)),
        LexostatusOutcome::NotEstablished { .. } => "niets vastgesteld".to_string(),
    };
    format!(
        "{} van {} op {}: {stand}",
        answer.name, answer.cell, answer.op_moment
    )
}

/// Eén regel in het journaal: één gebeurtenis, in de volgorde van ontstaan.
#[derive(Debug, Clone, Serialize)]
pub struct JournalEntry {
    /// De plek in het journaal, vanaf 0. Waarnaar [`Self::parent`] verwijst.
    pub seq: usize,
    /// Het moment in de logische tijd waarop dit gebeurde.
    pub moment: NaiveDate,
    /// Wie of wat het in gang zette.
    pub actor: JournalActor,
    /// Wat voor soort gebeurtenis dit is.
    pub kind: JournalKind,
    /// Korte omschrijving, in de woorden van het wereldbestand.
    pub description: String,
    /// De grammen die hierdoor ontstonden; leeg als er niets vastgelegd werd.
    pub grams: Vec<GramRef>,
    /// Wat er aan de stand van de zaak veranderde, per betrokken cel.
    pub changes: Vec<StatusChange>,
    /// De waarden die dit besluit van een andere cel accepteerde.
    pub accepted: Vec<AcceptedValue>,
    /// Wat dit besluit uitvoerde; alleen bij [`JournalKind::Besluit`].
    ///
    /// `None` bij elke andere soort regel. Een vastlegging, een betaling of een
    /// vraag voert niets uit, en een lege [`Execution`] zou beweren dat er een
    /// regeling gedraaid heeft die niets deed.
    pub executed: Option<Execution>,
    /// De vraag die over een celgrens ging; alleen bij [`JournalKind::Vraag`].
    ///
    /// Dezelfde vorm als in het beeld van de wereld ([`CrossingSnapshot`]) en
    /// niet een tweede: het is hetzelfde contact, en twee vormen zouden uiteen
    /// gaan lopen.
    ///
    /// Het contact draagt het **antwoord** zoals de bevraagde cel het gaf —
    /// waarde en moment — met de uitleg waarop het berust
    /// ([`Lexostatus::reductie`]). Dat het antwoord hier staat en niet alleen de
    /// vraag, is wat deze regel leesbaar maakt zonder het observatielog ernaast
    /// te leggen: dat log staat buiten de opstelling, en het verhaal hoort in
    /// het verhaal.
    pub question: Option<CrossingSnapshot>,
    /// De regel die deze uitlokte; `None` als deze op zichzelf staat.
    ///
    /// Een cross-cel-vraag hangt onder het besluit dat haar stelde: zij gebeurde
    /// ómdat dat besluit een waarde nodig had, en los gelezen is ze een vraag
    /// zonder aanleiding.
    pub parent: Option<usize>,
}

impl JournalEntry {
    /// Leesbaar verslag van deze regel, met haar grammen en verschillen eronder.
    ///
    /// `indent` is het aantal spaties vóór de regel zelf; wat eronder hangt komt
    /// vier spaties verder in.
    pub fn describe(&self, indent: usize) -> String {
        let pad = " ".repeat(indent);
        let sub = " ".repeat(indent + 4);
        let mut out = format!(
            "{pad}{} · {} · {}\n",
            self.moment,
            self.actor.describe(),
            self.description
        );
        for gram in &self.grams {
            let _ = writeln!(out, "{sub}gram: {}", gram.describe());
        }
        if let Some(executed) = &self.executed {
            let _ = writeln!(out, "{sub}uitgevoerd: {}", executed.describe_regulations());
            for input in &executed.inputs {
                let _ = writeln!(out, "{sub}{}", input.describe());
            }
            for output in &executed.outputs {
                let _ = writeln!(out, "{sub}{}", output.describe());
            }
        }
        for accepted in &self.accepted {
            let _ = writeln!(out, "{sub}{}", accepted.describe());
        }
        if let Some(question) = &self.question {
            let _ = writeln!(out, "{sub}antwoord: {}", describe_answer(&question.answer));
        }
        for change in &self.changes {
            let _ = writeln!(out, "{sub}{}", change.describe());
        }
        out
    }
}

/// Het hele journaal als verslag: elke regel, met haar vragen ingesprongen.
///
/// Eén bron voor alles wat het verhaal vertelt: het beeld van de wereld draagt
/// dezelfde regels, en de scenario-runner schrijft ze hiermee op.
pub fn describe(entries: &[JournalEntry]) -> String {
    let mut out = String::new();
    for entry in entries {
        let indent = if entry.parent.is_some() { 8 } else { 4 };
        out.push_str(&entry.describe(indent));
    }
    out
}

/// Eén lexostatus die meetelt voor "de stand van de zaak" van een cel.
///
/// **Casusdata.** Welke reductie de stand van een zaak draagt, is een eigenschap
/// van de casus en niet van het platform: bij de een is dat een
/// toekenningspositie op een zaakkenmerk, bij de ander een relatievorm op een
/// bsn. Het platform weet alleen dát er indicatoren zijn, en hoe hun parameters
/// uit een gebeurtenis komen.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatusIndicator {
    /// De gepubliceerde lexostatus van deze cel die de stand draagt.
    pub lexostatus: String,
    /// Wat een lezer ziet; standaard de naam van de lexostatus.
    #[serde(default)]
    pub label: Option<String>,
    /// Hoe de parameters van die lexostatus uit een gebeurtenis komen.
    ///
    /// `$veld` wijst een veld van de gebeurtenis aan — het zaakkenmerk van een
    /// besluit, de bsn van een feit — en alles wat niet met `$` begint is een
    /// letterlijke waarde. Wijst een verwijzing naar iets dat deze gebeurtenis
    /// niet draagt, dan gaat deze indicator niet over deze gebeurtenis en wordt
    /// hij niet gereduceerd.
    #[serde(default)]
    pub params: BTreeMap<String, IndicatorParam>,
}

impl StatusIndicator {
    /// Wat een lezer van deze indicator ziet.
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.lexostatus)
    }

    /// De parameters waarmee deze indicator over déze gebeurtenis gaat.
    ///
    /// `None` betekent dat de gebeurtenis er niet over gaat: er is een
    /// verwijzing die zij niet draagt. Dat is geen fout — een betaling zegt
    /// niets over een partnerschap — en levert daarom geen reductie en geen
    /// regel.
    pub fn resolve(&self, bearing: &BTreeMap<String, Value>) -> Option<BTreeMap<String, Value>> {
        self.params
            .iter()
            .map(|(name, param)| Some((name.clone(), param.resolve(bearing)?.clone())))
            .collect()
    }
}

/// Waarmee één parameter van een statusindicator gevuld wordt.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(try_from = "Value")]
pub enum IndicatorParam {
    /// Een veld van de gebeurtenis, bij naam.
    FromEvent(String),
    /// Een waarde die letterlijk in het wereldbestand staat.
    Literal(Value),
}

impl IndicatorParam {
    /// De waarde waarop deze parameter uitkomt; `None` als de gebeurtenis het
    /// aangewezen veld niet draagt.
    fn resolve<'a>(&'a self, bearing: &'a BTreeMap<String, Value>) -> Option<&'a Value> {
        match self {
            Self::FromEvent(field) => bearing.get(field),
            Self::Literal(value) => Some(value),
        }
    }
}

impl TryFrom<Value> for IndicatorParam {
    type Error = String;

    fn try_from(value: Value) -> std::result::Result<Self, Self::Error> {
        let Value::String(text) = &value else {
            return Ok(Self::Literal(value));
        };
        // Een `$`-woord is altijd een verwijzing en nooit tekst: zou het als
        // letterlijke waarde doorgaan, dan stond er straks `$zaakkenmrk` in een
        // vraag en zou de indicator stil nooit iets vinden.
        match text.strip_prefix('$') {
            Some("") => {
                Err("'$' is geen veldnaam; schrijf `$<veld van de gebeurtenis>`".to_string())
            }
            Some(field) => Ok(Self::FromEvent(field.to_string())),
            None => Ok(Self::Literal(value)),
        }
    }
}

/// Eén meting van één statusindicator, vóór of ná een gebeurtenis.
///
/// Intern: dit is het materiaal waaruit [`StatusChange`] volgt, en nooit iets
/// wat naar buiten gaat — een meting zonder haar tegenhanger zegt niets.
#[derive(Debug, Clone)]
pub(crate) struct Reading {
    /// De cel die gereduceerd is.
    pub(crate) cell: String,
    /// De gepubliceerde lexostatus.
    pub(crate) lexostatus: String,
    /// Wat een lezer ziet.
    pub(crate) label: String,
    /// Wat de cel vond; `None` is "niets vastgesteld".
    pub(crate) values: Option<BTreeMap<String, Value>>,
}

impl Reading {
    /// De meting van één antwoord.
    pub(crate) fn of(cell: &str, label: &str, answer: &Lexostatus) -> Self {
        Self {
            cell: cell.to_string(),
            lexostatus: answer.name.clone(),
            label: label.to_string(),
            values: match &answer.outcome {
                LexostatusOutcome::Established(values) => Some(values.clone()),
                LexostatusOutcome::NotEstablished { .. } => None,
            },
        }
    }
}

/// Het verschil tussen twee reeksen metingen, in de volgorde van de metingen.
///
/// Alleen wat veranderde, en alleen wat aan beide kanten gemeten is: een
/// indicator die aan één kant niet te reduceren was, levert geen verschil op
/// maar een halve waarneming, en die hoort niet als verandering gelezen te
/// worden.
pub(crate) fn changes(before: &[Reading], after: &[Reading]) -> Vec<StatusChange> {
    after
        .iter()
        .filter_map(|na| {
            let voor = before
                .iter()
                .find(|voor| voor.cell == na.cell && voor.lexostatus == na.lexostatus)?;
            (voor.values != na.values).then(|| StatusChange {
                cell: na.cell.clone(),
                lexostatus: na.lexostatus.clone(),
                label: na.label.clone(),
                voor: voor.values.clone(),
                na: na.values.clone(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(pairs: &[(&str, i64)]) -> Option<BTreeMap<String, Value>> {
        Some(
            pairs
                .iter()
                .map(|(name, value)| (name.to_string(), Value::Int(*value)))
                .collect(),
        )
    }

    fn reading(cell: &str, lexostatus: &str, values: Option<BTreeMap<String, Value>>) -> Reading {
        Reading {
            cell: cell.to_string(),
            lexostatus: lexostatus.to_string(),
            label: lexostatus.to_string(),
            values,
        }
    }

    #[test]
    fn een_indicator_die_niet_veranderde_levert_geen_regel() {
        let before = vec![reading(
            "belastingdienst",
            "betaald",
            values(&[("bedrag", 0)]),
        )];
        let after = before.clone();
        assert!(changes(&before, &after).is_empty());
    }

    #[test]
    fn van_niets_vastgesteld_naar_een_waarde_is_een_verandering() {
        let before = vec![reading("toeslagen", "beschikking", None)];
        let after = vec![reading(
            "toeslagen",
            "beschikking",
            values(&[("hoogte", 25000)]),
        )];
        let delta = changes(&before, &after);
        assert_eq!(delta.len(), 1);
        assert_eq!(
            delta[0].describe(),
            "toeslagen · beschikking: niets vastgesteld → 25000"
        );
    }

    /// Een reeks zonder uitkomsten leest als "niets vastgesteld".
    ///
    /// Niet als een lege regel: `niets vastgesteld → ` zou een verandering
    /// tonen waarvan de ene kant wegviel, en de weergave zegt hier hetzelfde
    /// (`describeStand`), dus twee lezers van hetzelfde verschil horen niet
    /// verschillende dingen te zien.
    #[test]
    fn een_stand_zonder_uitkomsten_leest_als_niets_vastgesteld() {
        let delta = changes(
            &[reading("brp", "partnerschap", Some(BTreeMap::new()))],
            &[reading("brp", "partnerschap", values(&[("type", 1)]))],
        );
        assert_eq!(delta.len(), 1);
        assert_eq!(
            delta[0].describe(),
            "brp · partnerschap: niets vastgesteld → 1"
        );
    }

    #[test]
    fn een_meting_zonder_tegenhanger_telt_niet_mee() {
        let after = vec![reading("brp", "partnerschap", values(&[("type", 1)]))];
        assert!(changes(&[], &after).is_empty());
    }

    #[test]
    fn een_verwijzing_leest_een_veld_van_de_gebeurtenis() {
        let indicator: StatusIndicator = serde_yaml_ng::from_str(
            "lexostatus: beschikking\nlabel: toekenning\nparams:\n  zaakkenmerk: $zaakkenmerk\n",
        )
        .expect("deze indicator hoort te laden");
        let bearing = BTreeMap::from([(
            "zaakkenmerk".to_string(),
            Value::String("zaak/1".to_string()),
        )]);
        assert_eq!(
            indicator.resolve(&bearing),
            Some(BTreeMap::from([(
                "zaakkenmerk".to_string(),
                Value::String("zaak/1".to_string())
            )]))
        );
        assert_eq!(indicator.label(), "toekenning");
        // Een gebeurtenis zonder dat veld gaat er niet over.
        assert_eq!(indicator.resolve(&BTreeMap::new()), None);
    }

    #[test]
    fn een_lege_verwijzing_wordt_geweigerd() {
        let error = serde_yaml_ng::from_str::<StatusIndicator>(
            "lexostatus: beschikking\nparams:\n  zaakkenmerk: $\n",
        )
        .expect_err("'$' wijst geen veld aan");
        assert!(
            error.to_string().contains("geen veldnaam"),
            "de melding hoort te zeggen wat er mis is: {error}"
        );
    }
}
