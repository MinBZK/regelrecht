//! Het **schema** van het decretogram dat een besluit-definitie kan voortbrengen.
//!
//! Een decretogram krijgt zijn velden uit drie bronnen, en dat is precies wat
//! RFC-022 als vraag openlaat: welk deel van een besluit volgt uit de wet?
//!
//! - het **lexogram**: de uitkomsten die het uitvoerende artikel declareert
//!   (`output[]` met hun type, `produces` met het rechtskarakter en de
//!   verplichtingen die het oplegt, `competent_authority` op het artikel of op
//!   het document);
//! - het **wereldbestand**: welke uitkomsten samen één gram vormen en het
//!   zaakkenmerk-sjabloon;
//! - het **platform**: de omslag eromheen — wanneer, door wie, waarop gerekend
//!   is, en het receipt.
//!
//! Dit schema zegt per veld welke van de drie het is. Het verandert niets aan
//! een besluit; het maakt meetbaar wat er nu waar staat, zodat de vervolgstappen
//! van RFC-022 — normatieve inhoud hoort in het lexogram — te zien zijn als
//! velden die van [`Herkomst::Wereldbestand`] naar [`Herkomst::Lexogram`]
//! verschuiven. Een veld van het wereldbestand draagt daarom [`Herkomst::gat`]:
//! de wet zwijgt erover terwijl ze het zou moeten zeggen.
//!
//! **Op de nieuwste geladen versie.** Het schema wordt bij het optuigen
//! uitgerekend en er is dan geen moment in het spel; het lexogram dat het noemt
//! is dus de nieuwste versie die de cel geladen heeft, en de versie staat er
//! daarom bij. Een besluit over een ouder moment landt op een oudere versie en
//! kan een ander schema hebben — het gram zelf zegt onder welke versie het
//! genomen is (`regulation_valid_from`), en dat blijft de bron voor wat er
//! werkelijk gold.

use crate::cell::besluit::{
    fixed_fields, BesluitDefinition, DeclaredObligations, ObligationDefinition, ObligationOrigin,
    AFWIJZING, AFWIJZINGSGROND, BESCHIKKINGEN, BESLUIT, BEVOEGD_GEZAG_REFERENCE, CHRONICLE_SOURCES,
    COMPETENT_AUTHORITY, DECISION_TYPE, EXECUTED_REGULATIONS, HOOK_NIET_UITGEVOERD, INPUTS,
    LEGAL_CHARACTER, NIETS_TE_BETALEN, OBLIGATIONS, RECEIPT, REGULATION_VALID_FROM, STAGE,
    STAGE_BESLUIT, TERUGVORDERING, VANAF_BEKENDMAKING, WACHT_OP_BEKENDMAKING, ZAAKKENMERK,
};
use crate::cell::extensions::{afwijzing_wanneer, ChronolexBlock};
use regelrecht_engine::article::Produces;
use regelrecht_engine::{
    Article, ArticleBasedLaw, LawExecutionService, ParameterType as EngineType, RegulatoryLayer,
};
use serde::Serialize;
use std::collections::BTreeMap;

/// Het veld met het moment waarop besloten is.
///
/// Anders dan de velden in [`fixed_fields`] staat het niet *in* de veldenlijst
/// van het gram maar op het gram zelf ([`crate::ChronicleEvent::op_moment`]) —
/// een decretogram is een gewoon executogram en draagt zijn moment zoals elk
/// ander gram. Voor een lezer van het schema is dat verschil er niet: het is een
/// waarde die elk decretogram draagt, dus ze staat erin.
const OP_MOMENT: &str = "op_moment";

/// Waar één veld van een decretogram vandaan komt.
///
/// Vier antwoorden, en de eerste twee zijn hetzelfde antwoord met een ander
/// gewicht: `beleid` is een lexogram waarvan de regeling
/// `regulatory_layer: UITVOERINGSBELEID` draagt. Dat apart te kunnen zien is het
/// punt — een uitkomst die uit een uitvoeringsregel volgt, volgt niet uit de wet,
/// en wie meet hoeveel van een besluit uit het recht komt, hoort die twee niet
/// bij elkaar op te tellen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Herkomst {
    /// Een regeling declareert dit veld.
    Lexogram,
    /// Een **uitvoeringsregel** declareert dit veld
    /// (`regulatory_layer: UITVOERINGSBELEID`).
    Beleid,
    /// Het wereldbestand zegt het, en geen enkele regeling: een **gat**.
    Wereldbestand,
    /// Het platform zet het in elk decretogram.
    Platform,
}

