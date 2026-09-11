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
    binding_name, check_documented_params, documents, engine_parameters, parameter_listing,
    published_outputs, CellSurface, DocumentedParameter,
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
    /// zegt waar hij vandaan komt: uit een eigen kroniek van de cel, uit de
    /// parameters van het besluit, of **geaccepteerd** van een andere cel
    /// (`accept_from`).
    #[serde(default)]
    pub inputs: BTreeMap<String, BesluitInput>,
}

/// De drie vormen die een input van een besluit kan hebben, voor foutmeldingen.
///
/// Eén tekst, want elke weigering hieronder somt ze op: wie er twee door elkaar
/// haalt, hoort in dezelfde melding te lezen wat de keuze was.
const INPUT_FORMS: &str = "een input komt uit een eigen kroniek (`from_chronicle` + `field`), \
                           uit een parameter (`param`), of van een andere cel \
                           (`accept_from` + `lexostatus` + `field`)";

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
    /// **Geaccepteerd** van een andere cel: die cel stelt de waarde vast, deze
    /// cel rekent haar niet na (RFC-009 beslisboom stap 4, invariant I5).
    ///
    /// De vraag gaat niet vanuit de cel: ze gaat langs de veiligheidscontext
    /// van de besluitende cel naar het transport, en die twee houdt een cel
    /// niet (RFC-022 §2). Wat de cel hier declareert is dus een *verzoek* —
    /// [`AcceptanceRequest`] — en wie het inwilligt staat buiten de cel.
    ///
    /// Wat terugkomt gaat als parameter de engine in en met haar herkomst het
    /// decretogram in ([`InputOrigin::Accepted`]); nergens anders. Een volgend
    /// besluit vraagt opnieuw.
    AcceptFrom {
        /// De cel die de waarde vaststelt.
        cell: String,
        /// De lexostatus die daar opgevraagd wordt.
        lexostatus: String,
        /// De uitkomst van die lexostatus die de waarde draagt.
        field: String,
        /// De parameters van die vraag, op de naam die de bevraagde cel
        /// documenteert. Een waarde `$naam` verwijst naar een gedocumenteerde
        /// parameter van dít besluit; elke andere waarde is letterlijke tekst —
        /// dezelfde vorm als de parameters van een reductie.
        params: BTreeMap<String, String>,
    },
}

impl BesluitInput {
    /// De cel waarvan deze input geaccepteerd wordt, als dat er een is.
    ///
    /// Voor wie de afspraken van buiten wil nalopen — de wereld toetst ermee of
    /// de peer bestaat — zonder de vorm van de variant na te bouwen.
    pub fn accepts_from(&self) -> Option<&str> {
        match self {
            Self::AcceptFrom { cell, .. } => Some(cell),
            Self::FromChronicle { .. } | Self::Param { .. } => None,
        }
    }
}

/// Het YAML-oppervlak van een input: alle velden van alle vormen, los.
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
    accept_from: Option<String>,
    lexostatus: Option<String>,
    params: Option<BTreeMap<String, String>>,
}

impl TryFrom<BesluitInputFields> for BesluitInput {
    type Error = String;

