//! De celconfiguratie: welke wetten een cel laadt, welke feiten ze houdt, welke
//! lexostatussen ze publiceert en welke besluiten ze kan nemen.
//!
//! Lexostatus- en besluit-definities zijn **data**, geen Rust. Ze staan in de
//! configuratie van de cel, worden gelezen door de loader en zijn verder
//! onveranderlijk. Een consument kan dus geen eigen reductie injecteren; hij kan
//! alleen een gepubliceerde naam opvragen met gedocumenteerde parameters
//! (RFC-022 §4.1).
//!
//! Wat de twee soorten definitie delen, staat hier ook: de gedocumenteerde
//! parameter, de controle van een vraag daartegen, en de toetsen die een
//! definitie aan de cel houden waarin ze staat ([`CellSurface`]). Ze beloven
//! allebei hetzelfde soort ding, dus ze horen op dezelfde manier afgekeurd te
//! worden.

use crate::cell::besluit::BesluitDefinition;
use crate::cell::chronicle::ChronicleStream;
use crate::error::{Result, SimulatorError, Subject};
use regelrecht_engine::Value;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

/// Alles wat nodig is om één cel op te tuigen.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellConfig {
    /// Het cel-id, bijvoorbeeld `toeslagen`.
    pub id: String,
    /// De regelingen die deze cel zelf laadt, bij `$id`.
    pub laws: Vec<String>,
    /// De kroniekstromen met de eigen feiten van de cel.
    #[serde(default)]
    pub chronicles: Vec<ChronicleStream>,
    /// De lexostatussen die de cel naar buiten publiceert.
    #[serde(default)]
    pub lexostatus_definitions: Vec<LexostatusDefinition>,
    /// De besluiten die de cel kan nemen.
    ///
    /// Niet gepubliceerd: een besluit wordt niet door een consument opgevraagd
    /// maar door de cel zelf uitgevoerd (zie [`crate::World::decide`]). Wat er
    /// naar buiten van te zien is, is het decretogram dat eruit komt — en dat
    /// via een reductie over de eigen kroniek.
    #[serde(default)]
    pub besluit_definitions: Vec<BesluitDefinition>,
    /// De cellen die de **wetten** van deze cel aanwijzen, en hoe daar te
    /// vragen (tier 3 van RFC-022 §4.2).
    ///
    /// Dit is de andere manier waarop een cel aan een waarde van een ander komt.
    /// Bij `accept_from` zegt de besluit-definitie het; hier zegt de wet het, met
    /// een `source.regulation` die geen regeling is maar een cel-id. De engine
    /// bereikt zo'n bron alleen langs een geregistreerde resolver, en alleen voor
    /// de cel-ids die hier staan: een cel die niet gedeclareerd is, kan niet per
    /// ongeluk bevraagd worden.
    ///
    /// Waarom dit niet uit de wet te lezen is: de wet noemt een cel-id en een
    /// uitkomstnaam, maar wat die naam bij de bevraagde cel is — welke
    /// gepubliceerde lexostatus, en welke uitkomst daarvan — is een afspraak
    /// tussen twee organisaties en geen eigenschap van het recht.
    #[serde(default)]
    pub accepts_from: Vec<AcceptedSource>,
}

/// Eén afspraak over een cel-bron van de wetten van deze cel (tier 3).
///
/// Leest als: *noemt een van mijn wetten `source.regulation: <cell>` voor
/// uitkomst `<output>`, dan vraag ik daarvoor lexostatus `<lexostatus>` bij die
/// cel en neem ik uitkomst `<field>` van het antwoord.*
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptedSource {
    /// Het cel-id zoals de wet het in `source.regulation` noemt.
    pub cell: String,
    /// De uitkomstnaam die de wet vraagt: `source.output`, of — als de wet die
    /// niet noemt — de naam van de input zelf. Dat is precies wat de engine aan
    /// de resolver doorgeeft.
    pub output: String,
    /// De gepubliceerde lexostatus waarmee die uitkomst bij de peer op te vragen
    /// is.
    pub lexostatus: String,
    /// De uitkomst van die lexostatus die de waarde draagt.
    pub field: String,
    /// Vrije toelichting; verschijnt nergens in een antwoord.
    #[serde(default)]
    pub doc: Option<String>,
}

