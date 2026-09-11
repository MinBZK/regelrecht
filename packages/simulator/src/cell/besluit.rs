//! Het besluit-pad: een cel voert een wet uit en legt de uitkomst vast.
//!
//! De lus van chronolexografie is informeren → concluderen → **vastleggen**, en
//! dit is het derde deel. Een besluit-definitie is data, net als een
//! lexostatus-definitie: ze noemt een eigen regeling, de uitkomst die het
//! besluit *is*, waar de inputs vandaan komen en hoe het zaakkenmerk gevormd
//! wordt. Wat eruit komt is een [`Decretogram`]: het RFC-013 Execution Receipt
//! plus het moment en het zaakkenmerk, vastgelegd als gewoon executogram in de
//! eigen kroniek van de cel.
//!
//! Twee dingen die dit pad met opzet *niet* doet:
//!
//! - **Het bewaart niets naast het decretogram.** Een waarde die het besluit
//!   ophaalde, gaat mee ín het decretogram (met haar herkomst) en niet als los
//!   feit in een kroniek. Anders zou een volgend besluit of een volgende
//!   reductie op andermans feiten leunen zonder opnieuw te kijken — de
//!   schaduwboekhouding die RFC-022 uitsluit.
//! - **Het rekent niet in een reductie.** [`crate::Cell::reduce`] leest het
//!   decretogram terug als kroniekfeit; ligt er geen, dan is het antwoord
//!   "niets vastgesteld". Nooit een herberekening onder een inmiddels andere
//!   wetsversie.

use crate::cell::config::{
    check_documented_params, documents, parameter_listing, published_outputs, CellSurface,
    DocumentedParameter,
};
use crate::cell::{ChronicleEvent, Intake};
use crate::error::{Result, SimulatorError, Subject};
use chrono::NaiveDate;
use regelrecht_engine::{ExecutionReceipt, Value};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

/// De kroniekstroom waarin een cel haar eigen decretogrammen legt.
///
/// Eén vaste naam per cel, automatisch aanwezig zodra de cel besluit-definities
/// heeft, en voorbehouden: een configuratie die zelf een stroom met deze naam
/// declareert wordt geweigerd. Dat de naam vastligt, is wat een reductie erover
/// mogelijk maakt zonder dat elke casus hem opnieuw verzint.
pub const BESCHIKKINGEN: &str = "beschikkingen";

/// De veldnamen die elk decretogram draagt, naast de uitkomsten van het besluit.
///
/// Ze staan hier bij elkaar omdat twee plekken ze allebei moeten kennen: het
/// vastleggen (welke velden krijgt het gram) en het optuigen (waarover mag een
/// lexostatus over deze stroom iets beloven). Zouden die uit elkaar lopen, dan
/// zou een reductie over een bestaand veld als typfout geweigerd worden.
pub const ZAAKKENMERK: &str = "zaakkenmerk";
/// Veld met de naam van de besluit-definitie die het gram voortbracht.
pub const BESLUIT: &str = "besluit";
/// Veld met de `$id` van de uitgevoerde regeling.
pub const REGULATION: &str = "regulation";
/// Veld met de `valid_from` van de regelingversie die gold op `op_moment`.
pub const REGULATION_VALID_FROM: &str = "regulation_valid_from";
/// Veld met het bevoegd gezag dat de regeling noemt (RFC-002).
pub const COMPETENT_AUTHORITY: &str = "competent_authority";
/// Veld met het rechtskarakter dat de regeling aan deze uitkomst geeft.
pub const LEGAL_CHARACTER: &str = "legal_character";
/// Veld met de inputs van het besluit, elk met hun herkomst.
pub const INPUTS: &str = "inputs";
/// Veld met het volledige RFC-013 Execution Receipt.
pub const RECEIPT: &str = "receipt";

/// De vaste velden van een decretogram, in de volgorde waarin ze hierboven staan.
const FIXED_FIELDS: [&str; 8] = [
    ZAAKKENMERK,
    BESLUIT,
    REGULATION,
    REGULATION_VALID_FROM,
    COMPETENT_AUTHORITY,
    LEGAL_CHARACTER,
    INPUTS,
    RECEIPT,
];