    fn try_from(fields: BesluitInputFields) -> std::result::Result<Self, Self::Error> {
        // Precies één van de drie ankers wijst de vorm aan. Twee ankers is geen
        // vorm met een extraatje maar een input waarvan niemand kan zeggen waar
        // ze vandaan komt, dus de melding noemt ze beide bij hun waarde.
        let anchors: Vec<(&str, &str)> = [
            ("from_chronicle", fields.from_chronicle.as_deref()),
            ("param", fields.param.as_deref()),
            ("accept_from", fields.accept_from.as_deref()),
        ]
        .into_iter()
        .filter_map(|(key, value)| value.map(|value| (key, value)))
        .collect();

        let (kind, value) = match anchors.as_slice() {
            [] => return Err(format!("input noemt geen bron: {INPUT_FORMS}")),
            [single] => *single,
            named => {
                let listed = named
                    .iter()
                    .map(|(key, value)| format!("`{key}` '{value}'"))
                    .collect::<Vec<_>>()
                    .join(" en ");
                return Err(format!("input noemt {listed}; {INPUT_FORMS}"));
            }
        };

        match kind {
            "from_chronicle" => {
                reject_unused(kind, fields.lexostatus.is_some(), "`lexostatus`")?;
                reject_unused(kind, fields.params.is_some(), "`params`")?;
                let field = fields.field.ok_or_else(|| {
                    format!(
                        "input uit kroniekstroom '{value}' mist `field`: zonder veld \
                         weet de cel niet welke waarde van de vastlegging ze bedoelt"
                    )
                })?;
                Ok(Self::FromChronicle {
                    chronicle: value.to_string(),
                    field,
                })
            }
            "accept_from" => {
                let lexostatus = fields.lexostatus.ok_or_else(|| {
                    format!(
                        "input die van cel '{value}' geaccepteerd wordt mist `lexostatus`: \
                         een cel is alleen te bevragen langs een naam die ze publiceert"
                    )
                })?;
                let field = fields.field.ok_or_else(|| {
                    format!(
                        "input die van cel '{value}' geaccepteerd wordt mist `field`: \
                         een lexostatus levert de uitkomsten die ze publiceert, en \
                         zonder veld weet de cel niet welke daarvan ze bedoelt"
                    )
                })?;
                Ok(Self::AcceptFrom {
                    cell: value.to_string(),
                    lexostatus,
                    field,
                    params: fields.params.unwrap_or_default(),
                })
            }
            // Onbereikbaar: de lijst hierboven kent geen vierde anker.
            _ => {
                reject_unused(kind, fields.field.is_some(), "`field`")?;
                reject_unused(kind, fields.lexostatus.is_some(), "`lexostatus`")?;
                reject_unused(kind, fields.params.is_some(), "`params`")?;
                Ok(Self::Param {
                    param: value.to_string(),
                })
            }
        }
    }
}

/// Weiger een veld dat bij een andere vorm hoort dan de gekozen.
///
/// Stil laten liggen zou erger zijn dan streng zijn: een `lexostatus` naast een
/// `param` leest als een vraag over de celgrens en is er geen.
fn reject_unused(kind: &str, present: bool, field: &str) -> std::result::Result<(), String> {
    if present {
        return Err(format!(
            "{field} hoort niet bij een input met `{kind}`; {INPUT_FORMS}"
        ));
    }
    Ok(())
}