/// Eén gepubliceerde lexostatus met haar gedocumenteerde parameters en reductie.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LexostatusDefinition {
    /// De naam waarmee een consument deze lexostatus opvraagt.
    pub name: String,
    /// Vrije toelichting; verschijnt niet in het antwoord, wel in de config.
    #[serde(default)]
    pub doc: Option<String>,
    /// De gedocumenteerde parameters. Een vraag die hiervan afwijkt, faalt.
    #[serde(default)]
    pub inputs: Vec<DocumentedParameter>,
    /// De gedocumenteerde uitkomsten: wat de cel onder deze naam publiceert.
    ///
    /// Wat hier niet staat, komt niet in het antwoord, ook al berekende de
    /// engine het onderweg. Bij de wetsvorm betekent leeg of afwezig: alleen
    /// de `output` van de reductie. Die uitkomst hoort er altijd bij — ze *is*
    /// de lexostatus — dus deze lijst breidt uit, ze perkt niet in.
    ///
    /// Bij een kroniekfilter is de lijst **verplicht**: daar is geen
    /// wetsuitkomst die het antwoord bepaalt, dus zonder deze lijst zou de cel
    /// haar hele vastlegging naar buiten geven en niets beloofd hebben.
    #[serde(default)]
    pub outputs: Vec<String>,
    /// Hoe de cel over haar eigen feiten reduceert.
    pub reduction: Reduction,
}

/// Eén gedocumenteerde parameter van een lexostatus of van een besluit.
///
/// Dezelfde vorm voor beide, want het is dezelfde belofte: dit zijn de namen en
/// typen die de aanroeper mag — en moet — meegeven, en alles daarbuiten wordt
/// geweigerd.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentedParameter {
    /// Parameternaam, waarnaar een reductie met `$naam` en een zaakkenmerk met
    /// `{naam}` verwijst.
    pub name: String,
    /// Het verwachte type van de meegegeven waarde.
    #[serde(rename = "type")]
    pub value_type: ParameterType,
}

/// De typen die een lexostatus-parameter kan hebben.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParameterType {
    /// Tekst, bijvoorbeeld een BSN.
    String,
    /// Geheel of decimaal getal.
    Number,
    /// Waar of niet waar.
    Boolean,
}

impl ParameterType {
    /// De naam zoals die in foutmeldingen verschijnt.
    fn label(self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Number => "number",
            Self::Boolean => "boolean",
        }
    }

    /// Past deze waarde bij het gedocumenteerde type?
    fn accepts(self, value: &Value) -> bool {
        match self {
            Self::String => matches!(value, Value::String(_)),
            Self::Number => matches!(value, Value::Int(_) | Value::Decimal(_)),
            Self::Boolean => matches!(value, Value::Bool(_)),
        }
    }
}