impl Herkomst {
    /// Is een veld met deze herkomst een gat?
    ///
    /// Alleen bij [`Self::Wereldbestand`]. De reden staat in de README van deze
    /// crate: het wereldbestand is configuratie van de opstelling, en normatieve
    /// inhoud die daar staat, staat buiten het recht dat ze uitdraagt. Een veld
    /// van het platform is géén gat — dat een gram zijn eigen moment en zijn
    /// eigen receipt draagt, is geen norm die een wet had moeten stellen.
    #[must_use]
    pub fn gat(self) -> bool {
        matches!(self, Self::Wereldbestand)
    }

    /// De herkomst van een veld dat door een regeling van deze laag gedeclareerd
    /// wordt.
    fn of_layer(layer: RegulatoryLayer) -> Self {
        match layer {
            RegulatoryLayer::Uitvoeringsbeleid => Self::Beleid,
            _ => Self::Lexogram,
        }
    }
}

/// Het lexogram dat een veld declareert: welke regeling, welke versie, welk
/// artikel.
///
/// Het artikel mag ontbreken: een `competent_authority` op documentniveau is een
/// declaratie van de regeling als geheel (RFC-002 laat beide toe, en het corpus
/// gebruikt beide). Dat er dan `null` staat en niet een geraden artikelnummer,
/// is het verschil tussen "de wet zegt het hier" en "de wet zegt het ergens".
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LexogramRef {
    /// De regeling, bij `$id`.
    pub regulation: String,
    /// De `valid_from` van de versie waarop dit schema staat.
    pub valid_from: Option<String>,
    /// Het artikel dat het veld declareert; `null` bij een declaratie op het
    /// document.
    pub article: Option<String>,
    /// De laag van de regeling (`WET`, `UITVOERINGSBELEID`, …), zoals het
    /// law-model haar noemt.
    pub regulatory_layer: String,
}

/// Eén veld van het decretogram dat een besluit kan voortbrengen.
///
/// Het is een **schema** en geen waarde: hier staat wat er in zo'n gram komt te
/// staan en waar dat vandaan komt, niet wat er in een bepaald gram stáát. Dat
/// laatste is het gram zelf, met zijn eigen herkomst per waarde
/// ([`crate::FieldOrigin`]) — die zegt waar een waarde vandaan kwam, deze zegt
/// wie het veld declareert.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DecretogramField {
    /// De veldnaam zoals ze in het gram komt te staan.
    pub name: String,
    /// Het type, in de woorden van de engine (`string`, `number`, `boolean`,
    /// `date`, `amount`, `array`, `object`); afwezig als geen enkele geladen
    /// versie het veld declareert.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub value_type: Option<String>,
    /// De eenheid uit het `type_spec` van de wet (`eurocent`, `ratio`, …).
    ///
    /// Een label en nooit een rekenregel (RFC-023), maar wel het verschil tussen
    /// een bedrag in centen en een bedrag in euro's — dus het hoort bij het type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// Wie dit veld declareert.
    pub herkomst: Herkomst,
    /// Dekt geen enkel lexogram dit veld? Zie [`Herkomst::gat`].
    pub gat: bool,
    /// Het lexogram dat het veld declareert, als er een is.
    ///
    /// Ook bij [`Herkomst::Platform`] gevuld waar de wet de waarde levert: het
    /// platform zet `competent_authority` en `legal_character` in élk
    /// decretogram — dat is wat ze tot platformvelden maakt — maar het *leest* ze
    /// uit de regeling, en welk artikel dat zegt hoort erbij te staan.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lexogram: Option<LexogramRef>,
    /// Waarom dit veld hier staat, in één regel; alleen waar dat niet uit de
    /// naam volgt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub toelichting: Option<String>,
}