/// Eén besluit dat een cel kan nemen.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BesluitDefinition {
    /// De naam waarmee dit besluit aangeroepen wordt.
    pub name: String,
    /// Vrije toelichting; verschijnt niet in het decretogram, wel in de config.
    #[serde(default)]
    pub doc: Option<String>,
    /// De regeling die uitgevoerd wordt, bij `$id`. Moet in `laws` van dezelfde
    /// cel staan: een cel besluit op haar eigen recht.
    pub regulation: String,
    /// De uitkomst die dit besluit *is* en die de uitvoering aanstuurt.
    pub output: String,
    /// Uitkomsten die naast [`Self::output`] in het decretogram meegaan.
    ///
    /// Wat samen ontstaat, wordt samen vastgelegd (RFC-022 §1.2): één besluit is
    /// één elementair gram met al zijn uitkomsten erin, niet één gram per
    /// uitkomst.
    #[serde(default)]
    pub outputs: Vec<String>,
    /// Het zaakkenmerk, als sjabloon met `{parameter}`-verwijzingen.
    ///
    /// Dit is waaronder de zaak terug te vinden is: de kroniek van
    /// decretogrammen over één zaak deelt één zaakkenmerk (RFC-022 §1.2).
    pub zaakkenmerk: String,
    /// De gedocumenteerde parameters van dit besluit.
    #[serde(default)]
    pub params: Vec<DocumentedParameter>,
    /// Wat het besluit aan de engine aanlevert, op inputnaam.
    ///
    /// De naam is die van een parameter of input van de regeling; de waarde
    /// zegt waar hij vandaan komt. In deze versie zijn dat de eigen kronieken
    /// van de cel en de parameters van het besluit; een waarde van een andere
    /// cel accepteren volgt apart.
    #[serde(default)]
    pub inputs: BTreeMap<String, BesluitInput>,
}

/// Waar één input van een besluit vandaan komt.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "BesluitInputFields")]
pub enum BesluitInput {
    /// Uit een eigen kroniek van de besluitende cel (tier 1 van RFC-022 §4.2).
    ///
    /// De laatste vastlegging op of vóór het moment van het besluit, gezocht op
    /// het sleutelveld van die stroom — en de waarde van dat sleutelveld komt
    /// uit de gedocumenteerde parameter met dezelfde naam.
    FromChronicle {
        /// De eigen kroniekstroom waaruit gelezen wordt.
        chronicle: String,
        /// Het veld van die vastlegging dat de waarde draagt.
        field: String,
    },
    /// Meegegeven bij het besluit zelf.
    Param {
        /// De gedocumenteerde parameter die de waarde levert.
        param: String,
    },
}

/// Het YAML-oppervlak van een input: alle velden van beide vormen, los.
///
/// Zelfde keuze als bij [`crate::Reduction`]: de vorm valt hieronder en niet in
/// serde, zodat er in de foutmelding staat wat er mis is in plaats van "data did
/// not match any variant".
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BesluitInputFields {
    from_chronicle: Option<String>,
    field: Option<String>,
    param: Option<String>,
}

impl TryFrom<BesluitInputFields> for BesluitInput {
    type Error = String;

    fn try_from(fields: BesluitInputFields) -> std::result::Result<Self, Self::Error> {
        match (fields.from_chronicle, fields.param) {
            (Some(chronicle), Some(param)) => Err(format!(
                "input noemt zowel kroniekstroom '{chronicle}' als parameter '{param}'; \
                 een input komt óf uit een eigen kroniek (`from_chronicle` + `field`) \
                 óf uit een parameter (`param`)"
            )),
            (Some(chronicle), None) => {
                let field = fields.field.ok_or_else(|| {
                    format!(
                        "input uit kroniekstroom '{chronicle}' mist `field`: zonder veld \
                         weet de cel niet welke waarde van de vastlegging ze bedoelt"
                    )
                })?;
                Ok(Self::FromChronicle { chronicle, field })
            }
            (None, Some(param)) => {
                if fields.field.is_some() {
                    return Err(format!(
                        "`field` hoort bij een input uit een kroniekstroom \
                         (`from_chronicle`), niet bij parameter '{param}'"
                    ));
                }
                Ok(Self::Param { param })
            }
            (None, None) => Err(
                "input noemt geen `from_chronicle` en geen `param`: een input komt uit \
                 een eigen kroniek (`from_chronicle` + `field`) of uit een parameter \
                 (`param`)"
                    .to_string(),
            ),
        }
    }
}