/// De chronolexoreductie: hoe de cel over haar eigen feiten reduceert.
///
/// Twee vormen, en de configuratie kiest door te noemen wat ze bedoelt:
///
/// - de **wetsvorm** (`regulation` + `output`) laat een eigen regeling over de
///   eigen feiten rekenen;
/// - het **kroniekfilter** (`chronicle` + `key`) leest rechtstreeks uit een
///   eigen kroniek, zonder engine.
///
/// Dat de tweede vorm bestaat is geen gemak maar de toets op het contract: een
/// organisatie die niet op RegelRecht draait is evengoed een cel, en haar
/// lexostatussen zijn filters over haar eigen vastleggingen (RFC-022 §2 — de
/// engine is een component dat in een cel kán draaien, niet de cel zelf).
///
/// Aggregeren (som, telling) over kronieken hoort in deze plek thuis en bestaat
/// nog niet; `latest: true` is het enige filter dat er nu is.
#[derive(Debug, Clone, Deserialize)]
#[serde(try_from = "ReductionFields")]
pub enum Reduction {
    /// Een uitkomst van een eigen regeling, berekend over de eigen feiten.
    Law {
        /// De regeling, bij `$id`. Moet in `laws` van dezelfde cel staan.
        regulation: String,
        /// De uitkomst van die regeling die de reductie moet opleveren.
        ///
        /// Dit stuurt de evaluatie aan. Wat het antwoord draagt, bepaalt
        /// [`LexostatusDefinition::outputs`]: de engine levert ook de
        /// uitkomsten die causaal met deze meekomen, en die zijn daarmee nog
        /// niet gepubliceerd.
        output: String,
        /// De parameters voor de regeling. Een waarde `$naam` verwijst naar een
        /// gedocumenteerde parameter van de lexostatus; elke andere waarde is
        /// een letterlijke tekst.
        parameters: BTreeMap<String, String>,
    },
    /// Een filter over één eigen kroniek: per sleutelwaarde de laatste
    /// vastlegging op of vóór het gevraagde moment.
    Chronicle {
        /// De kroniekstroom waarover gefilterd wordt. Moet een stroom van
        /// dezelfde cel zijn.
        chronicle: String,
        /// Het veld waarop de vastlegging gezocht wordt. Tevens de naam van de
        /// gedocumenteerde parameter die de waarde aanlevert.
        key: String,
        /// Extra gelijkheidsvoorwaarden op velden van de vastlegging (`where`).
        ///
        /// Ze bepalen wélke vastleggingen het filter in beschouwing neemt;
        /// daarna wint de laatste. Een voorwaarde op een veld dat over tijd
        /// verandert levert dus de laatste vastlegging die eraan voldeed, niet
        /// de huidige stand.
        conditions: BTreeMap<String, Value>,
    },
}

/// Het YAML-oppervlak van een reductie: alle velden van beide vormen, los.
///
/// De keuze tussen de vormen valt in [`Reduction::try_from`] en niet in serde.
/// `#[serde(untagged)]` zou hier "data did not match any variant" opleveren bij
/// elke typfout, en een verplicht `kind`-veld zou de configuratie laten zeggen
/// wat ze al toont. Zo staat er in de foutmelding wat er mis is.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReductionFields {
    /// Zie [`Reduction::Law::regulation`].
    regulation: Option<String>,
    /// Zie [`Reduction::Law::output`].
    output: Option<String>,
    /// Zie [`Reduction::Law::parameters`].
    parameters: Option<BTreeMap<String, String>>,
    /// Zie [`Reduction::Chronicle::chronicle`].
    chronicle: Option<String>,
    /// Zie [`Reduction::Chronicle::key`].
    key: Option<String>,
    /// Alleen `true` heeft betekenis; zie [`Reduction`].
    latest: Option<bool>,
    /// Zie [`Reduction::Chronicle::conditions`].
    #[serde(rename = "where")]
    conditions: Option<BTreeMap<String, Value>>,
}

impl TryFrom<ReductionFields> for Reduction {
    type Error = String;

    fn try_from(fields: ReductionFields) -> std::result::Result<Self, Self::Error> {
        match (fields.regulation, fields.chronicle) {
            (Some(regulation), Some(chronicle)) => Err(format!(
                "reductie noemt zowel regeling '{regulation}' als kroniekstroom '{chronicle}'; \
                 een reductie is óf een wetsvorm (`regulation` + `output`) \
                 óf een kroniekfilter (`chronicle` + `key`)"
            )),
            (Some(regulation), None) => {
                if fields.key.is_some() || fields.latest.is_some() || fields.conditions.is_some() {
                    return Err(format!(
                        "`key`, `latest` en `where` horen bij een kroniekfilter (`chronicle`), \
                         niet bij de reductie over regeling '{regulation}'"
                    ));
                }
                let output = fields.output.ok_or_else(|| {
                    format!(
                        "reductie over regeling '{regulation}' mist `output`: \
                         zonder uitkomst valt er niets te berekenen"
                    )
                })?;
                Ok(Self::Law {
                    regulation,
                    output,
                    parameters: fields.parameters.unwrap_or_default(),
                })
            }
            (None, Some(chronicle)) => {
                if fields.output.is_some() || fields.parameters.is_some() {
                    return Err(format!(
                        "`output` en `parameters` horen bij een reductie over een regeling, \
                         niet bij het kroniekfilter op '{chronicle}'; \
                         wat een kroniekfilter oplevert staat in `outputs`"
                    ));
                }
                if fields.latest == Some(false) {
                    return Err(format!(
                        "kroniekfilter op '{chronicle}' kent alleen `latest: true`: \
                         aggregeren over kronieken (som, telling) bestaat nog niet"
                    ));
                }
                let key = fields.key.ok_or_else(|| {
                    format!(
                        "kroniekfilter op '{chronicle}' mist `key`: zonder sleutelveld \
                         weet het filter niet over welk onderwerp de vraag gaat"
                    )
                })?;
                Ok(Self::Chronicle {
                    chronicle,
                    key,
                    conditions: fields.conditions.unwrap_or_default(),
                })
            }
            (None, None) => Err(
                "reductie noemt geen `regulation` en geen `chronicle`: een reductie is \
                 een wetsvorm (`regulation` + `output`) of een kroniekfilter \
                 (`chronicle` + `key`)"
                    .to_string(),
            ),
        }
    }
}