impl DecretogramField {
    /// Een veld zonder lexogram: het platform of het wereldbestand zegt het.
    ///
    /// Of het een gat is, zegt de herkomst zelf ([`Herkomst::gat`]) en niet de
    /// roepplek: één regel die bepaalt wat een gat is, zodat een vijfde herkomst
    /// of een verschoven grens niet op drie plekken nagelopen hoeft te worden.
    fn declared_by(
        herkomst: Herkomst,
        name: &str,
        value_type: &str,
        toelichting: Option<String>,
    ) -> Self {
        Self {
            name: name.to_string(),
            value_type: Some(value_type.to_string()),
            unit: None,
            herkomst,
            gat: herkomst.gat(),
            lexogram: None,
            toelichting,
        }
    }

    /// Een veld dat het platform in elk decretogram zet.
    fn platform(name: &str, value_type: &str) -> Self {
        Self::declared_by(Herkomst::Platform, name, value_type, None)
    }

    /// Een veld dat het wereldbestand zegt, en geen enkele regeling.
    fn wereldbestand(name: &str, value_type: &str, toelichting: String) -> Self {
        Self::declared_by(Herkomst::Wereldbestand, name, value_type, Some(toelichting))
    }

    /// Een veld dat een regeling declareert, met de plek waar ze dat doet.
    ///
    /// Anders dan [`Self::read_from`], dat een platformveld de plek meegeeft
    /// waar het zijn waarde leest: hier is de regeling de declarant, en de
    /// herkomst volgt daarom uit haar laag ([`Herkomst::of_layer`]).
    fn from_lexogram(
        name: &str,
        value_type: &str,
        layer: RegulatoryLayer,
        lexogram: LexogramRef,
        toelichting: String,
    ) -> Self {
        let herkomst = Herkomst::of_layer(layer);
        Self {
            name: name.to_string(),
            value_type: Some(value_type.to_string()),
            unit: None,
            herkomst,
            gat: herkomst.gat(),
            lexogram: Some(lexogram),
            toelichting: Some(toelichting),
        }
    }

    /// Hetzelfde veld, met de plek in de wet waar het platform zijn waarde leest.
    fn read_from(mut self, lexogram: Option<LexogramRef>, toelichting: &str) -> Self {
        if lexogram.is_some() {
            self.toelichting = Some(toelichting.to_string());
        }
        self.lexogram = lexogram;
        self
    }
}

/// Het type zoals de engine het noemt.
fn type_name(value_type: EngineType) -> &'static str {
    match value_type {
        EngineType::String => "string",
        EngineType::Number => "number",
        EngineType::Boolean => "boolean",
        EngineType::Amount => "amount",
        EngineType::Date => "date",
        EngineType::Array => "array",
        EngineType::Object => "object",
    }
}

/// De wet waarop dit schema staat, en hoe daarin iets op te zoeken is.
///
/// Eén plek voor de drie opzoekingen die het schema doet — welk artikel
/// declareert deze uitkomst, wat is haar type, en waar staat het bevoegd gezag —
/// zodat ze alle drie op dezelfde versie uitkomen. Zouden ze elk hun eigen versie
/// kiezen, dan zou het schema een artikel uit de ene versie naast een type uit de
/// andere zetten.
struct Lexicon<'a> {
    /// De regeling waarop het besluit gaat, bij `$id`.
    regulation: &'a str,
    /// De nieuwste geladen versie ervan; `None` als de cel haar niet laadt.
    law: Option<&'a ArticleBasedLaw>,
    /// De engine van de cel, voor de artikel-index op uitkomstnaam.
    service: Option<&'a LawExecutionService>,
}

impl<'a> Lexicon<'a> {
    /// Het lexicon van deze besluit-definitie.
    fn new(definition: &'a BesluitDefinition, service: Option<&'a LawExecutionService>) -> Self {
        Self {
            regulation: &definition.regulation,
            // `None` als moment: het schema staat op de nieuwste geladen versie.
            // Zie de moduledocs.
            law: service.and_then(|service| {
                service
                    .resolver()
                    .get_law_for_date(&definition.regulation, None)
            }),
            service,
        }
    }

    /// De laag van de regeling; `WET` als ze niet geladen is — dan is er toch
    /// geen lexogram om naar te wijzen.
    fn layer(&self) -> RegulatoryLayer {
        self.law
            .map_or(RegulatoryLayer::Wet, |law| law.regulatory_layer)
    }