/// Eén waarde die een besluit bij een andere cel gaat ophalen.
///
/// De cel stelt dit verzoek samen en zet het **niet** zelf door: ze houdt geen
/// veiligheidscontext en geen transport (RFC-022 §2). Wie het inwilligt — in
/// deze opstelling [`crate::World::decide`] — geeft de waarde met haar herkomst
/// terug aan het besluit.
#[derive(Debug, Clone, PartialEq)]
pub struct AcceptanceRequest {
    /// De input van het besluit die met deze waarde gevuld wordt.
    pub input: String,
    /// De cel aan wie gevraagd wordt.
    pub cell: String,
    /// De lexostatus die daar opgevraagd wordt.
    pub lexostatus: String,
    /// De uitkomst van die lexostatus die de waarde draagt.
    pub field: String,
    /// De parameters van de vraag, met de verwijzingen al ingevuld.
    pub params: BTreeMap<String, Value>,
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
    /// **Geaccepteerd** van een andere cel, en dus niet hier uitgerekend
    /// (invariant I5).
    ///
    /// Dit is het bewijsstuk van die ene vraag, uitgeschreven in gewone velden:
    /// bij wie, onder welke naam, op welk moment, en ondertekend door wie. Niet
    /// als geleend type uit de veiligheidscontext — een cel kent die niet — maar
    /// als wat het gram draagt en een lezer later nakijkt.
    Accepted {
        /// De cel die de waarde vaststelde.
        cell: String,
        /// De lexostatus waaronder ze dat publiceert.
        lexostatus: String,
        /// De uitkomst van die lexostatus die de waarde droeg.
        field: String,
        /// Het moment waarop het antwoord geldt.
        op_moment: NaiveDate,
        /// De identiteit die de vraag stelde en ondertekende.
        asked_by: String,
        /// De (gesimuleerde) ondertekening van die vraag.
        signature: String,
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
            Self::Accepted {
                cell,
                lexostatus,
                field,
                op_moment,
                asked_by,
                signature,
            } => Value::Object(BTreeMap::from([
                (
                    "herkomst".to_string(),
                    Value::String("geaccepteerd".to_string()),
                ),
                ("cell".to_string(), Value::String(cell.clone())),
                ("lexostatus".to_string(), Value::String(lexostatus.clone())),
                ("field".to_string(), Value::String(field.clone())),
                (
                    "op_moment".to_string(),
                    Value::String(op_moment.to_string()),
                ),
                ("asked_by".to_string(), Value::String(asked_by.clone())),
                ("signature".to_string(), Value::String(signature.clone())),
            ])),
        }
    }

    /// Is deze waarde van een andere cel geaccepteerd in plaats van hier
    /// uitgerekend?
    ///
    /// Het onderscheid van invariant I5, op één plek: een scenario, een verslag
    /// en de invarianten-gate stellen alle drie deze vraag, en ze horen hem niet
    /// elk op hun eigen manier te stellen.
    pub fn accepted_from(&self) -> Option<&str> {
        match self {
            Self::Accepted { cell, .. } => Some(cell),
            Self::OwnChronicle { .. } | Self::Parameter { .. } => None,
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
            Self::Accepted {
                cell,
                lexostatus,
                field,
                op_moment,
                asked_by,
                signature,
            } => format!(
                "geaccepteerd van cel '{cell}' ({lexostatus}.{field} op {op_moment}), \
                 gevraagd door {asked_by} [{signature}]"
            ),
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
    /// Elke waarde in dit gram die van een andere cel **geaccepteerd** is, op
    /// naam, met de cel die haar vaststelde.
    ///
    /// Twee wegen komen hier samen, en dat is met opzet één lijst: een input die
    /// de besluit-definitie met `accept_from` vulde, en een waarde die de
    /// *regeling* via een `source.regulation` naar een cel haalde (tier 3, die
    /// de engine in `accepted_values` van het receipt zet). Voor de vraag van
    /// invariant I5 — is dit narekenen of accepteren? — zijn ze hetzelfde, en
    /// wie ze apart houdt, controleert er straks maar één.
    pub fn accepted_values(&self) -> BTreeMap<&str, &str> {
        let from_inputs = self
            .inputs
            .iter()
            .filter_map(|(name, input)| Some((name.as_str(), input.origin.accepted_from()?)));
        let from_receipt = self
            .receipt
            .accepted_values
            .iter()
            .map(|accepted| (accepted.output.as_str(), accepted.authority.as_str()));
        from_inputs.chain(from_receipt).collect()
    }

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
    ///
    /// Weigert een waarde waarin een scheidingsteken van het sjabloon zelf
    /// voorkomt. Zie [`Template::check_separators_absent`]: zonder die weigering
    /// zouden twee verschillende zaken hetzelfde kenmerk kunnen krijgen, en dan
    /// levert een reductie op dat kenmerk het besluit van een ander.
    pub(crate) fn zaakkenmerk(
        &self,
        cell: &str,
        params: &BTreeMap<String, Value>,
    ) -> Result<String> {
        let template = Template::parse(&self.zaakkenmerk);
        template.check_separators_absent(cell, &self.name, &self.zaakkenmerk, params)?;

        let mut out = String::with_capacity(self.zaakkenmerk.len());
        out.push_str(template.leading);
        for part in &template.parts {
            if let Some(value) = params.get(part.reference) {
                out.push_str(&value.to_string());
            }
            out.push_str(part.literal);
        }
        Ok(out)
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
            self.validate_input(cell, surface, input, origin)?;
        }

        self.validate_zaakkenmerk(cell)
    }

    /// Eén input: is de stroom er een om feiten uit te lezen, bestaat ze, kent
    /// ze het veld, en is haar sleutel aan te leveren? Of, bij een parameter: is
    /// die gedocumenteerd?
    fn validate_input(
        &self,
        cell: &str,
        surface: &CellSurface<'_>,
        input: &str,
        origin: &BesluitInput,
    ) -> Result<()> {
        match origin {
            BesluitInput::FromChronicle { chronicle, field } => {
                // Een besluit leest geen besluit. De stroom met decretogrammen is
                // een gewone stroom zodra een cel besluit-definities heeft, dus
                // zonder deze weigering kan een besluit een veld van een eerder
                // decretogram als "eigen feit" binnenhalen — dezelfde
                // schaduwboekhouding die `register_own_facts` aan de kant van de
                // engine al buiten de deur houdt, langs de andere weg.
                if chronicle == BESCHIKKINGEN {
                    return Err(SimulatorError::DecretogramAsBesluitInput {
                        cell: cell.to_string(),
                        besluit: self.name.clone(),
                        input: input.to_string(),
                        stream: chronicle.clone(),
                        field: field.clone(),
                    });
                }
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
            BesluitInput::AcceptFrom {
                cell: peer, params, ..
            } => {
                // De eigen cel is geen peer. Voor eigen feiten is er een
                // kroniek of een eigen wet; wie zichzelf over de grens
                // bevraagt, zet een cross-cel-contact in het vraaggraf dat er
                // niet hoort en zou bij de veiligheidscontext alsnog stuklopen
                // — beter hier, bij het optuigen.
                if peer == cell {
                    return Err(SimulatorError::AcceptFromSelf {
                        cell: cell.to_string(),
                        besluit: self.name.clone(),
                        input: input.to_string(),
                    });
                }
                for reference in params.values().filter_map(|binding| binding_name(binding)) {
                    if !documents(&self.params, reference) {
                        return Err(SimulatorError::UnknownReference {
                            cell: cell.to_string(),
                            subject: Subject::Besluit,
                            name: self.name.clone(),
                            reference: reference.to_string(),
                        });
                    }
                }
                // Of de peer bestaat en deze naam publiceert, weet de cel niet:
                // ze kent geen andere cel. Dat valt bij de wereld (die de peers
                // kent) en anders bij het transport.
                Ok(())
            }
        }
    }

    /// De waarden die dit besluit bij een andere cel moet ophalen.
    ///
    /// Aanroepen ná [`Self::check_params`]: de verwijzingen in `params` zijn bij
    /// het optuigen aan gedocumenteerde parameters gebonden, en die zijn dan
    /// aanwezig.
    ///
    /// Leeg is het normale geval: een besluit dat alles zelf weet, vraagt
    /// niemand iets.
    pub(crate) fn acceptance_requests(
        &self,
        params: &BTreeMap<String, Value>,
    ) -> Vec<AcceptanceRequest> {
        self.inputs
            .iter()
            .filter_map(|(input, origin)| match origin {
                BesluitInput::AcceptFrom {
                    cell,
                    lexostatus,
                    field,
                    params: bindings,
                } => Some(AcceptanceRequest {
                    input: input.clone(),
                    cell: cell.clone(),
                    lexostatus: lexostatus.clone(),
                    field: field.clone(),
                    params: engine_parameters(bindings, params),
                }),
                BesluitInput::FromChronicle { .. } | BesluitInput::Param { .. } => None,
            })
            .collect()
    }

    /// Het zaakkenmerk-sjabloon: sluitende accolades, minstens één verwijzing,
    /// elke verwijzing een gedocumenteerde parameter, en tussen twee
    /// verwijzingen iets dat ze uit elkaar houdt.
    fn validate_zaakkenmerk(&self, cell: &str) -> Result<()> {
        if !closing_braces_match(&self.zaakkenmerk) {
            return Err(SimulatorError::MalformedZaakkenmerk {
                cell: cell.to_string(),
                besluit: self.name.clone(),
                template: self.zaakkenmerk.clone(),
            });
        }

        let template = Template::parse(&self.zaakkenmerk);
        for part in &template.parts {
            if !documents(&self.params, part.reference) {
                return Err(SimulatorError::UnknownReference {
                    cell: cell.to_string(),
                    subject: Subject::Besluit,
                    name: self.name.clone(),
                    reference: part.reference.to_string(),
                });
            }
        }

        if template.parts.is_empty() {
            return Err(SimulatorError::ZaakkenmerkWithoutReference {
                cell: cell.to_string(),
                besluit: self.name.clone(),
                template: self.zaakkenmerk.clone(),
            });
        }

        // Twee verwijzingen die aan elkaar plakken zijn nooit uit elkaar te
        // houden: `{jaar}{bsn}` met 2024 + 999993653 levert hetzelfde kenmerk
        // als 20249 + 99993653. Geen waarde kan dat repareren, dus dit is een
        // optuigfout en geen weigering bij het besluit.
        for pair in template.parts.windows(2) {
            if pair[0].literal.is_empty() {
                return Err(SimulatorError::AdjacentZaakkenmerkReferences {
                    cell: cell.to_string(),
                    besluit: self.name.clone(),
                    template: self.zaakkenmerk.clone(),
                    first: pair[0].reference.to_string(),
                    second: pair[1].reference.to_string(),
                });
            }
        }

        Ok(())
    }
}

/// Eén verwijzing uit een zaakkenmerk-sjabloon, met de letterlijke tekst erachter.
struct TemplatePart<'a> {
    /// De parameternaam tussen de accolades.
    reference: &'a str,
    /// Wat er letterlijk achter deze verwijzing staat, tot de volgende
    /// verwijzing of tot het eind.
    literal: &'a str,
}