/// Wat een cel te bieden heeft, zoals ze bij het optuigen blijkt te zijn.
///
/// Hiermee toetst elke lexostatus-definitie of ze belooft wat de cel kan
/// waarmaken: haar eigen regelingen met hun uitkomsten, en haar eigen
/// kroniekstromen met de velden die daarin voorkomen.
pub(crate) struct CellSurface<'a> {
    /// De regelingen die de cel zelf laadt, bij `$id`.
    pub(crate) laws: &'a [String],
    /// Per regeling de uitkomstnamen, over alle geladen versies heen.
    pub(crate) outputs: BTreeMap<String, BTreeSet<String>>,
    /// Per regeling de namen die ze als parameter of input declareert, over alle
    /// geladen versies heen. Dit is wat een besluit aan de engine mag aanleveren.
    pub(crate) regulation_inputs: BTreeMap<String, BTreeSet<String>>,
    /// Per kroniekstroom de veldnamen die de cel van die stroom kent.
    pub(crate) streams: BTreeMap<String, BTreeSet<String>>,
    /// Per kroniekstroom het sleutelveld waarop ze groepeert.
    pub(crate) stream_keys: BTreeMap<String, String>,
}

impl CellSurface<'_> {
    /// De uitkomsten die een regeling kent; leeg als de cel haar niet laadt.
    fn outputs_of(&self, regulation: &str) -> BTreeSet<String> {
        self.outputs.get(regulation).cloned().unwrap_or_default()
    }

    /// Laadt de cel deze regeling zelf?
    ///
    /// Eén plek voor de weigering, want de reden is voor een reductie en voor
    /// een besluit dezelfde: een cel rekent op haar eigen recht.
    pub(crate) fn check_own_regulation(
        &self,
        cell: &str,
        subject: Subject,
        name: &str,
        regulation: &str,
    ) -> Result<()> {
        if self.laws.iter().any(|law| law == regulation) {
            return Ok(());
        }
        Err(SimulatorError::ForeignRegulation {
            cell: cell.to_string(),
            subject,
            name: name.to_string(),
            regulation: regulation.to_string(),
        })
    }

    /// Kent deze regeling elk van deze uitkomsten?
    ///
    /// `self.outputs` bevat de uitkomstnamen van álle geladen versies. Een
    /// definitie afkeuren om een naam die alleen in de nieuwste versie ontbreekt,
    /// zou een vraag of een besluit over een ouder moment onterecht blokkeren.
    pub(crate) fn check_regulation_outputs<'a>(
        &self,
        cell: &str,
        subject: Subject,
        name: &str,
        regulation: &str,
        outputs: impl IntoIterator<Item = &'a str>,
    ) -> Result<()> {
        let known = self.outputs_of(regulation);
        for output in outputs {
            if !known.contains(output) {
                return Err(SimulatorError::UnknownOutput {
                    cell: cell.to_string(),
                    subject,
                    name: name.to_string(),
                    origin: format!("regeling '{regulation}'"),
                    output: output.to_string(),
                    known: listing(&known),
                });
            }
        }
        Ok(())
    }

    /// De velden die de cel van deze stroom kent, of de fout die zegt dat ze de
    /// stroom niet houdt.
    pub(crate) fn fields_of_stream(
        &self,
        cell: &str,
        subject: Subject,
        name: &str,
        stream: &str,
    ) -> Result<&BTreeSet<String>> {
        self.streams
            .get(stream)
            .ok_or_else(|| SimulatorError::UnknownStream {
                cell: cell.to_string(),
                subject,
                name: name.to_string(),
                stream: stream.to_string(),
                known: listing(self.streams.keys()),
            })
    }

    /// Het sleutelveld van een stroom; `None` als de cel haar niet houdt.
    pub(crate) fn stream_key(&self, stream: &str) -> Option<&str> {
        self.stream_keys.get(stream).map(String::as_str)
    }

    /// Kent deze stroom dit veld?
    pub(crate) fn check_stream_field(
        &self,
        cell: &str,
        subject: Subject,
        name: &str,
        stream: &str,
        field: &str,
    ) -> Result<()> {
        let fields = self.fields_of_stream(cell, subject, name, stream)?;
        if contains_name(fields, field) {
            return Ok(());
        }
        Err(SimulatorError::UnknownFilterField {
            cell: cell.to_string(),
            subject,
            name: name.to_string(),
            stream: stream.to_string(),
            field: field.to_string(),
            known: listing(fields),
        })
    }
}