    /// Een verwijzing naar deze regeling, op dit artikel (of op het document).
    fn reference(&self, article: Option<&str>) -> LexogramRef {
        LexogramRef {
            regulation: self.regulation.to_string(),
            valid_from: self.law.and_then(|law| law.valid_from.clone()),
            article: article.map(str::to_string),
            regulatory_layer: self.layer().as_str().to_string(),
        }
    }

    /// Het artikel dat deze uitkomst voortbrengt.
    ///
    /// Langs de resolver van de engine, dezelfde weg waarlangs
    /// [`crate::Cell::decide`] het bevoegd gezag op artikelniveau opzoekt: één
    /// antwoord op de vraag "welk artikel produceert dit?", en niet een tweede
    /// dat ernaast kan gaan lopen.
    fn article_for(&self, output: &str) -> Option<&'a Article> {
        self.service?
            .resolver()
            .get_article_by_output(self.regulation, output, None)
    }

    /// Het veld voor één uitkomst van het besluit: haar type, haar eenheid en het
    /// artikel dat haar declareert.
    fn output_field(&self, output: &str) -> DecretogramField {
        let article = self.article_for(output);
        let declared = article
            .and_then(Article::get_execution_spec)
            .and_then(|execution| execution.output.as_ref())
            .and_then(|outputs| outputs.iter().find(|declared| declared.name == output));
        let herkomst = Herkomst::of_layer(self.layer());
        DecretogramField {
            name: output.to_string(),
            value_type: declared.map(|declared| type_name(declared.output_type).to_string()),
            unit: declared
                .and_then(|declared| declared.type_spec.as_ref())
                .and_then(|spec| spec.unit.clone()),
            herkomst,
            gat: herkomst.gat(),
            lexogram: Some(self.reference(article.map(|article| article.number.as_str()))),
            toelichting: match declared {
                Some(_) => None,
                // Elke uitkomst van een besluit is bij het optuigen tegen de
                // regeling gehouden (zie `check_regulation_outputs`), dus ze staat
                // in ténminste één geladen versie. Staat ze niet in de nieuwste,
                // dan is dat iets om te zien en niet om te verzwijgen.
                None => Some(
                    "geen artikel in deze versie declareert deze uitkomst; ze komt uit \
                     een andere geladen versie"
                        .to_string(),
                ),
            },
        }
    }

    /// Waar het bevoegd gezag vandaan komt: het artikel dat de aansturende
    /// uitkomst voortbrengt, anders het document (RFC-002-volgorde, zoals
    /// [`crate::Cell::decide`] hem toepast); `None` als de regeling zwijgt.
    fn authority_reference(&self, driving: &str) -> Option<LexogramRef> {
        let article = self.article_for(driving);
        let on_article = article
            .and_then(|article| article.machine_readable.as_ref())
            .and_then(|machine_readable| machine_readable.competent_authority.as_ref())
            .is_some();
        if on_article {
            return Some(self.reference(article.map(|article| article.number.as_str())));
        }
        self.law?.competent_authority.as_ref()?;
        Some(self.reference(None))
    }

    /// Het artikel waarvan `produces` gelezen is: dat van de aansturende
    /// uitkomst.
    fn produces_reference(&self, driving: &str) -> Option<LexogramRef> {
        let article = self.article_for(driving)?;
        article.get_execution_spec()?.produces.as_ref()?;
        Some(self.reference(Some(&article.number)))
    }

    /// Het `produces` van het artikel achter de aansturende uitkomst.
    fn produces(&self, driving: &str) -> Option<&'a Produces> {
        self.article_for(driving)?
            .get_execution_spec()?
            .produces
            .as_ref()
    }

    /// De afwijzingsvoorwaarden die dat artikel declareert.
    ///
    /// Leeg als er geen blok staat — of als het niet te lezen is, en dat laatste
    /// kan hier niet: het schema wordt ná `validate` uitgerekend, en dáár valt een
    /// blok dat niet te lezen is.
    fn afwijzing_wanneer(&self, driving: &str) -> BTreeMap<String, bool> {
        ChronolexBlock::read(self.produces(driving), &self.origin(driving))
            .ok()
            .and_then(|block| block.afwijzing_wanneer)
            .as_ref()
            .map(afwijzing_wanneer)
            .and_then(std::result::Result::ok)
            .unwrap_or_default()
    }

    /// Waar een `chronolex`-blok van deze uitkomst staat: regeling, versie en
    /// artikel.
    ///
    /// Alleen voor de melding als het blok niet te lezen is; hier komt die
    /// melding niet naar buiten (zie [`Self::afwijzing_wanneer`]), maar de lezer
    /// vraagt erom en er is er maar één.
    fn origin(&self, driving: &str) -> ObligationOrigin {
        ObligationOrigin {
            regulation: self.regulation.to_string(),
            valid_from: self.law.and_then(|law| law.valid_from.clone()),
            article: self
                .article_for(driving)
                .map_or_else(String::new, |article| article.number.clone()),
        }
    }

    /// Een veld dat een regeling zelf declareert, bij het artikel dat het zegt.
    fn declared(
        &self,
        name: &str,
        value_type: &str,
        article: Option<&str>,
        toelichting: String,
    ) -> DecretogramField {
        let herkomst = Herkomst::of_layer(self.layer());
        DecretogramField {
            name: name.to_string(),
            value_type: Some(value_type.to_string()),
            unit: None,
            herkomst,
            gat: herkomst.gat(),
            lexogram: Some(self.reference(article)),
            toelichting: Some(toelichting),
        }
    }

    /// Wat het uitvoerende artikel aan verplichtingen oplegt, met de plek waar
    /// het dat zegt.
    ///
    /// Dezelfde opzoeking als bij `produces`, want het staat er ook: een
    /// verplichting hangt aan het artikel dat de beschikking voortbrengt
    /// (`produces.extensions.chronolex`). Een blok dat niet te lezen is komt
    /// hier niet langs — [`crate::Cell::from_config`] leest elk blok van elke
    /// geladen versie voordat er een schema wordt uitgerekend, en weigert de cel
    /// als er één niet klopt.
    fn obligations(&self, driving: &str) -> Option<Declared> {
        let article = self.article_for(driving)?;
        let declared =
            DeclaredObligations::from_article(self.origin(driving), None, article).ok()?;
        if declared.is_empty() {
            return None;
        }
        Some(Declared {
            lexogram: self.reference(Some(&article.number)),
            waits_for_bekendmaking: declared
                .items
                .iter()
                .any(|item| item.vanaf.as_deref() == Some(VANAF_BEKENDMAKING)),
            items: declared.items,
        })
    }
}