/// Waar één waarde in een decretogram vandaan kwam.
///
/// Dit is wat het decretogram zelf draagt: niet alleen de waarde waarop besloten
/// is, maar ook wie haar aanleverde en van wanneer ze was. Zonder die herkomst
/// is een besluit niet terug te lezen, en is "accepteren in plaats van
/// narekenen" niet van "gokken" te onderscheiden.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputOrigin {
    /// Uit een eigen kroniek van de besluitende cel.
    OwnChronicle {
        /// De stroom waaruit gelezen is.
        chronicle: String,
        /// Het veld dat de waarde droeg.
        field: String,
        /// Het moment van de vastlegging waaruit de waarde komt — niet het
        /// moment van het besluit. Dat verschil is het hele punt van een
        /// kroniek.
        recorded_op_moment: NaiveDate,
    },
    /// Meegegeven bij het besluit.
    Parameter {
        /// De parameter die de waarde leverde.
        parameter: String,
    },
}

impl InputOrigin {
    /// De herkomst als vastlegbare waarde, voor in het decretogram.
    fn as_value(&self) -> Value {
        match self {
            Self::OwnChronicle {
                chronicle,
                field,
                recorded_op_moment,
            } => Value::Object(BTreeMap::from([
                (
                    "herkomst".to_string(),
                    Value::String("eigen_kroniek".to_string()),
                ),
                ("chronicle".to_string(), Value::String(chronicle.clone())),
                ("field".to_string(), Value::String(field.clone())),
                (
                    "op_moment".to_string(),
                    Value::String(recorded_op_moment.to_string()),
                ),
            ])),
            Self::Parameter { parameter } => Value::Object(BTreeMap::from([
                (
                    "herkomst".to_string(),
                    Value::String("parameter".to_string()),
                ),
                ("parameter".to_string(), Value::String(parameter.clone())),
            ])),
        }
    }

    /// Leesbare herkomst voor een verslag.
    pub fn describe(&self) -> String {
        match self {
            Self::OwnChronicle {
                chronicle,
                field,
                recorded_op_moment,
            } => format!("eigen kroniek '{chronicle}.{field}', vastgelegd {recorded_op_moment}"),
            Self::Parameter { parameter } => format!("parameter '{parameter}'"),
        }
    }
}

/// Eén input van een besluit: de waarde waarop besloten is, met haar herkomst.
#[derive(Debug, Clone, PartialEq)]
pub struct DecretogramInput {
    /// De waarde zoals ze meedeed in het besluit.
    pub value: Value,
    /// Waar ze vandaan kwam.
    pub origin: InputOrigin,
}

/// Het vastgelegde besluit: het RFC-013 Execution Receipt plus moment en
/// zaakkenmerk.
///
/// Geen parallel formaat naast het receipt (RFC-022 §1.2 — een decretogram *is*
/// een engine-uitkomst met een rechtskarakter, en het receipt is haar lichaam).
/// Wat erbij komt, is wat het receipt niet kan weten: op welk moment in de
/// logische tijd dit besluit genomen is, onder welk zaakkenmerk het terug te
/// vinden is, en waar elke input vandaan kwam.
#[derive(Debug, Clone)]
pub struct Decretogram {
    /// De cel die besloot.
    pub cell: String,
    /// De besluit-definitie die uitgevoerd is.
    pub besluit: String,
    /// Waaronder deze zaak terug te vinden is.
    pub zaakkenmerk: String,
    /// Het moment waarop besloten is.
    pub op_moment: NaiveDate,
    /// De uitgevoerde regeling, bij `$id`.
    pub regulation: String,
    /// De `valid_from` van de regelingversie die op `op_moment` gold.
    ///
    /// Dit is wat een besluit van een lexogram onderscheidt: het gram houdt vast
    /// welk recht gold toen, ook als er later een andere versie in werking treedt.
    pub regulation_valid_from: Option<String>,
    /// Het bevoegd gezag dat de regeling noemt (RFC-002). Een juridisch feit van
    /// het besluit, geen eigenschap van de cel (RFC-022 §2).
    pub competent_authority: Option<String>,
    /// Het rechtskarakter dat de regeling aan deze uitkomst geeft
    /// (`produces.legal_character`), bijvoorbeeld `BESCHIKKING`.
    pub legal_character: Option<String>,
    /// De uitkomsten van het besluit: de aansturende uitkomst plus wat de
    /// definitie erbij noemt. Alle samen in één gram.
    pub outputs: BTreeMap<String, Value>,
    /// De inputs waarop besloten is, met hun herkomst.
    pub inputs: BTreeMap<String, DecretogramInput>,
    /// Het volledige receipt van de uitvoering.
    pub receipt: ExecutionReceipt,
}