/// De uitkomsten die een definitie publiceert: de uitkomst die de uitvoering
/// aanstuurt, plus wat `outputs` erbij noemt.
///
/// Eén plek voor de regel, want ze geldt voor een lexostatus en voor een
/// besluit: de aansturende uitkomst hoort er altijd bij — ze *is* het antwoord
/// of het besluit — dus `outputs` breidt uit en perkt niet in.
pub(crate) fn published_outputs<'a>(
    driving: Option<&'a str>,
    extra: &'a [String],
) -> BTreeSet<&'a str> {
    driving
        .into_iter()
        .chain(extra.iter().map(String::as_str))
        .collect()
}

/// Controleer de meegegeven parameters tegen de gedocumenteerde.
///
/// Weigert een ontbrekende parameter, een niet-gedocumenteerde parameter en een
/// parameter van het verkeerde type. Dat is wat "gedocumenteerde parameters"
/// waard maakt: de cel accepteert precies wat ze publiceert. Eén plek, want een
/// besluit is even strikt als een reductie.
pub(crate) fn check_documented_params(
    cell: &str,
    subject: Subject,
    name: &str,
    documented: &[DocumentedParameter],
    params: &BTreeMap<String, Value>,
) -> Result<()> {
    for supplied in params.keys() {
        if !documented.iter().any(|input| &input.name == supplied) {
            return Err(SimulatorError::UndocumentedParameter {
                cell: cell.to_string(),
                subject,
                name: name.to_string(),
                parameter: supplied.clone(),
                documented: parameter_listing(documented),
            });
        }
    }

    for input in documented {
        let Some(value) = params.get(&input.name) else {
            return Err(SimulatorError::MissingParameter {
                cell: cell.to_string(),
                subject,
                name: name.to_string(),
                parameter: input.name.clone(),
            });
        };
        if !input.value_type.accepts(value) {
            return Err(SimulatorError::ParameterType {
                cell: cell.to_string(),
                subject,
                name: name.to_string(),
                parameter: input.name.clone(),
                expected: input.value_type.label(),
                actual: value.type_name(),
            });
        }
    }

    Ok(())
}