/// De verplichtingen van het uitvoerende artikel, met de plek waar ze staan.
///
/// De twee samen, omdat ze samen één antwoord zijn: welke verplichtingen er
/// zijn en welk artikel ze declareert. Los doorgegeven kon een schema de
/// verplichting van het ene artikel naast de vindplaats van het andere zetten.
struct Declared {
    /// Het artikel dat de verplichtingen declareert.
    lexogram: LexogramRef,
    /// Wat het oplegt, in de volgorde van het artikel.
    items: Vec<ObligationDefinition>,
    /// Wacht er een van die verplichtingen op de bekendmaking?
    ///
    /// Bepaalt of het gram een gevulde [`WACHT_OP_BEKENDMAKING`] kan dragen, en
    /// dus of dat veld naar het lexogram wijst of leeg blijft. Uit de declaratie
    /// en niet uit een gram: het schema bestaat vóór het eerste besluit.
    waits_for_bekendmaking: bool,
}

/// Het schema van het decretogram dat deze besluit-definitie kan voortbrengen.
///
/// De volgorde is die van het gram zelf: eerst wat het besluit vaststelt (de
/// aansturende uitkomst, dan wat `outputs` erbij noemt), dan de verplichtingen
/// die eruit volgen, dan de omslag die elk decretogram draagt. Vast, zodat het
/// beeld van de wereld een contract blijft.
pub(crate) fn decretogram_schema(
    definition: &BesluitDefinition,
    service: Option<&LawExecutionService>,
) -> Vec<DecretogramField> {
    let lexicon = Lexicon::new(definition, service);
    let mut schema = Vec::new();

    // De uitkomsten: de aansturende voorop, want díe uitkomst *is* het besluit.
    let mut seen: Vec<&str> = Vec::new();
    for output in std::iter::once(definition.output.as_str())
        .chain(definition.outputs.iter().map(String::as_str))
    {
        if seen.contains(&output) {
            continue;
        }
        seen.push(output);
        schema.push(lexicon.output_field(output));
    }

    // Elke verplichting apart, want elke verplichting is een eigen belofte: twee
    // partijen, een bedrag, een ritme en de grondslag waarop ze berust. Ze staan
    // in het artikel dat de beschikking voortbrengt en niet in het wereldbestand,
    // dus ze zijn geen gat: wat het wereldbestand er nog over zegt, is wélke cel
    // de schuldenaar nakomt (`komt_na`) en niet wat er opgelegd wordt of aan wie.
    let declared = lexicon.obligations(&definition.output);
    for (index, obligation) in declared.iter().flat_map(|found| &found.items).enumerate() {
        let vanaf = match obligation.vanaf.as_deref() {
            Some(vanaf) => format!(", vanaf '{vanaf}'"),
            None => String::new(),
        };
        // De standaarden staan er met zoveel woorden bij: wie ze weglaat, hoort in
        // het schema te kunnen lezen wat er dan geldt.
        let schuldenaar = obligation
            .schuldenaar
            .clone()
            .unwrap_or_else(|| format!("{BEVOEGD_GEZAG_REFERENCE} (standaard)"));
        let schuldeiser = obligation.schuldeiser.clone().unwrap_or_else(|| {
            format!(
                "de parameter uit zaakkenmerk '{}' (standaard)",
                definition.zaakkenmerk
            )
        });
        let omkeren = match obligation.richting_bij_negatief {
            Some(_) => format!(
                "; een negatief bedrag keert de richting om en wordt een \
                 '{TERUGVORDERING}'"
            ),
            None => String::new(),
        };
        // Onbereikbaar leeg: de lus loopt over de items van dít antwoord.
        let Some(found) = &declared else { break };
        schema.push(DecretogramField::from_lexogram(
            &format!("{OBLIGATIONS}[{index}]"),
            // Een verplichting is er één, geen lijst: een soort, een bedrag en
            // een ritme. Wat zij in het gram wordt — een reeks termijnen — staat
            // in `obligations` hieronder, en dát veld is de array.
            "object",
            lexicon.layer(),
            found.lexogram.clone(),
            format!(
                "verplichting van soort '{}': schuldenaar {schuldenaar}, schuldeiser \
                 {schuldeiser}, bedrag {}, ritme '{}'{vanaf}, op grondslag '{}'{omkeren}; \
                 zij levert de termijnen in '{OBLIGATIONS}'. Wélke cel de schuldenaar \
                 nakomt, staat in het wereldbestand (`komt_na`)",
                obligation.soort, obligation.bedrag, obligation.ritme, obligation.grondslag
            ),
        ));
    }

    // De vaste velden, in de volgorde waarin het gram ze draagt, met het moment
    // voorop: dat zegt wanneer dit alles gold.
    schema.push(DecretogramField::platform(OP_MOMENT, "date"));
    for field in fixed_fields().iter().copied() {
        schema.push(fixed_field(field, definition, &lexicon, declared.as_ref()));
    }
    schema
}