impl Decretogram {
    /// Het decretogram als kroniekgebeurtenis.
    ///
    /// Een gewoon executogram met `intake: eigen_besluit`, zodat de tijdreductie
    /// er zonder speciale gevallen overheen werkt: wie op een moment vóór dit
    /// besluit vraagt, ziet het niet, en wie erna vraagt krijgt het terug zoals
    /// het toen vastgelegd is.
    pub(crate) fn event(&self) -> Result<ChronicleEvent> {
        let mut fields: BTreeMap<String, Value> = BTreeMap::from([
            (
                ZAAKKENMERK.to_string(),
                Value::String(self.zaakkenmerk.clone()),
            ),
            (BESLUIT.to_string(), Value::String(self.besluit.clone())),
            (
                REGULATION.to_string(),
                Value::String(self.regulation.clone()),
            ),
            (
                REGULATION_VALID_FROM.to_string(),
                optional_text(self.regulation_valid_from.as_deref()),
            ),
            (
                COMPETENT_AUTHORITY.to_string(),
                optional_text(self.competent_authority.as_deref()),
            ),
            (
                LEGAL_CHARACTER.to_string(),
                optional_text(self.legal_character.as_deref()),
            ),
            (
                INPUTS.to_string(),
                Value::Object(
                    self.inputs
                        .iter()
                        .map(|(name, input)| {
                            (
                                name.clone(),
                                Value::Object(BTreeMap::from([
                                    ("value".to_string(), input.value.clone()),
                                    ("origin".to_string(), input.origin.as_value()),
                                ])),
                            )
                        })
                        .collect(),
                ),
            ),
            (RECEIPT.to_string(), self.receipt_value()?),
        ]);
        fields.extend(
            self.outputs
                .iter()
                .map(|(name, value)| (name.clone(), value.clone())),
        );

        Ok(ChronicleEvent {
            name: self.besluit.clone(),
            intake: Intake::EigenBesluit,
            recording_actor: self.cell.clone(),
            grondslag: self.grondslag(),
            op_moment: self.op_moment,
            fields,
        })
    }

    /// Het receipt als vastlegbare waarde.
    ///
    /// Via serde en niet met de hand overgeschreven: een handmatige kopie zou
    /// bij elke uitbreiding van RFC-013 stil achterlopen, en dan zou het
    /// decretogram niet meer het receipt zijn maar een selectie eruit.
    fn receipt_value(&self) -> Result<Value> {
        let encoded = serde_yaml_ng::to_string(&self.receipt).map_err(|source| {
            SimulatorError::ReceiptEncoding {
                cell: self.cell.clone(),
                besluit: self.besluit.clone(),
                source,
            }
        })?;
        serde_yaml_ng::from_str(&encoded).map_err(|source| SimulatorError::ReceiptEncoding {
            cell: self.cell.clone(),
            besluit: self.besluit.clone(),
            source,
        })
    }

    /// De grondslag van deze vastlegging: de regeling die uitgevoerd is, met de
    /// versie die toen gold.
    fn grondslag(&self) -> String {
        match &self.regulation_valid_from {
            Some(valid_from) => format!("{} (versie {valid_from})", self.regulation),
            None => self.regulation.clone(),
        }
    }
}

/// Een tekst die er kan zijn, als vastlegbare waarde.
///
/// `null` en niet "de lege tekst": dat de regeling geen bevoegd gezag noemt is
/// iets anders dan een bevoegd gezag zonder naam.
fn optional_text(text: Option<&str>) -> Value {
    text.map_or(Value::Null, |value| Value::String(value.to_string()))
}