/// Komma-gescheiden lijst van gedocumenteerde parameters, voor foutmeldingen.
pub(crate) fn parameter_listing(documented: &[DocumentedParameter]) -> String {
    documented
        .iter()
        .map(|input| input.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Documenteert deze lijst een parameter met deze naam?
pub(crate) fn documents(documented: &[DocumentedParameter], name: &str) -> bool {
    documented.iter().any(|input| input.name == name)
}

/// Komma-gescheiden opsomming voor een foutmelding.
fn listing<'a>(names: impl IntoIterator<Item = &'a String>) -> String {
    names
        .into_iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Kent deze verzameling de naam, hoofdletterongevoelig?
///
/// Zelfde souplesse als de kroniekstore en de engine: veldnamen matchen
/// hoofdletterongevoelig, dus een definitie afkeuren op een hoofdletter zou
/// weigeren wat bij het bevragen wél werkt.
fn contains_name(names: &BTreeSet<String>, name: &str) -> bool {
    names.iter().any(|known| known.eq_ignore_ascii_case(name))
}

impl LexostatusDefinition {
    /// De uitkomsten die deze lexostatus publiceert.
    ///
    /// Bij de wetsvorm altijd de `output` van de reductie, plus wat
    /// [`Self::outputs`] noemt; bij een kroniekfilter precies
    /// [`Self::outputs`]. Een consument krijgt precies deze namen te zien.
    pub fn published_outputs(&self) -> BTreeSet<&str> {
        let driving = match &self.reduction {
            Reduction::Law { output, .. } => Some(output.as_str()),
            Reduction::Chronicle { .. } => None,
        };
        published_outputs(driving, &self.outputs)
    }

    /// Laat van het antwoord van de engine alleen de gepubliceerde uitkomsten
    /// over.
    ///
    /// De engine levert bij een gevraagde uitkomst ook de uitkomsten die
    /// causaal met haar meekomen. Die zijn berekend, niet gepubliceerd, en
    /// horen dus niet in het antwoord van de cel.
    pub(crate) fn project(&self, outputs: BTreeMap<String, Value>) -> BTreeMap<String, Value> {
        let published = self.published_outputs();
        outputs
            .into_iter()
            .filter(|(name, _)| published.contains(name.as_str()))
            .collect()
    }

    /// Controleer de definitie tegen de cel waarin ze staat.
    ///
    /// Een definitie is een belofte aan een consument, dus alles wat die belofte
    /// niet waar kan maken blijkt hier — bij het optuigen van de cel, en niet
    /// pas bij de eerste vraag.
    ///
    /// `surface.outputs` bevat per regeling de uitkomstnamen van álle geladen
    /// versies. Een definitie afkeuren om een naam die alleen in de nieuwste
    /// versie ontbreekt, zou een scenario over een ouder moment onterecht
    /// blokkeren.
    pub(crate) fn validate(&self, cell: &str, surface: &CellSurface<'_>) -> Result<()> {
        match &self.reduction {
            Reduction::Law {
                regulation,
                parameters,
                ..
            } => self.validate_law(cell, surface, regulation, parameters),
            Reduction::Chronicle {
                chronicle,
                key,
                conditions,
            } => self.validate_chronicle(cell, surface, chronicle, key, conditions),
        }
    }

    /// De wetsvorm: eigen regeling, bestaande uitkomsten, bestaande parameters.
    fn validate_law(
        &self,
        cell: &str,
        surface: &CellSurface<'_>,
        regulation: &str,
        parameters: &BTreeMap<String, String>,
    ) -> Result<()> {
        surface.check_own_regulation(cell, Subject::Lexostatus, &self.name, regulation)?;
        surface.check_regulation_outputs(
            cell,
            Subject::Lexostatus,
            &self.name,
            regulation,
            self.published_outputs(),
        )?;

        for reference in parameters
            .values()
            .filter_map(|binding| binding_name(binding))
        {
            if !self.documents(reference) {
                return Err(SimulatorError::UnknownReference {
                    cell: cell.to_string(),
                    subject: Subject::Lexostatus,
                    name: self.name.clone(),
                    reference: reference.to_string(),
                });
            }
        }

        Ok(())
    }

    /// Het kroniekfilter: eigen stroom, bestaande velden, en een sleutel die de
    /// consument kan meegeven.
    fn validate_chronicle(
        &self,
        cell: &str,
        surface: &CellSurface<'_>,
        chronicle: &str,
        key: &str,
        conditions: &BTreeMap<String, Value>,
    ) -> Result<()> {
        let fields = surface.fields_of_stream(cell, Subject::Lexostatus, &self.name, chronicle)?;

        if self.outputs.is_empty() {
            return Err(SimulatorError::ChronicleWithoutOutputs {
                cell: cell.to_string(),
                lexostatus: self.name.clone(),
                stream: chronicle.to_string(),
            });
        }

        for published in self.published_outputs() {
            if !contains_name(fields, published) {
                return Err(SimulatorError::UnknownOutput {
                    cell: cell.to_string(),
                    subject: Subject::Lexostatus,
                    name: self.name.clone(),
                    origin: format!("kroniekstroom '{chronicle}'"),
                    output: published.to_string(),
                    known: listing(fields),
                });
            }
        }

        for field in std::iter::once(key).chain(conditions.keys().map(String::as_str)) {
            surface.check_stream_field(cell, Subject::Lexostatus, &self.name, chronicle, field)?;
        }

        // `where` vergelijkt met letterlijke waarden. Wie de `$naam`-vorm van de
        // wetsvorm hier overneemt, krijgt anders een filter dat de tekst `$naam`
        // zoekt en dus op elke vraag "niets vastgesteld" antwoordt — niet te
        // onderscheiden van een leeg verleden, en daarom hier een optuigfout.
        for (field, expected) in conditions {
            if let Some(reference) = expected.as_str().and_then(binding_name) {
                return Err(SimulatorError::FilterValueReference {
                    cell: cell.to_string(),
                    lexostatus: self.name.clone(),
                    field: field.clone(),
                    reference: reference.to_string(),
                });
            }
        }

        if !self.documents(key) {
            return Err(SimulatorError::ChronicleKeyWithoutParameter {
                cell: cell.to_string(),
                lexostatus: self.name.clone(),
                key: key.to_string(),
                documented: self.documented_parameters(),
            });
        }

        Ok(())
    }

    /// Documenteert deze lexostatus een parameter met deze naam?
    fn documents(&self, name: &str) -> bool {
        documents(&self.inputs, name)
    }

    /// Controleer de vraag van een consument tegen de gedocumenteerde parameters.
    ///
    /// Geldt voor beide reductievormen — een bron-cel zonder engine houdt haar
    /// consument even strikt aan de gepubliceerde vraag.
    pub(crate) fn check_params(&self, cell: &str, params: &BTreeMap<String, Value>) -> Result<()> {
        check_documented_params(cell, Subject::Lexostatus, &self.name, &self.inputs, params)
    }

    /// Komma-gescheiden lijst van gedocumenteerde parameters, voor foutmeldingen.
    fn documented_parameters(&self) -> String {
        parameter_listing(&self.inputs)
    }
}

/// Zet de gecontroleerde vraag om in parameters voor de engine.
///
/// Alleen de wetsvorm heeft dit nodig: een kroniekfilter praat niet met een
/// engine. Aanroepen ná [`LexostatusDefinition::check_params`] — een `$naam`
/// die daar niet doorkwam, landt hier als [`Value::Null`].
pub(crate) fn engine_parameters(
    bindings: &BTreeMap<String, String>,
    params: &BTreeMap<String, Value>,
) -> BTreeMap<String, Value> {
    bindings
        .iter()
        .map(|(name, binding)| {
            let value = match binding_name(binding) {
                Some(reference) => params.get(reference).cloned().unwrap_or(Value::Null),
                None => Value::String(binding.clone()),
            };
            (name.clone(), value)
        })
        .collect()
}

/// `"$bsn"` → `Some("bsn")`; alles zonder `$` is een letterlijke waarde.
pub(crate) fn binding_name(binding: &str) -> Option<&str> {
    binding.strip_prefix('$')
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Leest een reductie zoals de loader dat doet, en levert de foutmelding als
    /// tekst: deze tests gaan juist over wat er in die melding staat.
    fn reduction(yaml: &str) -> std::result::Result<Reduction, String> {
        serde_yaml_ng::from_str(yaml).map_err(|e| e.to_string())
    }

    #[test]
    fn de_wetsvorm_wordt_gelezen() {
        let parsed = reduction(
            r"
regulation: wet_op_de_zorgtoeslag
output: heeft_recht_op_zorgtoeslag
parameters:
  bsn: $bsn
",
        )
        .unwrap_or_else(|e| panic!("de wetsvorm moet gelezen worden: {e}"));
        let Reduction::Law { output, .. } = parsed else {
            panic!("verwachtte de wetsvorm, kreeg {parsed:?}");
        };
        assert_eq!(output, "heeft_recht_op_zorgtoeslag");
    }

    #[test]
    fn het_kroniekfilter_wordt_gelezen() {
        let parsed = reduction(
            r"
chronicle: relaties
key: bsn
latest: true
where:
  partnerschap_type: HUWELIJK
",
        )
        .unwrap_or_else(|e| panic!("het kroniekfilter moet gelezen worden: {e}"));
        let Reduction::Chronicle {
            chronicle,
            key,
            conditions,
        } = parsed
        else {
            panic!("verwachtte een kroniekfilter, kreeg {parsed:?}");
        };
        assert_eq!(chronicle, "relaties");
        assert_eq!(key, "bsn");
        assert_eq!(
            conditions.get("partnerschap_type"),
            Some(&Value::String("HUWELIJK".to_string()))
        );
    }

    #[test]
    fn latest_mag_weggelaten_worden() {
        assert!(
            reduction("chronicle: relaties\nkey: bsn\n").is_ok(),
            "`latest: true` is de enige modus, dus de afwezigheid ervan is geen keuze"
        );
    }

    #[test]
    fn twee_vormen_in_een_reductie_noemt_beide() {
        let err = reduction("regulation: wet_x\noutput: y\nchronicle: relaties\nkey: bsn\n")
            .expect_err("twee vormen in één reductie hoort te falen");
        assert!(
            err.contains("wet_x") && err.contains("relaties"),
            "de melding moet beide vormen noemen, kreeg: {err}"
        );
    }

    #[test]
    fn geen_van_beide_vormen_noemt_ze_beide() {
        let err = reduction("parameters:\n  bsn: $bsn\n")
            .expect_err("een reductie zonder vorm hoort te falen");
        assert!(
            err.contains("`regulation`") && err.contains("`chronicle`"),
            "de melding moet vertellen welke twee vormen er zijn, kreeg: {err}"
        );
    }

    #[test]
    fn de_wetsvorm_zonder_output_wordt_geweigerd() {
        let err = reduction("regulation: wet_x\n").expect_err("zonder `output` hoort het te falen");
        assert!(
            err.contains("`output`"),
            "de melding moet `output` noemen, kreeg: {err}"
        );
    }

    #[test]
    fn een_kroniekfilter_met_een_wetsveld_wordt_geweigerd() {
        let err = reduction("chronicle: relaties\nkey: bsn\noutput: heeft_partner\n")
            .expect_err("`output` bij een kroniekfilter hoort te falen");
        assert!(
            err.contains("`outputs`"),
            "de melding moet naar `outputs` wijzen, kreeg: {err}"
        );
    }

    #[test]
    fn een_wetsvorm_met_een_kroniekveld_wordt_geweigerd() {
        let err = reduction("regulation: wet_x\noutput: y\nlatest: true\n")
            .expect_err("`latest` bij de wetsvorm hoort te falen");
        assert!(
            err.contains("`latest`"),
            "de melding moet `latest` noemen, kreeg: {err}"
        );
    }

    #[test]
    fn aggregeren_bestaat_nog_niet_en_zegt_dat() {
        let err = reduction("chronicle: relaties\nkey: bsn\nlatest: false\n")
            .expect_err("`latest: false` hoort te falen zolang er niets te aggregeren valt");
        assert!(
            err.contains("aggregeren"),
            "de melding moet zeggen wat er ontbreekt, kreeg: {err}"
        );
    }

    #[test]
    fn een_kroniekfilter_zonder_sleutel_wordt_geweigerd() {
        let err = reduction("chronicle: relaties\n").expect_err("zonder `key` hoort het te falen");
        assert!(
            err.contains("`key`"),
            "de melding moet `key` noemen, kreeg: {err}"
        );
    }
}