/// Het veld met het besluittype: wélk besluit dit is.
///
/// Anders dan [`LEGAL_CHARACTER`] een veld van de **wet** en niet van het
/// platform. Het rechtskarakter is altijd `BESCHIKKING` — dat is wat een
/// decretogram tot een decretogram maakt, en het platform weigert elke andere
/// waarde — maar wát dit besluit is, zegt het artikel: `decision_type` voor de
/// gewone afloop, en [`AFWIJZING`] zodra een voorwaarde uit zijn
/// `afwijzing_wanneer` vervuld is. Het platform kiest daar niets in.
///
/// Zegt het artikel geen van beide, dan draagt het gram `null` en is er niets uit
/// de wet gelezen: dan is het een platformveld dat leeg blijft.
fn decision_type_field(definition: &BesluitDefinition, lexicon: &Lexicon<'_>) -> DecretogramField {
    let driving = definition.output.as_str();
    let produces = lexicon.produces(driving);
    let declared = produces.and_then(|produces| produces.decision_type.as_deref());
    let conditions = lexicon.afwijzing_wanneer(driving);
    if declared.is_none() && conditions.is_empty() {
        return DecretogramField::platform(DECISION_TYPE, "string");
    }

    let gewoon = match declared {
        Some(declared) => format!("'{declared}'"),
        None => "leeg".to_string(),
    };
    let afwijzing = match conditions.is_empty() {
        true => ", en deze regeling wijst niet af".to_string(),
        false => format!(", of '{AFWIJZING}' als een afwijzingsvoorwaarde vervuld is"),
    };
    lexicon.declared(
        DECISION_TYPE,
        "string",
        lexicon
            .article_for(driving)
            .map(|article| article.number.as_str()),
        format!("{gewoon} zolang het besluit doorgaat{afwijzing}"),
    )
}

