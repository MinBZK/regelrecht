//! Het uitvoeringsreceipt van één decretogram, op verzoek leesbaar gemaakt.
//!
//! RFC-022 §1.2 zegt dat een decretogram *is* het RFC-013 Execution Receipt van
//! het besluit: de herkomst van de uitvoering, de regelingen die geladen waren
//! met hun hash, de parameters, de uitkomsten en de waarden die van een ander
//! geaccepteerd zijn. Het ligt ook echt in het gram — [`crate::Decretogram`]
//! schrijft het er ongewijzigd in — maar het gaat met opzet **niet** mee in het
//! beeld van de wereld: het draagt wandkloktijd, en een contract dat per run
//! verschilt is geen contract (zie de moduledocs van [`crate::snapshot`]).
//!
//! Daarmee was het van buitenaf onzichtbaar dat een decretogram het receipt
//! draagt. Deze module is de weg ernaartoe die dat oplost zonder het beeld te
//! vervuilen: een lezer vraagt het receipt van één gram, en krijgt precies dat.
//! Het beeld blijft receipt-loos.
//!
//! Drie dingen die hier gebeuren en die het ruwe veld uit het gram niet doet:
//!
//! - **Het zegt van welk gram dit het receipt is** ([`ReceiptGram`]). Wie een
//!   receipt los in handen krijgt, hoort te kunnen zien in wiens kroniek het
//!   ligt en over welke zaak het gaat.
//! - **Het maakt `accepted_values` compleet.** Een besluit kan op twee manieren
//!   een waarde accepteren — `accept_from` in de besluit-definitie en een
//!   `source.regulation` die een cel aanwijst — en de engine ziet alleen de
//!   tweede. Hier komen ze in één lijst, met de bron-cel, het bevoegd gezag van
//!   die bron, het moment en het zaakkenmerk erbij. Dat is dezelfde vereniging
//!   die [`crate::Decretogram::accepted_values`] maakt, en om dezelfde reden:
//!   wie ze apart houdt, controleert er straks maar één.
//! - **Het labelt de tijdstempel.** [`ReceiptTimestamp`] zegt erbij dat dit de
//!   wandkloktijd van de uitvoering is en geen moment in de logische tijd van
//!   deze wereld — precies de reden dat het beeld hem niet draagt.
//!
//! Wat hier **niet** gebeurt, is het receipt overschrijven. Elke andere sectie
//! gaat door zoals het gram haar draagt ([`GramReceipt::sections`]), dus een
//! RFC-013 die morgen een sectie toevoegt, staat hier morgen in beeld zonder
//! dat er iets aangepast hoeft te worden.

use crate::cell::{
    ChronicleEvent, Intake, BESLUIT, COMPETENT_AUTHORITY, INPUTS, RECEIPT, ZAAKKENMERK,
};
use crate::error::{Result, SimulatorError};
use chrono::NaiveDate;
use regelrecht_engine::Value;
use serde::Serialize;
use std::collections::BTreeMap;

/// De sectie van een RFC-013-receipt die de geaccepteerde waarden draagt.
const ACCEPTED_VALUES: &str = "accepted_values";

/// De sectie van een RFC-013-receipt die de wandkloktijd draagt.
const TIMESTAMP: &str = "timestamp";

/// De naam waaronder de engine een geaccepteerde waarde in het receipt zet.
const OUTPUT: &str = "output";

/// Het veld waarin de engine zet van wie ze de waarde accepteerde. Dat is bij
/// de cel-tier het cel-id: de engine kent adressen, geen gezagen.
const AUTHORITY: &str = "authority";

/// De waarde van een geaccepteerde uitkomst, zoals de engine haar opschrijft.
const VALUE: &str = "value";

/// Het label dat bij de tijdstempel hoort.
///
/// Eén plek, want dit is het hele punt van de sectie: dat een lezer niet denkt
/// dat dit een moment in de logische tijd van de wereld is.
const TIMESTAMP_NOTE: &str = "wandkloktijd van de uitvoering, niet de logische tijd van de \
                              wereld; daarom draagt het beeld van de wereld dit receipt niet";