impl BesluitDefinition {
    /// De uitkomsten die dit besluit vastlegt.
    pub fn recorded_outputs(&self) -> BTreeSet<&str> {
        published_outputs(Some(self.output.as_str()), &self.outputs)
    }

    /// Controleer de meegegeven parameters tegen de gedocumenteerde.
    pub(crate) fn check_params(&self, cell: &str, params: &BTreeMap<String, Value>) -> Result<()> {
        check_documented_params(cell, Subject::Besluit, &self.name, &self.params, params)
    }

    /// Het zaakkenmerk voor deze parameters.
    ///
    /// Aanroepen ná [`Self::check_params`]: elke verwijzing is bij het optuigen
    /// aan een gedocumenteerde parameter gebonden, en die is dan aanwezig.
    pub(crate) fn zaakkenmerk(&self, params: &BTreeMap<String, Value>) -> String {
        let mut out = String::with_capacity(self.zaakkenmerk.len());
        let mut rest = self.zaakkenmerk.as_str();
        while let Some((before, after)) = rest.split_once('{') {
            out.push_str(before);
            let Some((reference, remainder)) = after.split_once('}') else {
                out.push('{');
                out.push_str(after);
                return out;
            };
            if let Some(value) = params.get(reference) {
                out.push_str(&value.to_string());
            }
            rest = remainder;
        }
        out.push_str(rest);
        out
    }

    /// Controleer de definitie tegen de cel waarin ze staat.
    ///
    /// Een besluit belooft dat het uit te voeren is: op een eigen regeling, met
    /// uitkomsten die die regeling kent, met inputs die ze declareert, uit
    /// stromen die de cel houdt. Dat blijkt hier — bij het optuigen — en niet
    /// pas op het moment dat er besloten moet worden.
    pub(crate) fn validate(&self, cell: &str, surface: &CellSurface<'_>) -> Result<()> {
        surface.check_own_regulation(cell, Subject::Besluit, &self.name, &self.regulation)?;
        surface.check_regulation_outputs(
            cell,
            Subject::Besluit,
            &self.name,
            &self.regulation,
            self.recorded_outputs(),
        )?;

        // De uitkomsten komen in hetzelfde gram als de vaste velden. Een
        // uitkomst die zo heet, zou er een overschrijven — het gram zou dan
        // bijvoorbeeld zijn receipt kwijt zijn zonder dat iemand het merkt.
        for output in self.recorded_outputs() {
            if FIXED_FIELDS.contains(&output) {
                return Err(SimulatorError::ReservedDecretogramField {
                    cell: cell.to_string(),
                    besluit: self.name.clone(),
                    output: output.to_string(),
                    fixed: FIXED_FIELDS.join(", "),
                });
            }
        }

        let known_inputs = surface
            .regulation_inputs
            .get(&self.regulation)
            .cloned()
            .unwrap_or_default();
        for (input, origin) in &self.inputs {
            if !known_inputs
                .iter()
                .any(|known| known.eq_ignore_ascii_case(input))
            {
                return Err(SimulatorError::UnknownRegulationInput {
                    cell: cell.to_string(),
                    besluit: self.name.clone(),
                    input: input.clone(),
                    regulation: self.regulation.clone(),
                    known: known_inputs
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>()
                        .join(", "),
                });
            }
            self.validate_input(cell, surface, origin)?;
        }

        self.validate_zaakkenmerk(cell)
    }

    /// Eén input: bestaat de stroom, kent ze het veld, en is haar sleutel
    /// aan te leveren? Of, bij een parameter: is die gedocumenteerd?
    fn validate_input(
        &self,
        cell: &str,
        surface: &CellSurface<'_>,
        origin: &BesluitInput,
    ) -> Result<()> {
        match origin {
            BesluitInput::FromChronicle { chronicle, field } => {
                surface.check_stream_field(cell, Subject::Besluit, &self.name, chronicle, field)?;
                // Onbereikbaar leeg: `check_stream_field` heeft de stroom
                // hierboven al gevonden, en elke stroom declareert een sleutel.
                let key = surface
                    .stream_key(chronicle)
                    .unwrap_or_default()
                    .to_string();
                if !documents(&self.params, &key) {
                    return Err(SimulatorError::BesluitStreamKeyWithoutParameter {
                        cell: cell.to_string(),
                        besluit: self.name.clone(),
                        stream: chronicle.clone(),
                        key,
                        documented: parameter_listing(&self.params),
                    });
                }
                Ok(())
            }
            BesluitInput::Param { param } => {
                if documents(&self.params, param) {
                    return Ok(());
                }
                Err(SimulatorError::UnknownReference {
                    cell: cell.to_string(),
                    subject: Subject::Besluit,
                    name: self.name.clone(),
                    reference: param.clone(),
                })
            }
        }
    }