/// Het veld met de vervulde afwijzingsvoorwaarden.
///
/// Wat erin kan komen te staan, staat helemaal in de wet: welke uitkomst op welke
/// waarde tot een weigering leidt, declareert het artikel in
/// `produces.extensions.chronolex.afwijzing_wanneer`. Het lexogram wijst naar het
/// artikel dat de afwijzende uitkomst voortbrengt — dát is de grondslag van de
/// weigering — en bij meer dan één voorwaarde naar het artikel waarop het blok
/// staat, met de andere in de toelichting.
///
/// Wijst de regeling niet af, dan blijft het veld leeg en is er niets uit de wet
/// gelezen: een platformveld, en geen gat — dat een regeling geen weigering kent,
/// is geen norm die ze had moeten stellen.
fn afwijzingsgrond_field(
    definition: &BesluitDefinition,
    lexicon: &Lexicon<'_>,
) -> DecretogramField {
    let driving = definition.output.as_str();
    let conditions = lexicon.afwijzing_wanneer(driving);
    let Some(first) = conditions.keys().next() else {
        let mut field = DecretogramField::platform(AFWIJZINGSGROND, "array");
        field.toelichting = Some("leeg: deze regeling kent geen afwijzingsgrond".to_string());
        return field;
    };

    let genoemd = conditions
        .iter()
        .map(|(output, waarde)| {
            let article = match lexicon.article_for(output) {
                Some(article) => format!(" (artikel {})", article.number),
                None => String::new(),
            };
            format!("{output} = {waarde}{article}")
        })
        .collect::<Vec<_>>()
        .join(", ");
    // Eén voorwaarde: het artikel dat háár voortbrengt is de grondslag. Meer dan
    // één: die kunnen uit verschillende artikelen komen, en dan is het artikel
    // waarop het blok staat het enige dat ze alle noemt.
    let article = match conditions.len() {
        1 => lexicon
            .article_for(first)
            .map(|article| article.number.to_string()),
        _ => lexicon
            .article_for(driving)
            .map(|article| article.number.to_string()),
    };
    lexicon.declared(
        AFWIJZINGSGROND,
        "array",
        article.as_deref(),
        format!("gevuld zodra een van deze voorwaarden vervuld is: {genoemd}"),
    )
}