/// Het uitvoeringsreceipt van één decretogram, met het gram erbij.
///
/// De secties van RFC-013 staan op het hoogste niveau — `provenance`,
/// `engine_config`, `scope`, `execution`, `results` — zodat dit antwoord het
/// receipt *is* en geen omhulsel eromheen. Twee daarvan zijn vervangen door een
/// rijkere uitgave: zie [`Self::accepted_values`] en [`Self::timestamp`].
#[derive(Debug, Clone, Serialize)]
pub struct GramReceipt {
    /// Van welk gram dit het receipt is.
    pub gram: ReceiptGram,
    /// Elke sectie van het receipt zoals het gram haar draagt.
    ///
    /// Doorgegeven en niet overgeschreven: een handmatige kopie zou bij elke
    /// uitbreiding van RFC-013 stil achterlopen, en dan zou dit niet meer het
    /// receipt zijn maar een selectie eruit. Dezelfde afweging als bij het
    /// vastleggen ervan in [`crate::Decretogram`].
    ///
    /// `accepted_values` en `timestamp` zitten er niet in; die staan hieronder,
    /// met meer dan het receipt zelf wist.
    #[serde(flatten)]
    pub sections: BTreeMap<String, Value>,
    /// Elke waarde die dit besluit van een andere cel accepteerde.
    pub accepted_values: Vec<ReceiptAcceptedValue>,
    /// De wandkloktijd van de uitvoering, met erbij wat dat betekent.
    pub timestamp: ReceiptTimestamp,
}

/// Het gram waarvan dit het receipt is.
///
/// Waar het ligt (cel, stroom, plek) en waarover het gaat (naam, besluit,
/// zaakkenmerk, moment). De plek is de volgorde van vastlegging, dezelfde
/// waarmee het beeld de grammen geeft en waarmee een journaalregel ernaar wijst
/// (zie [`crate::journal::GramRef`]).
#[derive(Debug, Clone, Serialize)]
pub struct ReceiptGram {
    /// De cel in wiens kroniek het gram ligt.
    pub cell: String,
    /// De kroniekstroom.
    pub chronicle: String,
    /// De plek in die stroom, geteld vanaf nul.
    pub index: usize,
    /// Hoe het gram heet, in de woorden van de cel.
    pub name: String,
    /// De besluit-definitie die uitgevoerd is.
    pub besluit: Option<String>,
    /// Waaronder deze zaak terug te vinden is.
    pub zaakkenmerk: Option<String>,
    /// Het moment in de logische tijd waarop besloten is. Niet te verwarren met
    /// [`ReceiptTimestamp`], en dat ze allebei in beeld staan is precies waarom.
    pub op_moment: NaiveDate,
}

/// Eén waarde die dit besluit van een andere cel accepteerde in plaats van
/// nagerekend (invariant I5, RFC-013 `accepted_values`).
///
/// Rijker dan wat de engine alleen weet, want de engine kent van de cel-tier
/// niet meer dan een adres en een antwoord. Wat er hier bij komt, komt uit het
/// gram zelf: onder welke lexostatus gevraagd is, op welk moment het antwoord
/// geldt, wie de vraag ondertekende, en welk bevoegd gezag de bron erbij noemde.
#[derive(Debug, Clone, Serialize)]
pub struct ReceiptAcceptedValue {
    /// De naam waaronder het besluit de waarde gebruikte.
    pub output: String,
    /// De waarde zoals ze meedeed. `None` als het gram haar niet draagt: wat de
    /// *wet* via een cel-bron haalde, staat niet in de inputs van het gram.
    pub value: Option<Value>,
    /// De bron-cel die de waarde vaststelde.
    pub cell: String,
    /// Het bevoegd gezag dat die bron-cel bij haar antwoord noemde.
    ///
    /// `None` betekent dat haar lexostatus er niets over publiceert. Dat is een
    /// gat bij de bron en geen reden om hier iets in te vullen: wie het
    /// terugleest hoort te zien dat er geen gezag bij stond.
    pub authority: Option<String>,
    /// De lexostatus waaronder gevraagd is; `None` bij een waarde die de wet via
    /// een cel-bron haalde, want die noemt geen naam.
    pub lexostatus: Option<String>,
    /// De uitkomst van die lexostatus die de waarde droeg.
    pub field: Option<String>,
    /// Het moment waarop het antwoord geldt.
    pub op_moment: Option<NaiveDate>,
    /// De zaak waarvoor geaccepteerd is: het zaakkenmerk van dit besluit.
    ///
    /// Van het besluit en niet van de bron: de bron-cel stelde een feit vast en
    /// weet van deze zaak niets. Het staat er zodat een geaccepteerde waarde ook
    /// los van haar receipt aan een zaak te koppelen is.
    pub zaakkenmerk: Option<String>,
    /// De identiteit die de vraag stelde en ondertekende.
    pub asked_by: Option<String>,
    /// De (gesimuleerde) ondertekening van die vraag.
    pub signature: Option<String>,
}