    /// Het zaakkenmerk-sjabloon: sluitende accolades, minstens één verwijzing,
    /// en elke verwijzing een gedocumenteerde parameter.
    fn validate_zaakkenmerk(&self, cell: &str) -> Result<()> {
        let mut rest = self.zaakkenmerk.as_str();
        let mut references = 0usize;
        while let Some((_, after)) = rest.split_once('{') {
            let Some((reference, remainder)) = after.split_once('}') else {
                return Err(SimulatorError::MalformedZaakkenmerk {
                    cell: cell.to_string(),
                    besluit: self.name.clone(),
                    template: self.zaakkenmerk.clone(),
                });
            };
            if !documents(&self.params, reference) {
                return Err(SimulatorError::UnknownReference {
                    cell: cell.to_string(),
                    subject: Subject::Besluit,
                    name: self.name.clone(),
                    reference: reference.to_string(),
                });
            }
            references += 1;
            rest = remainder;
        }

        if references == 0 {
            return Err(SimulatorError::ZaakkenmerkWithoutReference {
                cell: cell.to_string(),
                besluit: self.name.clone(),
                template: self.zaakkenmerk.clone(),
            });
        }
        Ok(())
    }
}

/// De veldnamen die de stroom met decretogrammen van deze cel gaat dragen.
///
/// Bekend vóór het eerste besluit, en dat moet ook: een lexostatus die over deze
/// stroom reduceert wordt bij het optuigen getoetst, en dan is de stroom nog
/// leeg. Zonder deze lijst zou elke reductie over een decretogram als typfout
/// geweigerd worden.
pub(crate) fn declared_fields(definitions: &[BesluitDefinition]) -> BTreeSet<String> {
    let mut fields: BTreeSet<String> = FIXED_FIELDS.iter().map(|f| (*f).to_string()).collect();
    for definition in definitions {
        fields.extend(
            definition
                .recorded_outputs()
                .into_iter()
                .map(str::to_string),
        );
    }
    fields
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(yaml: &str) -> std::result::Result<BesluitInput, String> {
        serde_yaml_ng::from_str(yaml).map_err(|e| e.to_string())
    }

    #[test]
    fn een_input_uit_een_kroniek_wordt_gelezen() {
        let parsed = input("from_chronicle: inkomensleveringen\nfield: verzamelinkomen\n")
            .unwrap_or_else(|e| panic!("de kroniekvorm moet gelezen worden: {e}"));
        assert_eq!(
            parsed,
            BesluitInput::FromChronicle {
                chronicle: "inkomensleveringen".to_string(),
                field: "verzamelinkomen".to_string(),
            }
        );
    }

    #[test]
    fn een_input_uit_een_parameter_wordt_gelezen() {
        let parsed =
            input("param: bsn\n").unwrap_or_else(|e| panic!("de parametervorm moet lezen: {e}"));
        assert_eq!(
            parsed,
            BesluitInput::Param {
                param: "bsn".to_string()
            }
        );
    }

    #[test]
    fn een_input_met_twee_vormen_noemt_ze_beide() {
        let err = input("from_chronicle: relaties\nfield: x\nparam: bsn\n")
            .expect_err("twee vormen in één input hoort te falen");
        assert!(
            err.contains("relaties") && err.contains("bsn"),
            "de melding moet beide vormen noemen, kreeg: {err}"
        );
    }

    #[test]
    fn een_input_uit_een_kroniek_zonder_veld_wordt_geweigerd() {
        let err =
            input("from_chronicle: relaties\n").expect_err("zonder `field` hoort het te falen");
        assert!(
            err.contains("`field`"),
            "de melding moet `field` noemen, kreeg: {err}"
        );
    }

    #[test]
    fn een_input_zonder_vorm_noemt_de_twee_vormen() {
        let err = input("{}\n").expect_err("een input zonder vorm hoort te falen");
        assert!(
            err.contains("`from_chronicle`") && err.contains("`param`"),
            "de melding moet vertellen welke twee vormen er zijn, kreeg: {err}"
        );
    }

    fn definition(zaakkenmerk: &str) -> BesluitDefinition {
        serde_yaml_ng::from_str(&format!(
            r"
name: toekenning
regulation: wet_op_de_zorgtoeslag
output: heeft_recht_op_zorgtoeslag
zaakkenmerk: '{zaakkenmerk}'
params:
  - name: bsn
    type: string
"
        ))
        .unwrap_or_else(|e| panic!("testdefinitie moet parsen: {e}"))
    }

    #[test]
    fn het_zaakkenmerk_vult_de_parameters_in() {
        let params = BTreeMap::from([("bsn".to_string(), Value::String("999993653".to_string()))]);
        assert_eq!(
            definition("zorgtoeslag/{bsn}").zaakkenmerk(&params),
            "zorgtoeslag/999993653"
        );
    }

    #[test]
    fn een_zaakkenmerk_zonder_verwijzing_wordt_geweigerd() {
        let err = definition("zorgtoeslag")
            .validate_zaakkenmerk("toeslagen")
            .expect_err("een zaakkenmerk dat elke zaak gelijk maakt hoort te falen");
        assert!(
            matches!(err, SimulatorError::ZaakkenmerkWithoutReference { .. }),
            "verwachtte ZaakkenmerkWithoutReference, kreeg {err}"
        );
    }

    #[test]
    fn een_zaakkenmerk_met_een_open_accolade_wordt_geweigerd() {
        let err = definition("zorgtoeslag/{bsn")
            .validate_zaakkenmerk("toeslagen")
            .expect_err("een accolade die niet sluit hoort te falen");
        assert!(
            matches!(err, SimulatorError::MalformedZaakkenmerk { .. }),
            "verwachtte MalformedZaakkenmerk, kreeg {err}"
        );
    }

    /// Een oppervlak dat alles kent wat de definitie hieronder noemt.
    ///
    /// Rechtstreeks in elkaar gezet en niet uit een corpus gelezen: de botsing
    /// die deze test afdekt vraagt een regeling met een uitkomst die `receipt`
    /// heet, en die bestaat in het corpus niet. Dat ze er niet is, is geen reden
    /// om de weigering ongetest te laten — ze is er morgen misschien wel.
    fn surface_met_uitkomst<'a>(laws: &'a [String], output: &str) -> CellSurface<'a> {
        CellSurface {
            laws,
            outputs: BTreeMap::from([(
                "wet_op_de_zorgtoeslag".to_string(),
                BTreeSet::from([output.to_string()]),
            )]),
            regulation_inputs: BTreeMap::new(),
            streams: BTreeMap::new(),
            stream_keys: BTreeMap::new(),
        }
    }

    #[test]
    fn een_uitkomst_die_een_vast_veld_zou_overschrijven_wordt_geweigerd() {
        let laws = vec!["wet_op_de_zorgtoeslag".to_string()];
        let mut definition = definition("zorgtoeslag/{bsn}");
        definition.output = RECEIPT.to_string();

        let err = definition
            .validate("toeslagen", &surface_met_uitkomst(&laws, RECEIPT))
            .expect_err("een uitkomst die een vast veld overschrijft hoort te falen");
        assert!(
            matches!(err, SimulatorError::ReservedDecretogramField { .. }),
            "verwachtte ReservedDecretogramField, kreeg {err}"
        );
    }

    #[test]
    fn een_zaakkenmerk_dat_naar_een_onbekende_parameter_verwijst_wordt_geweigerd() {
        let err = definition("zorgtoeslag/{burgerservicenummer}")
            .validate_zaakkenmerk("toeslagen")
            .expect_err("een verwijzing zonder gedocumenteerde parameter hoort te falen");
        assert!(
            matches!(err, SimulatorError::UnknownReference { .. }),
            "verwachtte UnknownReference, kreeg {err}"
        );
    }
}