/// Eén vast veld van het decretogram: wat het draagt, en wie het zegt.
fn fixed_field(
    field: &str,
    definition: &BesluitDefinition,
    lexicon: &Lexicon<'_>,
    declared: Option<&Declared>,
) -> DecretogramField {
    match field {
        ZAAKKENMERK => DecretogramField::wereldbestand(
            field,
            "string",
            format!(
                "sjabloon '{}'; waaronder de zaak in kroniek '{BESCHIKKINGEN}' terugkomt",
                definition.zaakkenmerk
            ),
        ),
        BESLUIT => DecretogramField::wereldbestand(
            field,
            "string",
            format!(
                "besluit-definitie '{}': welke uitkomsten samen één gram vormen, staat in \
                 het wereldbestand",
                definition.name
            ),
        ),
        // De termijnen volgen uit wat het artikel oplegt, dus dit veld volgt de
        // verplichtingen hierboven: geen gat meer, maar een lexogram. Legt het
        // artikel niets op, dan blijft er een leeg veld over dat het platform in
        // elk gram zet — er is dan geen artikel om naar te wijzen, en een leeg
        // vak is ook geen gat in de wet.
        OBLIGATIONS => match declared {
            Some(declared) => DecretogramField::from_lexogram(
                field,
                "array",
                lexicon.layer(),
                declared.lexogram.clone(),
                "de termijnen die uit de verplichtingen hierboven volgen, elk met haar \
                 schuldenaar, haar schuldeiser en de cel die haar nakomt"
                    .to_string(),
            ),
            None => DecretogramField::declared_by(
                Herkomst::Platform,
                field,
                "array",
                Some(
                    "leeg: het artikel dat dit besluit uitvoert legt geen verplichting op"
                        .to_string(),
                ),
            ),
        },
        // De stage waarin dit gram ontstond. Wélke stages er zijn, zegt de
        // algemene wet met haar procedure (RFC-008); dát elk gram er een draagt,
        // is van het platform — en in dit gram staat er altijd de eerste, want
        // een decretogram *is* het besluit.
        STAGE => DecretogramField::declared_by(
            Herkomst::Platform,
            field,
            "string",
            Some(format!(
                "'{STAGE_BESLUIT}': elke stage van de procedure legt een eigen gram op \
                 hetzelfde zaakkenmerk"
            )),
        ),
        // Wat dit besluit oplegde maar nog niet kon inroosteren. Volgt de
        // verplichtingen hierboven, net als [`OBLIGATIONS`]: legt het artikel
        // niets op dat op de bekendmaking wacht, dan blijft er een leeg veld over.
        WACHT_OP_BEKENDMAKING => {
            match declared.filter(|declared| declared.waits_for_bekendmaking) {
                Some(declared) => DecretogramField::from_lexogram(
                    field,
                    "array",
                    lexicon.layer(),
                    declared.lexogram.clone(),
                    format!(
                        "de verplichtingen met `vanaf: {VANAF_BEKENDMAKING}`: bedrag en partijen \
                     staan vast, de vervaldatum volgt uit de bekendmaking"
                    ),
                ),
                None => DecretogramField::declared_by(
                    Herkomst::Platform,
                    field,
                    "array",
                    Some(format!(
                        "leeg: geen verplichting van dit artikel wacht op de bekendmaking \
                     (`vanaf: {VANAF_BEKENDMAKING}`)"
                    )),
                ),
            }
        }
        // Wat er van een verplichting overblijft als haar bedrag op nul uitkwam.
        // Volgt de verplichtingen hierboven, net als [`OBLIGATIONS`]: elke
        // verplichting kan op nul uitkomen, dus zodra het artikel er een oplegt
        // wijst dit veld naar dat artikel.
        NIETS_TE_BETALEN => match declared {
            Some(declared) => DecretogramField::from_lexogram(
                field,
                "array",
                lexicon.layer(),
                declared.lexogram.clone(),
                "de verplichtingen hierboven waarvan het bedrag op nul uitkwam: met \
                 `bedrag: 0` en zonder termijnen, want er valt niets te betalen"
                    .to_string(),
            ),
            None => DecretogramField::declared_by(
                Herkomst::Platform,
                field,
                "array",
                Some(
                    "leeg: het artikel dat dit besluit uitvoert legt geen verplichting op"
                        .to_string(),
                ),
            ),
        },
        COMPETENT_AUTHORITY => DecretogramField::platform(field, "string").read_from(
            lexicon.authority_reference(&definition.output),
            "het platform schrijft het in elk gram; de regeling wijst het aan (RFC-002)",
        ),
        LEGAL_CHARACTER => DecretogramField::platform(field, "string").read_from(
            lexicon.produces_reference(&definition.output),
            "het platform schrijft het in elk gram; het artikel zegt het in `produces`",
        ),
        DECISION_TYPE => decision_type_field(definition, lexicon),
        AFWIJZINGSGROND => afwijzingsgrond_field(definition, lexicon),
        REGULATION_VALID_FROM => DecretogramField::platform(field, "date"),
        EXECUTED_REGULATIONS | CHRONICLE_SOURCES => DecretogramField::platform(field, "array"),
        // Welke hooks er vuren, zegt de wet; dát een hook die niet kon draaien in
        // het gram genoemd wordt in plaats van het besluit te laten omvallen, is
        // van het platform.
        HOOK_NIET_UITGEVOERD => DecretogramField::declared_by(
            Herkomst::Platform,
            field,
            "array",
            Some(
                "de hooks die op dit besluit vuurden maar niet draaiden, elk met artikel en \
                 ontbrekende input; leeg als elke hook draaide"
                    .to_string(),
            ),
        ),
        INPUTS | RECEIPT => DecretogramField::platform(field, "object"),
        // `regulation`, `besloten_door` en wat er ooit bij komt: tekst die het
        // platform zelf opschrijft.
        _ => DecretogramField::platform(field, "string"),
    }
}