/// De tijdstempel van het receipt, met erbij wat voor tijd het is.
///
/// Twee velden en niet één string, want de string alleen is precies wat er mis
/// kan gaan: wie hem naast [`ReceiptGram::op_moment`] ziet staan, houdt hem voor
/// een moment in de logische tijd van deze wereld. Dat is hij niet, en daarom
/// staat het erbij.
#[derive(Debug, Clone, Serialize)]
pub struct ReceiptTimestamp {
    /// De wandkloktijd zoals het receipt haar draagt.
    pub wall_clock: Option<String>,
    /// Wat voor tijd dat is, in gewone woorden.
    pub note: &'static str,
}

/// Bouw het receipt van één gram uit een kroniek.
///
/// `pub(crate)`: de weg hiernaartoe is [`crate::World::gram_receipt`]. Dit is
/// geen tweede ingang naar een kroniek — de aanroeper moet het gram al in
/// handen hebben om het hier te kunnen aanbieden.
pub(crate) fn build(
    cell: &str,
    chronicle: &str,
    index: usize,
    event: &ChronicleEvent,
) -> Result<GramReceipt> {
    let receipt = decretogram_receipt(event).ok_or_else(|| SimulatorError::GramWithoutReceipt {
        cell: cell.to_string(),
        stream: chronicle.to_string(),
        index,
        name: event.name.clone(),
    })?;

    let zaakkenmerk = text(event.fields.get(ZAAKKENMERK));
    let mut sections = receipt.clone();
    let accepted = sections.remove(ACCEPTED_VALUES);
    let timestamp = sections.remove(TIMESTAMP);

    Ok(GramReceipt {
        gram: ReceiptGram {
            cell: cell.to_string(),
            chronicle: chronicle.to_string(),
            index,
            name: event.name.clone(),
            besluit: text(event.fields.get(BESLUIT)),
            zaakkenmerk: zaakkenmerk.clone(),
            op_moment: event.op_moment,
        },
        sections,
        accepted_values: accepted_values(&event.fields, accepted.as_ref(), zaakkenmerk.as_deref()),
        timestamp: ReceiptTimestamp {
            wall_clock: text(timestamp.as_ref()),
            note: TIMESTAMP_NOTE,
        },
    })
}

/// Het receipt dat dit gram draagt, of `None` als het er geen draagt.
///
/// Twee eisen, en de tweede is de echte: het gram ontstond langs het besluit-pad
/// (`intake: eigen_besluit`) én er staat een receipt in. Een bron-cel legt haar
/// eigen vaststelling ook als `eigen_besluit` vast — even goed een decretogram —
/// maar zonder engine is er geen uitvoering geweest, en dus geen receipt. Zie
/// [`crate::snapshot`], dat dezelfde twee vragen stelt om de herkomst van een
/// veld te kunnen noemen.
fn decretogram_receipt(event: &ChronicleEvent) -> Option<&BTreeMap<String, Value>> {
    if event.intake != Intake::EigenBesluit {
        return None;
    }
    event.fields.get(RECEIPT)?.as_object()
}