/// Een zaakkenmerk-sjabloon, uit elkaar gehaald.
///
/// Eén parser voor het optuigen én het invullen: zouden die uit elkaar lopen,
/// dan zou een sjabloon dat bij het optuigen goedgekeurd is bij het besluit
/// iets anders opleveren dan de toets veronderstelde.
struct Template<'a> {
    /// De letterlijke tekst vóór de eerste verwijzing.
    leading: &'a str,
    /// De verwijzingen, in volgorde.
    parts: Vec<TemplatePart<'a>>,
}

impl<'a> Template<'a> {
    /// Haal een sjabloon uit elkaar.
    ///
    /// Een accolade die niet sluit levert geen verwijzing op; dat geval wordt bij
    /// het optuigen apart geweigerd ([`closing_braces_match`]), zodat het hier
    /// niet stil als letterlijke tekst hoeft te eindigen.
    fn parse(template: &'a str) -> Self {
        let (leading, mut rest) = match template.split_once('{') {
            Some((leading, rest)) => (leading, rest),
            None => {
                return Self {
                    leading: template,
                    parts: Vec::new(),
                }
            }
        };

        let mut parts = Vec::new();
        while let Some((reference, after)) = rest.split_once('}') {
            // Geen volgende `{` betekent dat de rest letterlijke tekst is; `rest`
            // wordt dan leeg en de lus stopt vanzelf op de volgende ronde.
            let (literal, remainder) = after.split_once('{').unwrap_or((after, ""));
            parts.push(TemplatePart { reference, literal });
            rest = remainder;
        }
        Self { leading, parts }
    }