/// Elke waarde die dit besluit accepteerde, uit de twee wegen samen.
///
/// De inputs van het gram eerst: die dragen de volle herkomst. Wat de *wet* via
/// een cel-bron haalde, staat daar niet in en komt uit `accepted_values` van het
/// receipt zelf — met wat de engine ervan wist en niet meer dan dat. Een naam
/// die in beide zit, komt één keer in beeld, en dan in de rijke vorm.
fn accepted_values(
    fields: &BTreeMap<String, Value>,
    from_receipt: Option<&Value>,
    zaakkenmerk: Option<&str>,
) -> Vec<ReceiptAcceptedValue> {
    let mut accepted = from_inputs(fields, zaakkenmerk);
    for entry in from_receipt.and_then(Value::as_array).unwrap_or_default() {
        let Some(parts) = entry.as_object() else {
            continue;
        };
        let Some(output) = text(parts.get(OUTPUT)) else {
            continue;
        };
        if accepted.iter().any(|known| known.output == output) {
            continue;
        }
        accepted.push(ReceiptAcceptedValue {
            output,
            value: parts.get(VALUE).cloned(),
            // De engine schrijft hier het cel-id: zij kent de peer als adres en
            // niet als gezag. Wat de bron als bevoegd gezag noemde, is langs deze
            // weg nergens vastgelegd, en dan staat er `None` in plaats van het
            // adres nog een keer onder een andere naam.
            cell: text(parts.get(AUTHORITY)).unwrap_or_default(),
            authority: None,
            lexostatus: None,
            field: None,
            op_moment: None,
            zaakkenmerk: zaakkenmerk.map(str::to_string),
            asked_by: None,
            signature: None,
        });
    }
    accepted.sort_by(|a, b| a.output.cmp(&b.output));
    accepted
}

/// De geaccepteerde inputs van het gram, met hun volle herkomst.
///
/// Uit het gram gelezen en niet uit een tweede bron: wat hier uitkomt is precies
/// wat er in de kroniek staat. Een gram zonder inputs — of met een herkomst die
/// niet te lezen is — levert een lege lijst op; een receipt hoort niet om te
/// vallen omdat één herkomst niet te lezen was. Zelfde afweging als in
/// [`crate::snapshot`].
fn from_inputs(
    fields: &BTreeMap<String, Value>,
    zaakkenmerk: Option<&str>,
) -> Vec<ReceiptAcceptedValue> {
    let Some(inputs) = fields.get(INPUTS).and_then(Value::as_object) else {
        return Vec::new();
    };
    inputs
        .iter()
        .filter_map(|(name, entry)| {
            let parts = entry.as_object()?;
            let origin = parts.get("origin")?.as_object()?;
            if text(origin.get("herkomst")).as_deref() != Some(ACCEPTED) {
                return None;
            }
            Some(ReceiptAcceptedValue {
                output: name.clone(),
                value: parts.get(VALUE).cloned(),
                cell: text(origin.get("cell")).unwrap_or_default(),
                authority: text(origin.get(COMPETENT_AUTHORITY)),
                lexostatus: text(origin.get("lexostatus")),
                field: text(origin.get("field")),
                op_moment: text(origin.get("op_moment"))
                    .and_then(|moment| moment.parse::<NaiveDate>().ok()),
                zaakkenmerk: zaakkenmerk.map(str::to_string),
                asked_by: text(origin.get("asked_by")),
                signature: text(origin.get("signature")),
            })
        })
        .collect()
}

/// Hoe een geaccepteerde herkomst zich in een gram noemt.
///
/// Dezelfde tekst die [`crate::InputOrigin`] schrijft; die vorm is het contract
/// van het gram en staat daar.
const ACCEPTED: &str = "geaccepteerd";

/// Een veld als tekst, of `None` als het er niet is of geen tekst draagt.
fn text(value: Option<&Value>) -> Option<String> {
    value?.as_str().map(str::to_string)
}