    /// De letterlijke stukken die twee verwijzingen uit elkaar houden.
    ///
    /// De tekst vóór de eerste en die ná de laatste verwijzing tellen niet mee:
    /// die staan vast en kunnen geen twee invullingen laten samenvallen.
    fn separators(&self) -> impl Iterator<Item = &'a str> + '_ {
        let last = self.parts.len().saturating_sub(1);
        self.parts[..last].iter().map(|part| part.literal)
    }

    /// Weiger een parameterwaarde waarin een scheidingsteken van dit sjabloon
    /// voorkomt.
    ///
    /// Het zaakkenmerk is waaronder een zaak terug te vinden is, dus twee zaken
    /// mogen er nooit één worden. Bij `{jaar}/{bsn}` zou `jaar = "2024/9"` met
    /// `bsn = "99993653"` hetzelfde kenmerk geven als `jaar = "2024"` met
    /// `bsn = "999993653"`, en dan levert een reductie op dat kenmerk het besluit
    /// over iemand anders. Zolang geen waarde een scheidingsteken bevat, is de
    /// invulling omkeerbaar en kan dat niet gebeuren.
    ///
    /// Een sjabloon met één verwijzing heeft geen scheidingstekens en weigert dus
    /// niets: daar is elke waarde ondubbelzinnig.
    fn check_separators_absent(
        &self,
        cell: &str,
        besluit: &str,
        template: &str,
        params: &BTreeMap<String, Value>,
    ) -> Result<()> {
        let separators: Vec<&str> = self
            .separators()
            .filter(|separator| !separator.is_empty())
            .collect();
        if separators.is_empty() {
            return Ok(());
        }

        for part in &self.parts {
            let Some(value) = params.get(part.reference) else {
                continue;
            };
            let text = value.to_string();
            for separator in &separators {
                if text.contains(separator) {
                    return Err(SimulatorError::ZaakkenmerkSeparatorInValue {
                        cell: cell.to_string(),
                        besluit: besluit.to_string(),
                        template: template.to_string(),
                        parameter: part.reference.to_string(),
                        separator: (*separator).to_string(),
                    });
                }
            }
        }
        Ok(())
    }
}

/// Sluit elke `{` in dit sjabloon weer?
///
/// Apart van [`Template::parse`], omdat de parser een niet-sluitende accolade
/// overslaat: die twee moeten het eens zijn over wat een verwijzing is, en de
/// weigering hoort bij het optuigen te vallen.
fn closing_braces_match(template: &str) -> bool {
    let mut rest = template;
    while let Some((_, after)) = rest.split_once('{') {
        let Some((_, remainder)) = after.split_once('}') else {
            return false;
        };
        rest = remainder;
    }
    true
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
    fn een_input_zonder_vorm_noemt_de_drie_vormen() {
        let err = input("{}\n").expect_err("een input zonder vorm hoort te falen");
        assert!(
            err.contains("`from_chronicle`")
                && err.contains("`param`")
                && err.contains("`accept_from`"),
            "de melding moet vertellen welke vormen er zijn, kreeg: {err}"
        );
    }

    #[test]
    fn een_input_die_van_een_andere_cel_geaccepteerd_wordt_wordt_gelezen() {
        let parsed = input(
            "accept_from: belastingdienst\nlexostatus: toetsingsinkomen\n\
             field: toetsingsinkomen\nparams:\n  bsn: $bsn\n",
        )
        .unwrap_or_else(|e| panic!("de accepteervorm moet gelezen worden: {e}"));
        assert_eq!(
            parsed,
            BesluitInput::AcceptFrom {
                cell: "belastingdienst".to_string(),
                lexostatus: "toetsingsinkomen".to_string(),
                field: "toetsingsinkomen".to_string(),
                params: BTreeMap::from([("bsn".to_string(), "$bsn".to_string())]),
            }
        );
        assert_eq!(parsed.accepts_from(), Some("belastingdienst"));
    }

    #[test]
    fn accepteren_zonder_lexostatus_wordt_geweigerd() {
        // Een cel is alleen te bevragen langs een naam die ze publiceert; zonder
        // die naam is er geen vraag om te stellen.
        let err = input("accept_from: belastingdienst\nfield: toetsingsinkomen\n")
            .expect_err("zonder `lexostatus` hoort het te falen");
        assert!(
            err.contains("`lexostatus`") && err.contains("belastingdienst"),
            "de melding moet zeggen wat er mist en bij wie, kreeg: {err}"
        );
    }

    #[test]
    fn accepteren_zonder_veld_wordt_geweigerd() {
        let err = input("accept_from: belastingdienst\nlexostatus: toetsingsinkomen\n")
            .expect_err("zonder `field` hoort het te falen");
        assert!(
            err.contains("`field`"),
            "de melding moet `field` noemen, kreeg: {err}"
        );
    }

    /// Een `lexostatus` naast een `param` leest als een vraag over de celgrens en
    /// is er geen. Stil laten liggen zou een input opleveren die iets anders doet
    /// dan er staat.
    #[test]
    fn een_veld_van_een_andere_vorm_wordt_geweigerd() {
        let err = input("param: bsn\nlexostatus: toetsingsinkomen\n")
            .expect_err("een veld van een andere vorm hoort te falen");
        assert!(
            err.contains("`lexostatus`") && err.contains("param"),
            "de melding moet zeggen welk veld niet bij welke vorm hoort, kreeg: {err}"
        );
    }

    #[test]
    fn accepteren_van_de_eigen_cel_wordt_geweigerd() {
        let mut definition = definition("zorgtoeslag/{bsn}");
        definition.inputs.insert(
            "toetsingsinkomen".to_string(),
            BesluitInput::AcceptFrom {
                cell: "toeslagen".to_string(),
                lexostatus: "toetsingsinkomen".to_string(),
                field: "toetsingsinkomen".to_string(),
                params: BTreeMap::new(),
            },
        );

        let err = definition
            .validate_input(
                "toeslagen",
                &surface_met_uitkomst(&[], "heeft_recht_op_zorgtoeslag"),
                "toetsingsinkomen",
                &definition.inputs["toetsingsinkomen"],
            )
            .expect_err("de eigen cel is geen peer");
        assert!(
            matches!(err, SimulatorError::AcceptFromSelf { .. }),
            "verwachtte AcceptFromSelf, kreeg {err}"
        );
    }

    #[test]
    fn een_verwijzing_in_de_parameters_van_een_accepteervraag_moet_gedocumenteerd_zijn() {
        let definition = definition("zorgtoeslag/{bsn}");
        let origin = BesluitInput::AcceptFrom {
            cell: "belastingdienst".to_string(),
            lexostatus: "toetsingsinkomen".to_string(),
            field: "toetsingsinkomen".to_string(),
            params: BTreeMap::from([("bsn".to_string(), "$burgerservicenummer".to_string())]),
        };

        let err = definition
            .validate_input(
                "toeslagen",
                &surface_met_uitkomst(&[], "heeft_recht_op_zorgtoeslag"),
                "toetsingsinkomen",
                &origin,
            )
            .expect_err("een verwijzing zonder gedocumenteerde parameter hoort te falen");
        assert!(
            matches!(err, SimulatorError::UnknownReference { .. }),
            "verwachtte UnknownReference, kreeg {err}"
        );
    }

    #[test]
    fn de_verzoeken_van_een_besluit_vullen_hun_verwijzingen_in() {
        let mut definition = definition("zorgtoeslag/{bsn}");
        definition.inputs.insert(
            "toetsingsinkomen".to_string(),
            BesluitInput::AcceptFrom {
                cell: "belastingdienst".to_string(),
                lexostatus: "toetsingsinkomen".to_string(),
                field: "toetsingsinkomen".to_string(),
                params: BTreeMap::from([("bsn".to_string(), "$bsn".to_string())]),
            },
        );
        definition.inputs.insert(
            "is_verzekerde".to_string(),
            BesluitInput::Param {
                param: "bsn".to_string(),
            },
        );

        let params = BTreeMap::from([
            ("bsn".to_string(), Value::String("999993653".to_string())),
            ("jaar".to_string(), Value::String("2024".to_string())),
        ]);
        let requests = definition.acceptance_requests(&params);

        assert_eq!(
            requests,
            vec![AcceptanceRequest {
                input: "toetsingsinkomen".to_string(),
                cell: "belastingdienst".to_string(),
                lexostatus: "toetsingsinkomen".to_string(),
                field: "toetsingsinkomen".to_string(),
                params: BTreeMap::from([(
                    "bsn".to_string(),
                    Value::String("999993653".to_string())
                )]),
            }],
            "alleen de accepteervorm levert een verzoek, met de waarde erin"
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
  - name: jaar
    type: string
"
        ))
        .unwrap_or_else(|e| panic!("testdefinitie moet parsen: {e}"))
    }

    /// Vul een sjabloon in, of leg luid uit waarom dat niet mocht.
    fn zaakkenmerk(template: &str, params: &[(&str, &str)]) -> Result<String> {
        let params: BTreeMap<String, Value> = params
            .iter()
            .map(|(name, value)| ((*name).to_string(), Value::String((*value).to_string())))
            .collect();
        definition(template).zaakkenmerk("toeslagen", &params)
    }

    #[test]
    fn het_zaakkenmerk_vult_de_parameters_in() {
        assert_eq!(
            zaakkenmerk("zorgtoeslag/{bsn}", &[("bsn", "999993653")])
                .unwrap_or_else(|e| panic!("het sjabloon moet in te vullen zijn: {e}")),
            "zorgtoeslag/999993653"
        );
    }

    /// Eén verwijzing kent geen scheidingsteken, dus elke waarde is eenduidig.
    ///
    /// Wat er vóór en achter staat ligt vast, dus `zorgtoeslag/` gevolgd door een
    /// waarde met een `/` erin blijft terug te lezen. Weigeren zou hier een regel
    /// opleggen die niets beschermt.
    #[test]
    fn een_enkele_verwijzing_neemt_elke_waarde_zoals_ze_is() {
        assert_eq!(
            zaakkenmerk("zorgtoeslag/{bsn}", &[("bsn", "9999/93653")])
                .unwrap_or_else(|e| panic!("één verwijzing hoort niets te weigeren: {e}")),
            "zorgtoeslag/9999/93653"
        );
    }

    /// Twee zaken mogen nooit één kenmerk krijgen.
    ///
    /// `{jaar}/{bsn}` met `2024/9` + `99993653` levert letterlijk hetzelfde
    /// kenmerk als `2024` + `999993653`. Dan wijst het kenmerk naar twee zaken en
    /// levert een reductie erop het besluit over iemand anders.
    #[test]
    fn een_waarde_met_het_scheidingsteken_erin_wordt_geweigerd() {
        let eerlijk = zaakkenmerk("{jaar}/{bsn}", &[("jaar", "2024"), ("bsn", "999993653")])
            .unwrap_or_else(|e| panic!("een gewone invulling moet slagen: {e}"));

        let err = zaakkenmerk("{jaar}/{bsn}", &[("jaar", "2024/9"), ("bsn", "99993653")])
            .expect_err("een waarde die het scheidingsteken bevat hoort te falen");
        assert!(
            matches!(err, SimulatorError::ZaakkenmerkSeparatorInValue { .. }),
            "verwachtte ZaakkenmerkSeparatorInValue, kreeg {err}"
        );
        assert_eq!(
            eerlijk, "2024/999993653",
            "de botsing die geweigerd wordt, is precies dit kenmerk"
        );
    }

    #[test]
    fn twee_verwijzingen_zonder_scheiding_worden_bij_het_optuigen_geweigerd() {
        let err = definition("{jaar}{bsn}")
            .validate_zaakkenmerk("toeslagen")
            .expect_err("twee verwijzingen tegen elkaar aan hoort te falen");
        assert!(
            matches!(err, SimulatorError::AdjacentZaakkenmerkReferences { .. }),
            "verwachtte AdjacentZaakkenmerkReferences, kreeg {err}"
        );
    }

    #[test]
    fn twee_verwijzingen_met_scheiding_mogen_wel() {
        definition("{jaar}/{bsn}")
            .validate_zaakkenmerk("toeslagen")
            .unwrap_or_else(|e| panic!("een gescheiden sjabloon hoort te mogen: {e}"));
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
