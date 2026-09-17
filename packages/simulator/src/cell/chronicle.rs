//! De kroniekstore van een cel: de feiten die de cel zélf bijhoudt.
//!
//! Een kroniekstroom is een tijdgeordende groepering van vastleggingen over
//! hetzelfde soort onderwerp, gesleuteld op één veld (meestal een BSN). Elke
//! vastlegging draagt het moment waarop het feit feit werd — de as waarop een
//! reductie ordent (RFC-022 §4.1).
//!
//! Een kroniek bevat **alleen wat de cel zelf overkwam**: een aanvraag die
//! binnenkwam, een levering die ze ontving, een betaling die ze deed, een
//! besluit dat ze zelf nam. Geen schaduwboekhouding van waarden die ze elders
//! ophaalde — dat zou het totaalbeeld dat volgens RFC-022 nergens bestaat
//! alsnog in één cel leggen.
//!
//! De store is bewust *niet* publiek bereikbaar vanaf een cel: zie
//! [`crate::cell::Cell`].

use super::config::{DocumentedParameter, Mismatch, ParameterType};
use super::reductie::Gemist;
use crate::error::{Result, SimulatorError};
use crate::values::equivalent;
use chrono::NaiveDate;
use regelrecht_engine::Value;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

/// Het kanaal waarlangs een feit de cel binnenkwam of in de cel ontstond.
///
/// RFC-022 §1.3 noemt dit `intake` en houdt het vocabulaire open. Hier is het
/// een enum, want een typfout in een kanaalnaam mag niet stil doorgaan: dan
/// staat er een vastlegging in de kroniek waarvan niemand meer kan zeggen
/// waarlangs ze binnenkwam. Een kanaal erbij is één regel — het is
/// platformvocabulaire, geen casusdata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Intake {
    /// Iemand vroeg iets aan bij deze cel.
    Aanvraag,
    /// Een andere partij leverde een feit aan deze cel.
    Levering,
    /// Er werd door of aan deze cel betaald.
    Betaling,
    /// De cel nam zelf een besluit en legde dat vast.
    EigenBesluit,
}

/// Het gebeurtenisschema van een stroom: wat een gebeurtenis van deze naam
/// vastlegt.
///
/// Het typeschema van een executogram is generiek en compile-time — het geldt
/// voor elke vastlegging van deze naam, niet voor één casus — en hoort dus
/// **data** te zijn en geen Rust. RFC-022 §1.3 vraagt per gebeurtenis drie
/// dingen: welke velden ze draagt en van welk type, op welke grondslag ze
/// vastgelegd wordt, en langs welk kanaal ze binnenkomt. Die drie staan hier.
///
/// Wat dat oplevert: een typfout in een fixture valt bij het optuigen en niet
/// pas als de tijdlijn erlangs komt, een tweede soort gebeurtenis kan erbij
/// zonder dat er Rust aan te pas komt, en een lezer van het beeld ziet wat een
/// stroom draagt voordat er één gram in ligt.
///
/// Een stroom die geen schema declareert wordt niet getoetst. Dat is met opzet:
/// een kroniek van een organisatie die er nooit een schreef, is nog steeds een
/// kroniek, en de toets hoort erbij te komen doordat iemand hem opschrijft.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct GebeurtenisSchema {
    /// De naam waaronder deze gebeurtenis in de stroom vastgelegd wordt.
    pub name: String,
    /// Het kanaal waarlangs een gebeurtenis van deze naam binnenkomt.
    ///
    /// Declaratie en geen toets: het gram draagt zijn kanaal zelf, en dat is wat
    /// er in de kroniek staat. Wat het hier toevoegt is dat een lezer van het
    /// schema ziet waarlangs dit soort feit een cel bereikt, ook als de stroom
    /// nog leeg is.
    pub intake: Intake,
    /// De grondslag die een gram van deze naam draagt als het er zelf geen
    /// noemt.
    ///
    /// De grondslag is een eigenschap van het *soort* vastlegging en niet van
    /// één gram: dat een betaling op Awb 4:89 berust, geldt voor elke betaling.
    /// Ze hier één keer opschrijven is dus geen gemak maar de juiste plek; een
    /// gram dat er zelf een draagt, houdt de zijne (zie
    /// [`GebeurtenisSchema::grondslag_voor`]).
    #[serde(default)]
    pub grondslag: String,
    /// De velden die een gram van deze naam draagt, elk met zijn type.
    ///
    /// Een **ondergrens** en geen opsomming: wat hier staat moet erin, en een
    /// gram mag meer dragen. Een besluit schrijft zijn eigen uitkomsten in het
    /// gram en die zijn per regeling anders; zou het schema die ook moeten
    /// noemen, dan stond de wet twee keer opgeschreven.
    #[serde(default)]
    pub fields: Vec<SchemaVeld>,
}

/// Eén veld van een gebeurtenisschema: de naam en het type.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct SchemaVeld {
    /// De veldnaam, zoals ze in het gram staat.
    pub name: String,
    /// Het type van de waarde.
    ///
    /// Dezelfde typen als die van een gedocumenteerde parameter
    /// ([`ParameterType`]) en met opzet dezelfde: een veld dat via het formulier
    /// van een actie in een kroniek belandt, gaat door beide toetsen, en twee
    /// typestelsels zouden daar tegen elkaar in kunnen gaan.
    #[serde(rename = "type")]
    pub value_type: ParameterType,
}

impl GebeurtenisSchema {
    /// De grondslag die een gram van deze naam draagt.
    ///
    /// Het gram gaat voor: een vastlegging die haar eigen grondslag noemt, weet
    /// beter dan het schema waarop juist zíj berustte. Noemt ze er geen, dan is
    /// de grondslag van het schema wat er in de kroniek komt te staan — niet als
    /// weergave, maar als veld van het gram, want een grondslag die alleen bij
    /// het tonen wordt aangevuld staat nergens vast.
    fn grondslag_voor(&self, event: &ChronicleEvent) -> Option<String> {
        if event.grondslag.is_empty() && !self.grondslag.is_empty() {
            Some(self.grondslag.clone())
        } else {
            None
        }
    }
}

/// Eén vastlegging in een kroniekstroom: één executogram.
///
/// De vorm komt uit RFC-022 §1.3, die vraagt dat per vastlegging vier dingen
/// vaststaan: *wat* ([`Self::name`] en [`Self::fields`]), *door wie*
/// ([`Self::recording_actor`]), op *welke grondslag* ([`Self::grondslag`]) en
/// op *welk moment* ([`Self::op_moment`]). [`Self::intake`] vult dat aan met
/// *waarlangs*. Een vastlegging die alleen een veldwaarde en een datum draagt,
/// laat de helft van die vragen onbeantwoord.
///
/// `Serialize` hoort erbij voor één ding: de hash over een stroom zoals ze op
/// een moment lag (zie [`ReducedStream::content_hash`]). Die hash gaat over het
/// hele gram — naam, kanaal, actor, grondslag, moment en velden — want een
/// vastlegging die op één van die punten anders is, is een andere vastlegging.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChronicleEvent {
    /// Wat er gebeurde, in de woorden van de cel: `inkomenslevering`,
    /// `aanvraag_ontvangen`, `relatie_gewijzigd`.
    pub name: String,
    /// Het kanaal waarlangs dit feit de cel bereikte.
    pub intake: Intake,
    /// Wie vastlegde: het id van de cel zelf (RFC-022 §1.3 — de celbeheerder,
    /// niet de `competent_authority`, want een executogram is een feit en geen
    /// besluit). Een cel kan dus niet beweren dat een ander haar kroniek
    /// bijhield.
    pub recording_actor: String,
    /// Op welke grondslag dit feit vastgelegd wordt; vrije tekst, mag leeg.
    #[serde(default)]
    pub grondslag: String,
    /// Het moment waarop dit feit in deze cel feit werd.
    pub op_moment: NaiveDate,
    /// De vastgelegde velden. Veldnamen komen overeen met de `input`-namen van
    /// de regelingen van de cel; de engine matcht hoofdletterongevoelig.
    pub fields: BTreeMap<String, Value>,
}

/// Een tijdgeordende groepering van vastleggingen, gesleuteld op één veld.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChronicleStream {
    /// Naam van de stroom; dient tevens als naam van de databron in de engine.
    pub stream: String,
    /// Het veld waarop vastleggingen gegroepeerd worden (bijvoorbeeld `bsn`).
    pub key: String,
    /// Het schema van de stroom: per gebeurtenisnaam wat ze vastlegt.
    ///
    /// Leeg is toegestaan en betekent *ongetoetst*: dan draagt de stroom wat
    /// erin gelegd wordt. Staat er wél een schema, dan wordt elke vastlegging
    /// eraan gehouden — bij het optuigen als ze uit het wereldbestand komt, bij
    /// het vastleggen als ze tijdens de run ontstaat.
    #[serde(default)]
    pub gebeurtenissen: Vec<GebeurtenisSchema>,
    /// De vastleggingen, in willekeurige volgorde; de store ordent zelf.
    #[serde(default)]
    pub events: Vec<ChronicleEvent>,
}

/// De feiten van één cel.
///
/// Alleen te bevragen via een reductie van de cel die haar bezit — er is geen
/// weg van buiten naar binnen (invariant: geen gedeelde state).
#[derive(Debug, Default)]
pub struct ChronicleStore {
    streams: Vec<ChronicleStream>,
}

/// Eén stroom zoals ze erbij ligt, geleend voor het inspectiebeeld.
///
/// De tegenhanger van [`ReducedStream`]: die kiest per onderwerp de laatste
/// vastlegging voor de engine, deze houdt élk gram zoals het vastgelegd is. Dat
/// is wat een beeld van de wereld nodig heeft — wie kijkt, hoort de hele kroniek
/// te zien en niet alleen wat er op dit moment van telt.
///
/// Geleend en niet in bezit: het inspectiebeeld maakt er zijn eigen doorsnede
/// van (zie [`crate::Snapshot`]) en kan hier niets aan veranderen.
pub(crate) struct ChronicleView<'a> {
    /// Naam van de stroom.
    pub(crate) stream: &'a str,
    /// Het sleutelveld van de stroom.
    pub(crate) key: &'a str,
    /// Het gebeurtenisschema van de stroom; leeg als ze er geen declareert.
    pub(crate) gebeurtenissen: &'a [GebeurtenisSchema],
    /// De vastleggingen, in de volgorde waarin ze vastgelegd zijn.
    pub(crate) events: &'a [ChronicleEvent],
}

/// Eén stroom, teruggebracht tot de feiten die op een moment bekend waren.
pub(crate) struct ReducedStream {
    /// Naam van de stroom.
    pub(crate) stream: String,
    /// Het sleutelveld van de stroom.
    pub(crate) key: String,
    /// Per sleutelwaarde de velden van de laatste vastlegging over dat
    /// onderwerp — één gram in zijn geheel, geen samenraapsel.
    pub(crate) records: Vec<BTreeMap<String, Value>>,
}

/// De stand van één stroom op een moment: hoeveel grammen, en welke.
///
/// Dit is de "inhoud en versie" die RFC-022 §1.3 vraagt van een kroniek die
/// aan een uitvoering bijdraagt: een besluit dat deze hash draagt, is na te
/// rekenen zolang de stroom tot dit moment dezelfde grammen draagt onder
/// dezelfde naam en sleutel — en een kroniek groeit alleen, dus dat blijft zo.
/// Apart van [`ReducedStream`], want een reductie heeft hem niet nodig en hoort
/// er niet voor te betalen; alleen het besluit-pad vraagt erom.
pub(crate) struct StreamStand {
    /// Naam van de stroom.
    pub(crate) stream: String,
    /// Hoeveel grammen er op het moment in de stroom lagen.
    pub(crate) grams: usize,
    /// Een hash over de naam, de sleutel en precies die grammen, in de volgorde
    /// van de tijdas.
    pub(crate) content_hash: String,
}

impl ChronicleStore {
    /// Bouw een store uit de stromen van een celconfiguratie.
    ///
    /// Faalt als een vastlegging het sleutelveld van haar stroom mist: dan valt
    /// niet vast te stellen over welk onderwerp het feit gaat. Faalt ook op twee
    /// stromen met dezelfde naam: die naam is tevens de naam van de databron in
    /// de engine, en daar zou de tweede de eerste stil schaduwen. En om dezelfde
    /// reden op twee gebeurtenissen met dezelfde naam binnen één schema: elke
    /// toets zoekt de eerste die past, dus de tweede declaratie zou niets doen
    /// zonder dat iets dat zegt.
    pub(crate) fn from_streams(cell: &str, mut streams: Vec<ChronicleStream>) -> Result<Self> {
        let mut seen: BTreeSet<String> = BTreeSet::new();
        for stream in &mut streams {
            if !seen.insert(stream.stream.clone()) {
                return Err(SimulatorError::DuplicateStream {
                    cell: cell.to_string(),
                    stream: stream.stream.clone(),
                });
            }
            let mut namen: BTreeSet<&str> = BTreeSet::new();
            for gebeurtenis in &stream.gebeurtenissen {
                if !namen.insert(gebeurtenis.name.as_str()) {
                    return Err(SimulatorError::DuplicateGebeurtenis {
                        cell: cell.to_string(),
                        stream: stream.stream.clone(),
                        name: gebeurtenis.name.clone(),
                    });
                }
            }
            for event in &mut stream.events {
                apply_grondslag(&stream.gebeurtenissen, event);
                check_schema(cell, &stream.stream, &stream.gebeurtenissen, event)?;
                check_event(cell, &stream.stream, &stream.key, event)?;
            }
        }
        Ok(Self { streams })
    }

    /// De veldnamen die deze cel van elke stroom kent.
    ///
    /// Het sleutelveld hoort er altijd bij — de stroom declareert het — en
    /// verder alles wat in een vastlegging voorkomt. Hiermee valt bij het
    /// optuigen te toetsen of een kroniekfilter praat over velden die bestaan,
    /// in plaats van stil "niets vastgesteld" te antwoorden op een typfout.
    pub(crate) fn declared_fields(&self) -> BTreeMap<String, BTreeSet<String>> {
        self.streams
            .iter()
            .map(|stream| {
                let mut fields: BTreeSet<String> = BTreeSet::from([stream.key.clone()]);
                // Wat het schema declareert telt mee vóórdat er één gram ligt —
                // dat is nu juist waarvoor het er staat. Zonder deze regel zou
                // een som over `bedrag` als typfout geweigerd worden zolang er
                // nog niets betaald is, en dat is precies het moment waarop een
                // wereld opgetuigd wordt.
                for gebeurtenis in &stream.gebeurtenissen {
                    fields.extend(gebeurtenis.fields.iter().map(|veld| veld.name.clone()));
                }
                for event in &stream.events {
                    fields.extend(event.fields.keys().cloned());
                }
                (stream.stream.clone(), fields)
            })
            .collect()
    }

    /// Het sleutelveld dat elke stroom declareert, op stroomnaam.
    pub(crate) fn declared_keys(&self) -> BTreeMap<String, String> {
        self.streams
            .iter()
            .map(|stream| (stream.stream.clone(), stream.key.clone()))
            .collect()
    }

    /// Het gebeurtenisschema van één stroom; leeg als ze er geen declareert of
    /// als de cel haar niet houdt.
    pub(crate) fn schema_of(&self, stream: &str) -> &[GebeurtenisSchema] {
        self.streams
            .iter()
            .find(|candidate| candidate.stream == stream)
            .map_or(&[], |found| &found.gebeurtenissen)
    }

    /// Kan het formulier van een actie een gram van deze naam opleveren?
    ///
    /// Dezelfde drie bezwaren als bij een vastlegging — onbekende naam,
    /// ontbrekend veld, verkeerd type — maar één stap eerder: een actie noemt
    /// haar formulier in het wereldbestand, dus wat eruit komt staat bij het
    /// optuigen al vast. De toets hoort dan ook daar te vallen, en niet pas bij
    /// de eerste druk op de knop.
    ///
    /// De tekst gaat in de weigering van de actie zelf (zie
    /// [`SimulatorError::ActionRecording`]), want die weet welke actie het was.
    pub(crate) fn check_form(
        &self,
        stream: &str,
        name: &str,
        form: &[DocumentedParameter],
    ) -> std::result::Result<(), String> {
        let schema = self.schema_of(stream);
        if schema.is_empty() {
            return Ok(());
        }
        let Some(gebeurtenis) = schema.iter().find(|candidate| candidate.name == name) else {
            return Err(format!(
                "het schema van die stroom kent geen gebeurtenis '{name}' (wel: {})",
                schema
                    .iter()
                    .map(|candidate| candidate.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        };
        for veld in &gebeurtenis.fields {
            let Some(field) = form
                .iter()
                .find(|candidate| candidate.name.eq_ignore_ascii_case(&veld.name))
            else {
                return Err(format!(
                    "gebeurtenis '{name}' draagt volgens het schema veld '{}' ({}), en dat \
                     veld staat niet in het formulier",
                    veld.name,
                    veld.value_type.label()
                ));
            };
            if field.value_type != veld.value_type {
                return Err(format!(
                    "veld '{}' is in het formulier {}, en het schema van gebeurtenis \
                     '{name}' declareert {}",
                    veld.name,
                    field.value_type.label(),
                    veld.value_type.label()
                ));
            }
        }
        Ok(())
    }

    /// De namen van de stromen die deze cel houdt, voor een foutmelding.
    pub(crate) fn stream_names(&self) -> Vec<&str> {
        self.streams
            .iter()
            .map(|stream| stream.stream.as_str())
            .collect()
    }

    /// Elke stroom zoals ze erbij ligt, geleend.
    ///
    /// Voor het inspectiebeeld van de wereld, en voor niets anders: een cel geeft
    /// hiermee geen weg naar een andere cel prijs (zie [`ChronicleView`]).
    pub(crate) fn view(&self) -> Vec<ChronicleView<'_>> {
        self.streams
            .iter()
            .map(|stream| ChronicleView {
                stream: &stream.stream,
                key: &stream.key,
                gebeurtenissen: &stream.gebeurtenissen,
                events: &stream.events,
            })
            .collect()
    }

    /// Ligt er in deze stroom op of vóór `op_moment` een gram met deze naam?
    ///
    /// Ja of nee, nooit een waarde: dit is wat een termijn moet weten om te
    /// kunnen zeggen dat een feit ontbrak (zie [`crate::world::Deadline`]).
    pub(crate) fn contains_named(&self, stream: &str, name: &str, op_moment: NaiveDate) -> bool {
        self.streams
            .iter()
            .find(|candidate| candidate.stream == stream)
            .is_some_and(|found| {
                found
                    .events
                    .iter()
                    .any(|event| event.op_moment <= op_moment && event.name == name)
            })
    }

    /// De laatste waarde die één veld in deze stroom kreeg, op of vóór
    /// `op_moment`.
    ///
    /// Over álle sleutelwaarden heen, en dat is het verschil met
    /// [`Self::latest_recording`]: die beantwoordt een vraag over één onderwerp,
    /// deze zoekt de laatste waarde die er hoe dan ook ligt. Dat is precies wat
    /// een voorinvulling nodig heeft — welk onderwerp het wordt, is wat de
    /// invuller nog moet kiezen (zie [`crate::cell::Prefill`]).
    ///
    /// Dezelfde tijdregel als overal: de dag is de korrel, en bij twee
    /// vastleggingen op één dag wint de laatste. `None` is een antwoord en geen
    /// fout: hierover ligt nog niets, en dan blijft het veld leeg.
    pub(crate) fn last_value(
        &self,
        stream: &str,
        name: &str,
        op_moment: NaiveDate,
    ) -> Option<&Value> {
        self.streams
            .iter()
            .find(|candidate| candidate.stream == stream)?
            .events
            .iter()
            .filter(|event| event.op_moment <= op_moment)
            .filter(|event| field(&event.fields, name).is_some())
            .max_by_key(|event| event.op_moment)
            .and_then(|event| field(&event.fields, name))
    }

    /// Het sleutelveld van één stroom; `None` als de cel haar niet houdt.
    pub(crate) fn key_of(&self, stream: &str) -> Option<&str> {
        self.streams
            .iter()
            .find(|candidate| candidate.stream == stream)
            .map(|candidate| candidate.key.as_str())
    }

    /// De laatst vastgelegde gebeurtenis van een stroom; `None` als er geen is.
    ///
    /// Alleen voor tests: wat een reductie op één dag oplevert hangt af van de
    /// volgorde van vastleggen, en dat is van buiten de cel niet te zien.
    #[cfg(test)]
    pub(crate) fn last_recording(&self, stream: &str) -> Option<&ChronicleEvent> {
        self.streams
            .iter()
            .find(|candidate| candidate.stream == stream)?
            .events
            .last()
    }

    /// Het aantal vastleggingen in een stroom; `None` als de cel haar niet houdt.
    ///
    /// Alleen voor tests: dat een besluit precies één gram vastlegt (RFC-022
    /// §1.2 — elk chronolexogram is elementair) is niet van buiten de cel te
    /// zien, en het hoort ook niet van buiten de cel te zien te zijn.
    #[cfg(test)]
    pub(crate) fn len_of(&self, stream: &str) -> Option<usize> {
        self.streams
            .iter()
            .find(|candidate| candidate.stream == stream)
            .map(|candidate| candidate.events.len())
    }

    /// De laatste vastlegging op of vóór `op_moment` met deze sleutelwaarde.
    ///
    /// Dit is de reductie van een cel zonder engine: geen toestandsmerge over
    /// velden heen, maar precies één vastlegging. Wat samen vastgelegd is,
    /// blijft samen — de eigenschap die [`Self::reduce_to`] opgeeft omdat de
    /// engine records als databron wil.
    ///
    /// `conditions` bepaalt wélke vastleggingen meedoen, en pas daarna wint de
    /// laatste. Een voorwaarde op een veld dat over tijd verandert levert dus de
    /// laatste vastlegging die eraan voldeed, niet de huidige stand.
    ///
    /// `None` is een antwoord en geen fout: op dit moment was er niets
    /// vastgesteld over dit onderwerp.
    pub(crate) fn latest_recording(
        &self,
        stream: &str,
        key: &str,
        key_value: &Value,
        conditions: &BTreeMap<String, Value>,
        op_moment: NaiveDate,
    ) -> Option<&ChronicleEvent> {
        // `max_by_key` levert bij gelijke sleutel het laatste element, dus twee
        // vastleggingen op één dag volgen dezelfde regel als in `reduce_to`: de
        // volgorde in de stroom beslist, en de laatste wint. Voor een stroom uit
        // de configuratie is dat de volgorde in het bestand; voor een stroom
        // waarin de cel zelf vastlegt (zie [`Self::record`]) de volgorde waarin
        // dat gebeurde. Twee besluiten op één dag over dezelfde zaak leveren dus
        // het laatstgenomen besluit — de dag is hier de fijnste korrel.
        self.latest_recording_with_place(stream, key, key_value, conditions, op_moment)
            .map(|(_, event)| event)
    }

    /// Dezelfde vastlegging, met haar plek in de stroom erbij.
    ///
    /// De plek is wat een verwijzing naar dit gram nodig heeft (zie
    /// [`crate::GramRef`]): een kroniek groeit achteraan en wijzigt nooit, dus
    /// ze is stabiel en het beeld van de wereld geeft de grammen in precies die
    /// volgorde. Alleen de reductie-uitleg heeft haar nodig; wie de vastlegging
    /// zelf wil, vraagt hierboven.
    pub(crate) fn latest_recording_with_place(
        &self,
        stream: &str,
        key: &str,
        key_value: &Value,
        conditions: &BTreeMap<String, Value>,
        op_moment: NaiveDate,
    ) -> Option<(usize, &ChronicleEvent)> {
        self.recordings_with_place(stream, key, key_value, conditions, op_moment)
            .into_iter()
            .max_by_key(|(_, event)| event.op_moment)
    }

    /// Dezelfde vastleggingen, elk met haar plek in de stroom.
    ///
    /// Het filter zelf staat hier en niet hierboven: welke vastleggingen
    /// meedoen, is één vraag, en twee kopieën ervan zouden een uitleg kunnen
    /// opleveren over andere grammen dan waaruit het antwoord komt.
    pub(crate) fn recordings_with_place(
        &self,
        stream: &str,
        key: &str,
        key_value: &Value,
        conditions: &BTreeMap<String, Value>,
        op_moment: NaiveDate,
    ) -> Vec<(usize, &ChronicleEvent)> {
        self.streams
            .iter()
            .find(|candidate| candidate.stream == stream)
            .into_iter()
            .flat_map(|found| found.events.iter().enumerate())
            .filter(|(_, event)| event.op_moment <= op_moment)
            .filter(|(_, event)| field_equals(&event.fields, key, key_value))
            .filter(|(_, event)| {
                conditions
                    .iter()
                    .all(|(field, expected)| field_equals(&event.fields, field, expected))
            })
            .collect()
    }

    /// Wat er in deze stroom lag en tóch niet meedeed, geteld.
    ///
    /// De tegenhanger van [`Self::recordings_with_place`]: die zegt welke
    /// vastleggingen het filter haalden, deze waaróp de rest afviel. Een cel die
    /// niets vaststelde, kan daarmee zeggen of ze te vroeg gevraagd is, over de
    /// verkeerde zaak, of over een zaak waarover een voorwaarde niet uitkwam —
    /// drie verschillende dingen die anders alle drie "niets" heten.
    ///
    /// Geteld en niet opgesomd: het zijn grammen waar het antwoord juist niet
    /// over gaat.
    pub(crate) fn missed(
        &self,
        stream: &str,
        key: &str,
        key_value: &Value,
        conditions: &BTreeMap<String, Value>,
        op_moment: NaiveDate,
    ) -> Gemist {
        let events: &[ChronicleEvent] = self
            .streams
            .iter()
            .find(|candidate| candidate.stream == stream)
            .map_or(&[], |found| &found.events);

        let mut gemist = Gemist {
            in_de_stroom: events.len(),
            na_het_moment: 0,
            andere_sleutel: 0,
            buiten_de_voorwaarden: 0,
        };
        for event in events {
            if event.op_moment > op_moment {
                gemist.na_het_moment += 1;
            } else if !field_equals(&event.fields, key, key_value) {
                gemist.andere_sleutel += 1;
            } else if !conditions
                .iter()
                .all(|(field, expected)| field_equals(&event.fields, field, expected))
            {
                gemist.buiten_de_voorwaarden += 1;
            }
        }
        gemist
    }

    /// Zou deze vastlegging in deze stroom mogen?
    ///
    /// Zodat een wereld een vastlegging kan afkeuren bij het optuigen in plaats
    /// van halverwege de tijdlijn. Dezelfde toets als bij het vastleggen zelf,
    /// zodat er geen fixture is die het optuigen haalt en later alsnog omvalt.
    /// Geeft niets prijs over wat er al in de stroom staat.
    pub(crate) fn check_recording(
        &self,
        cell: &str,
        stream: &str,
        event: &ChronicleEvent,
    ) -> Result<()> {
        self.checked_index(cell, stream, event).map(|_| ())
    }

    /// De plaats van de stroom waarin deze vastlegging mag, of de fout die haar
    /// tegenhoudt.
    ///
    /// Eén plek voor de toets, zodat het optuigen en het vastleggen niet uit
    /// elkaar kunnen lopen.
    fn checked_index(&self, cell: &str, stream: &str, event: &ChronicleEvent) -> Result<usize> {
        let Some(index) = self.index_of(stream) else {
            return Err(self.unknown_stream(cell, stream));
        };
        let target = &self.streams[index];
        check_schema(cell, &target.stream, &target.gebeurtenissen, event)?;
        check_event(cell, &target.stream, &target.key, event)?;
        Ok(index)
    }

    /// De plaats van een stroom in de store, op naam.
    fn index_of(&self, stream: &str) -> Option<usize> {
        self.streams.iter().position(|s| s.stream == stream)
    }

    /// De fout voor een stroom die deze cel niet houdt, met wat ze wél houdt.
    fn unknown_stream(&self, cell: &str, stream: &str) -> SimulatorError {
        SimulatorError::UnknownChronicleStream {
            cell: cell.to_string(),
            stream: stream.to_string(),
            known: self
                .streams
                .iter()
                .map(|s| s.stream.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        }
    }

    /// Voeg één vastlegging achteraan een stroom toe.
    ///
    /// Toevoegen is het enige dat kan: een bestaande vastlegging wordt nooit
    /// gewijzigd of verwijderd. Een kroniek groeit, en een reductie over een
    /// eerder moment blijft daardoor exact hetzelfde antwoord geven.
    ///
    /// Achteraan, en niet op datumpositie: bij een gelijk moment beslist de
    /// volgorde van vastlegging, en [`Self::reduce_to`] sorteert stabiel.
    pub(crate) fn record(
        &mut self,
        cell: &str,
        stream: &str,
        mut event: ChronicleEvent,
    ) -> Result<()> {
        // Eerst aanvullen, dan toetsen: de grondslag van het schema hoort in het
        // gram te staan voordat [`Self::checked_index`] ernaar kijkt. Die toets
        // blijft de enige, ook hier.
        if let Some(index) = self.index_of(stream) {
            apply_grondslag(&self.streams[index].gebeurtenissen, &mut event);
        }
        let index = self.checked_index(cell, stream, &event)?;
        self.streams[index].events.push(event);
        Ok(())
    }

    /// De feiten zoals ze op `op_moment` in deze cel bekend waren.
    ///
    /// Dit is de tijdreductie: vastleggingen ná `op_moment` bestaan voor deze
    /// vraag niet, en van de rest wint per sleutelwaarde de **laatste
    /// vastlegging, in haar geheel**. Een vraag over een moment in het verleden
    /// levert dus het beeld van toen, niet het beeld van nu.
    ///
    /// In haar geheel, en niet veld voor veld: wat samen vastgelegd is, blijft
    /// samen, en wat apart vastgelegd is wordt niet stil tot één record
    /// samengevoegd dat als vastlegging nooit bestaan heeft. Dat is de
    /// elementariteit van de paper — elk chronolexogram is één vaststelling op
    /// één moment — en RFC-022 zegt het de engine na: de reductie redeneert
    /// over *vaststellingen op momenten*, niet over een toestand van de wereld.
    /// Een veld dat de laatste vastlegging niet draagt, is op dit moment dus
    /// niet vastgesteld, ook als een eerdere het wél droeg. Dezelfde regel als
    /// [`Self::latest_recording`], en met opzet: een bron-cel en een engine horen
    /// over dezelfde kroniek hetzelfde te zien.
    ///
    /// Twee vastleggingen op hetzelfde moment kunnen niet op de tijdas uit
    /// elkaar gehouden worden. De volgorde in de configuratie beslist dan, en de
    /// laatste wint — `sort_by_key` is stabiel, dus dat is een vastgelegde
    /// eigenschap en geen toeval. De dag is in deze versie de fijnste korrel;
    /// wie twee vastleggingen op één dag wil ordenen, heeft een fijnere tijdas
    /// nodig en niet een andere sorteersleutel.
    pub(crate) fn reduce_to(&self, op_moment: NaiveDate) -> Vec<ReducedStream> {
        self.streams
            .iter()
            .map(|stream| {
                let mut events: Vec<&ChronicleEvent> = stream
                    .events
                    .iter()
                    .filter(|event| event.op_moment <= op_moment)
                    .collect();
                events.sort_by_key(|event| event.op_moment);

                let mut per_key: BTreeMap<String, &ChronicleEvent> = BTreeMap::new();
                for event in &events {
                    let Some(key) = record_key(&event.fields, &stream.key) else {
                        continue;
                    };
                    per_key.insert(key, event);
                }

                ReducedStream {
                    stream: stream.stream.clone(),
                    key: stream.key.clone(),
                    records: per_key
                        .into_values()
                        .map(|event| event.fields.clone())
                        .collect(),
                }
            })
            .collect()
    }

    /// De stand van elke stroom op `op_moment`: dezelfde grammen als
    /// [`Self::reduce_to`] meetelt, geteld en gehasht.
    ///
    /// Alleen voor het besluit-pad, dat in zijn gram vastlegt waarop het leunde
    /// (RFC-022 §1.3). Een gram dat niet te serialiseren is, is een fout en geen
    /// stille constante: een hash die er als een hash uitziet maar niets
    /// identificeert, is erger dan geen besluit.
    pub(crate) fn stand(&self, op_moment: NaiveDate) -> Result<Vec<StreamStand>> {
        self.streams
            .iter()
            .map(|stream| {
                let mut events: Vec<&ChronicleEvent> = stream
                    .events
                    .iter()
                    .filter(|event| event.op_moment <= op_moment)
                    .collect();
                events.sort_by_key(|event| event.op_moment);
                Ok(StreamStand {
                    stream: stream.stream.clone(),
                    grams: events.len(),
                    content_hash: content_hash(&stream.stream, &stream.key, &events)?,
                })
            })
            .collect()
    }
}

/// Een hash over een stroom: haar naam, haar sleutel en haar grammen zoals ze
/// op de tijdas liggen.
///
/// Over de serialisatie van de grammen zelf en niet over een samenvatting: twee
/// stromen met dezelfde hash dragen dezelfde vastleggingen in dezelfde
/// volgorde, onder dezelfde naam en gegroepeerd op dezelfde sleutel — want de
/// sleutel bepaalt wat de engine ervan te zien krijgt, dus een andere sleutel
/// is een andere bron. SHA-256, dezelfde keuze als de engine voor de hash van
/// een regeling in het receipt.
///
/// De serialisatie is die van `serde_yaml_ng` over de velden in `BTreeMap`-
/// volgorde: deterministisch bij gelijke afhankelijkheden, geen vastgepind
/// draadformaat. Een nieuwe versie van die crate kan dus elke vastgelegde hash
/// veranderen; dat is een prijs van deze eerste vorm en staat hier zodat niemand
/// er in een vergelijking over struikelt.
fn content_hash(stream: &str, key: &str, events: &[&ChronicleEvent]) -> Result<String> {
    let text =
        serde_yaml_ng::to_string(events).map_err(|source| SimulatorError::ChronicleHashing {
            stream: stream.to_string(),
            source,
        })?;
    let mut hasher = Sha256::new();
    hasher.update(stream.as_bytes());
    hasher.update(b"\n");
    hasher.update(key.as_bytes());
    hasher.update(b"\n");
    hasher.update(text.as_bytes());
    let digest = hasher.finalize();
    Ok(format!("sha256:{digest:x}"))
}

/// De waarde van een veld, hoofdletterongevoelig opgezocht.
///
/// Zelfde souplesse als de engine, die inputnamen ook hoofdletterongevoelig
/// matcht: een vastlegging die `BSN` schrijft, gaat over hetzelfde veld als een
/// die `bsn` schrijft.
pub(crate) fn field<'a>(fields: &'a BTreeMap<String, Value>, name: &str) -> Option<&'a Value> {
    fields
        .iter()
        .find(|(field, _)| field.eq_ignore_ascii_case(name))
        .map(|(_, value)| value)
}

/// Wat een vereist schema vraagt en een gedeclareerd schema niet dekt.
///
/// Voor de toets bij het optuigen dat een cel die een verplichting nakomt daar
/// ook een stroom voor houdt die het bijhoudt (zie
/// [`SimulatorError::ObligationStream`]). Een opsomming en geen ja-of-nee, want
/// wie dit leest moet weten wát hij nog moet opschrijven — "het schema klopt
/// niet" laat hem zoeken in een stroom waarin niets fout lijkt.
///
/// Een gebeurtenis die helemaal ontbreekt, staat er als naam; een veld dat
/// ontbreekt of een ander type draagt, als `gebeurtenis.veld`. Een gedeclareerd
/// schema mag méér dragen dan gevraagd: het vereiste is een ondergrens.
pub(crate) fn uncovered(
    declared: &[GebeurtenisSchema],
    required: &[GebeurtenisSchema],
) -> Vec<String> {
    let mut missing = Vec::new();
    for gevraagd in required {
        let Some(gebeurtenis) = declared
            .iter()
            .find(|candidate| candidate.name == gevraagd.name)
        else {
            missing.push(gevraagd.name.clone());
            continue;
        };
        for veld in &gevraagd.fields {
            let found = gebeurtenis
                .fields
                .iter()
                .find(|candidate| candidate.name.eq_ignore_ascii_case(&veld.name));
            match found {
                Some(declared_veld) if declared_veld.value_type == veld.value_type => {}
                Some(declared_veld) => missing.push(format!(
                    "{}.{} ({}, gedeclareerd als {})",
                    gevraagd.name,
                    veld.name,
                    veld.value_type.label(),
                    declared_veld.value_type.label()
                )),
                None => missing.push(format!(
                    "{}.{} ({})",
                    gevraagd.name,
                    veld.name,
                    veld.value_type.label()
                )),
            }
        }
    }
    missing
}

/// Vul de grondslag aan die het schema van deze gebeurtenis voorschrijft.
///
/// Vóór elke toets, en niet erna: wat getoetst en weggeschreven wordt is het
/// gram zoals het in de kroniek komt te liggen, en niet een halve versie ervan.
/// De toets zelf staat bewust niet hier — die heeft één plek
/// ([`ChronicleStore::checked_index`]), zodat het optuigen en het vastleggen
/// niet uit elkaar kunnen lopen.
fn apply_grondslag(schema: &[GebeurtenisSchema], event: &mut ChronicleEvent) {
    if let Some(grondslag) = schema
        .iter()
        .find(|gebeurtenis| gebeurtenis.name == event.name)
        .and_then(|gebeurtenis| gebeurtenis.grondslag_voor(event))
    {
        event.grondslag = grondslag;
    }
}

/// Houdt deze vastlegging zich aan het schema van haar stroom?
///
/// Drie bezwaren, en ze zeggen alle drie iets anders: de stroom kent deze
/// gebeurtenis niet, het gram mist een veld dat de gebeurtenis declareert, of
/// een veld draagt een waarde van een ander type. Een stroom zonder schema
/// wordt niet getoetst — zie [`GebeurtenisSchema`].
///
/// Wat er **niet** in staat, is een bezwaar tegen een veld dat het schema niet
/// noemt: het schema is een ondergrens. Een besluit legt zijn eigen uitkomsten
/// in het gram, en die volgen uit de regeling die het uitvoerde; zou het schema
/// ze ook moeten opsommen, dan stond de wet twee keer opgeschreven. Een typfout
/// in een veldnaam valt daarmee nog steeds: het gedeclareerde veld ontbreekt
/// dan.
///
/// `null` komt door elke typetoets heen. Dat is geen gat in de toets maar de
/// betekenis van `null` in dit stelsel (RFC-036): een bron die zegt dat er geen
/// partner is, doet een uitspraak over het veld en laat het niet leeg. Een veld
/// dat er helemaal niet staat, wordt wél geweigerd — dat is het verschil.
fn check_schema(
    cell: &str,
    stream: &str,
    schema: &[GebeurtenisSchema],
    event: &ChronicleEvent,
) -> Result<()> {
    if schema.is_empty() {
        return Ok(());
    }
    let Some(gebeurtenis) = schema.iter().find(|candidate| candidate.name == event.name) else {
        return Err(SimulatorError::UnknownGebeurtenis {
            cell: cell.to_string(),
            stream: stream.to_string(),
            name: event.name.clone(),
            known: schema
                .iter()
                .map(|candidate| candidate.name.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        });
    };
    for veld in &gebeurtenis.fields {
        let Some(value) = field(&event.fields, &veld.name) else {
            return Err(SimulatorError::GebeurtenisVeldOntbreekt {
                cell: cell.to_string(),
                stream: stream.to_string(),
                name: event.name.clone(),
                field: veld.name.clone(),
                expected: veld.value_type.label(),
            });
        };
        if matches!(value, Value::Null) {
            continue;
        }
        match veld.value_type.bind(value) {
            Ok(()) => {}
            Err(Mismatch::Type) => {
                return Err(SimulatorError::GebeurtenisVeldType {
                    stream: stream.to_string(),
                    name: event.name.clone(),
                    field: veld.name.clone(),
                    expected: veld.value_type.label(),
                    actual: value.type_name(),
                })
            }
            Err(Mismatch::Date) => {
                return Err(SimulatorError::GebeurtenisVeldDatum {
                    stream: stream.to_string(),
                    name: event.name.clone(),
                    field: veld.name.clone(),
                    value: value.to_string(),
                })
            }
        }
    }
    Ok(())
}

/// Mag deze vastlegging in deze stroom van deze cel?
///
/// Twee dingen moeten kloppen, en bij het vastleggen net zo goed als bij het
/// optuigen: de vastlegging moet het sleutelveld van haar stroom dragen (anders
/// valt niet vast te stellen over welk onderwerp het feit gaat), en ze moet op
/// naam van de cel zelf staan. Dat laatste is de kern van RFC-022 §1.3: een
/// kroniek is het eigen journaal van de celbeheerder, dus een vastlegging op
/// naam van een ander is geen feit maar een aanname over een ander.
fn check_event(cell: &str, stream: &str, key: &str, event: &ChronicleEvent) -> Result<()> {
    if !event.fields.keys().any(|f| f.eq_ignore_ascii_case(key)) {
        return Err(SimulatorError::ChronicleEventWithoutKey {
            stream: stream.to_string(),
            key: key.to_string(),
            op_moment: event.op_moment.to_string(),
        });
    }
    if event.recording_actor != cell {
        return Err(SimulatorError::ForeignRecordingActor {
            cell: cell.to_string(),
            stream: stream.to_string(),
            recording_actor: event.recording_actor.clone(),
            op_moment: event.op_moment.to_string(),
        });
    }
    Ok(())
}

/// Heeft deze vastlegging dit veld, met deze waarde?
///
/// Een veld dat de vastlegging niet heeft, voldoet niet: "onbekend" is geen
/// gelijkheid.
fn field_equals(fields: &BTreeMap<String, Value>, name: &str, expected: &Value) -> bool {
    field(fields, name).is_some_and(|value| equivalent(value, expected))
}

/// De sleutelwaarde van een vastlegging, hoofdletterongevoelig opgezocht.
fn record_key(fields: &BTreeMap<String, Value>, key: &str) -> Option<String> {
    field(fields, key).map(ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Faalt luid op een onleesbare datum: deze tests gáán over de tijdas, dus
    /// een typfout die stil op de standaarddatum uitkomt zou een gebroken test
    /// achter een onschuldige assertie verstoppen.
    fn date(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d")
            .unwrap_or_else(|e| panic!("testdatum '{text}' moet leesbaar zijn: {e}"))
    }

    fn event(op_moment: &str, fields: &[(&str, Value)]) -> ChronicleEvent {
        ChronicleEvent {
            name: "relatie_gewijzigd".to_string(),
            intake: Intake::Levering,
            recording_actor: "toeslagen".to_string(),
            grondslag: String::new(),
            op_moment: date(op_moment),
            fields: fields
                .iter()
                .map(|(name, value)| ((*name).to_string(), value.clone()))
                .collect(),
        }
    }

    fn store(events: Vec<ChronicleEvent>) -> ChronicleStore {
        ChronicleStore::from_streams(
            "toeslagen",
            vec![ChronicleStream {
                stream: "relatie".to_string(),
                key: "bsn".to_string(),
                gebeurtenissen: Vec::new(),
                events,
            }],
        )
        .unwrap_or_else(|e| panic!("teststore moet op te bouwen zijn: {e}"))
    }

    #[test]
    fn latere_vastlegging_overschrijft_eerdere() {
        let store = store(vec![
            event(
                "2024-01-01",
                &[
                    ("bsn", Value::String("1".to_string())),
                    ("partnerschap_type", Value::String("GEEN".to_string())),
                ],
            ),
            event(
                "2024-06-01",
                &[
                    ("bsn", Value::String("1".to_string())),
                    ("partnerschap_type", Value::String("HUWELIJK".to_string())),
                ],
            ),
        ]);

        let reduced = store.reduce_to(date("2025-01-01"));
        assert_eq!(reduced.len(), 1);
        assert_eq!(reduced[0].records.len(), 1);
        assert_eq!(
            reduced[0].records[0].get("partnerschap_type"),
            Some(&Value::String("HUWELIJK".to_string()))
        );
    }

    /// Eén vastlegging gaat er in haar geheel uit, geen samenraapsel.
    ///
    /// De paper: wat samen ontstaat, wordt samen vastgelegd, en een reductie
    /// redeneert over vaststellingen op momenten en niet over een toestand.
    /// Draagt de laatste vastlegging een veld niet, dan is dat veld op dit
    /// moment niet vastgesteld — ook al droeg een eerdere het wél. Zou de
    /// reductie velden over vastleggingen heen samenvoegen, dan leverde ze een
    /// record dat als vastlegging nooit bestaan heeft.
    #[test]
    fn de_reductie_levert_de_laatste_vastlegging_in_haar_geheel() {
        let store = store(vec![
            event(
                "2024-01-01",
                &[
                    ("bsn", Value::String("1".to_string())),
                    ("partnerschap_type", Value::String("HUWELIJK".to_string())),
                    ("partner_bsn", Value::String("2".to_string())),
                ],
            ),
            event(
                "2024-06-01",
                &[
                    ("bsn", Value::String("1".to_string())),
                    ("partnerschap_type", Value::String("GEEN".to_string())),
                ],
            ),
        ]);

        let reduced = store.reduce_to(date("2025-01-01"));
        let record = &reduced[0].records[0];
        assert_eq!(
            record.get("partnerschap_type"),
            Some(&Value::String("GEEN".to_string()))
        );
        assert_eq!(
            record.get("partner_bsn"),
            None,
            "een veld dat de laatste vastlegging niet draagt, komt niet uit een eerdere"
        );
        assert_eq!(record.len(), 2, "precies de velden van die ene vastlegging");
    }

    /// De stand van een stroom is te benoemen: hoeveel grammen, en welke.
    ///
    /// Dezelfde grammen geven dezelfde hash; één gram erbij geeft een andere,
    /// en een moment vóór dat gram geeft de oude terug. Dat is wat een besluit
    /// nodig heeft om te zeggen waarop het leunde (RFC-022 §1.3).
    #[test]
    fn de_stand_van_een_stroom_draagt_een_hash_over_precies_haar_grammen() {
        let eerste = event("2024-01-01", &[("bsn", Value::String("1".to_string()))]);
        let tweede = event("2024-06-01", &[("bsn", Value::String("1".to_string()))]);

        let stand = |events: Vec<ChronicleEvent>, moment: &str| {
            store(events)
                .stand(date(moment))
                .unwrap_or_else(|e| panic!("de stand moet te bepalen zijn: {e}"))
        };
        let een = stand(vec![eerste.clone()], "2025-01-01");
        let twee = stand(vec![eerste.clone(), tweede], "2025-01-01");
        let twee_op_maart = stand(
            vec![
                eerste.clone(),
                event("2024-06-01", &[("bsn", Value::String("1".to_string()))]),
            ],
            "2024-03-01",
        );

        assert_eq!(een[0].grams, 1);
        assert_eq!(twee[0].grams, 2);
        assert!(
            een[0].content_hash.starts_with("sha256:"),
            "de hash noemt haar algoritme: {}",
            een[0].content_hash
        );
        assert_ne!(
            een[0].content_hash, twee[0].content_hash,
            "een gram erbij is een andere stand"
        );
        assert_eq!(
            een[0].content_hash, twee_op_maart[0].content_hash,
            "op een moment vóór het tweede gram is de stand dezelfde als zonder dat gram"
        );
        assert_eq!(
            stand(vec![eerste.clone()], "2025-01-01")[0].content_hash,
            een[0].content_hash,
            "dezelfde grammen, dezelfde hash"
        );

        // Dezelfde grammen onder een andere sleutel zijn een andere bron: de
        // sleutel bepaalt wat de engine ervan te zien krijgt.
        let anders_gesleuteld = ChronicleStore::from_streams(
            "toeslagen",
            vec![ChronicleStream {
                stream: "relatie".to_string(),
                key: "BSN".to_string(),
                gebeurtenissen: Vec::new(),
                events: vec![eerste],
            }],
        )
        .unwrap_or_else(|e| panic!("teststore moet op te bouwen zijn: {e}"))
        .stand(date("2025-01-01"))
        .unwrap_or_else(|e| panic!("de stand moet te bepalen zijn: {e}"));
        assert_ne!(
            anders_gesleuteld[0].content_hash, een[0].content_hash,
            "een andere sleutel is een andere bron"
        );
    }

    #[test]
    fn feiten_van_na_het_moment_tellen_niet_mee() {
        let store = store(vec![
            event(
                "2024-01-01",
                &[
                    ("bsn", Value::String("1".to_string())),
                    ("partnerschap_type", Value::String("GEEN".to_string())),
                ],
            ),
            event(
                "2024-06-01",
                &[
                    ("bsn", Value::String("1".to_string())),
                    ("partnerschap_type", Value::String("HUWELIJK".to_string())),
                ],
            ),
        ]);

        let reduced = store.reduce_to(date("2024-03-01"));
        assert_eq!(
            reduced[0].records[0].get("partnerschap_type"),
            Some(&Value::String("GEEN".to_string()))
        );
    }

    #[test]
    fn vastlegging_zonder_sleutel_wordt_geweigerd() {
        let err = ChronicleStore::from_streams(
            "toeslagen",
            vec![ChronicleStream {
                stream: "relatie".to_string(),
                key: "bsn".to_string(),
                gebeurtenissen: Vec::new(),
                events: vec![event("2024-01-01", &[("partnerschap_type", Value::Null)])],
            }],
        )
        .expect_err("een vastlegging zonder sleutelveld hoort te falen");
        assert!(matches!(
            err,
            SimulatorError::ChronicleEventWithoutKey { .. }
        ));
    }

    #[test]
    fn twee_stromen_met_dezelfde_naam_worden_geweigerd() {
        let stream = |events| ChronicleStream {
            stream: "relatie".to_string(),
            key: "bsn".to_string(),
            gebeurtenissen: Vec::new(),
            events,
        };
        let err = ChronicleStore::from_streams(
            "toeslagen",
            vec![
                stream(vec![event(
                    "2024-01-01",
                    &[("bsn", Value::String("1".to_string()))],
                )]),
                stream(vec![event(
                    "2024-02-01",
                    &[("bsn", Value::String("1".to_string()))],
                )]),
            ],
        )
        .expect_err("twee stromen met dezelfde naam horen te falen");
        assert!(
            matches!(err, SimulatorError::DuplicateStream { .. }),
            "verwachtte DuplicateStream, kreeg {err}"
        );
    }

    #[test]
    fn vastlegging_op_naam_van_een_ander_wordt_geweigerd() {
        let mut foreign = event("2024-01-01", &[("bsn", Value::String("1".to_string()))]);
        foreign.recording_actor = "belastingdienst".to_string();
        let err = ChronicleStore::from_streams(
            "toeslagen",
            vec![ChronicleStream {
                stream: "relatie".to_string(),
                key: "bsn".to_string(),
                gebeurtenissen: Vec::new(),
                events: vec![foreign],
            }],
        )
        .expect_err("een vastlegging op naam van een andere cel hoort te falen");
        assert!(
            matches!(err, SimulatorError::ForeignRecordingActor { .. }),
            "verwachtte ForeignRecordingActor, kreeg {err}"
        );
    }

    #[test]
    fn vastleggen_voegt_toe_en_wijzigt_niets() {
        let mut store = store(vec![event(
            "2024-01-01",
            &[
                ("bsn", Value::String("1".to_string())),
                ("partnerschap_type", Value::String("GEEN".to_string())),
            ],
        )]);

        store
            .record(
                "toeslagen",
                "relatie",
                event(
                    "2024-06-01",
                    &[
                        ("bsn", Value::String("1".to_string())),
                        ("partnerschap_type", Value::String("HUWELIJK".to_string())),
                    ],
                ),
            )
            .unwrap_or_else(|e| panic!("vastleggen in een eigen stroom moet lukken: {e}"));

        let before = store.reduce_to(date("2024-03-01"));
        assert_eq!(
            before[0].records[0].get("partnerschap_type"),
            Some(&Value::String("GEEN".to_string())),
            "een latere vastlegging mag het beeld van een eerder moment niet raken"
        );
        let after = store.reduce_to(date("2024-07-01"));
        assert_eq!(
            after[0].records[0].get("partnerschap_type"),
            Some(&Value::String("HUWELIJK".to_string()))
        );
    }

    #[test]
    fn vastleggen_in_een_onbekende_stroom_wordt_geweigerd() {
        let mut store = store(vec![]);
        let err = store
            .record(
                "toeslagen",
                "betalingen",
                event("2024-01-01", &[("bsn", Value::String("1".to_string()))]),
            )
            .expect_err("een stroom die de cel niet houdt hoort te falen");
        let SimulatorError::UnknownChronicleStream { known, .. } = &err else {
            panic!("verwachtte UnknownChronicleStream, kreeg {err}");
        };
        assert_eq!(known, "relatie");
    }

    #[test]
    fn bij_gelijk_moment_wint_de_laatste_uit_de_configuratie() {
        let store = store(vec![
            event(
                "2024-07-01",
                &[
                    ("bsn", Value::String("1".to_string())),
                    ("partnerschap_type", Value::String("HUWELIJK".to_string())),
                ],
            ),
            event(
                "2024-07-01",
                &[
                    ("bsn", Value::String("1".to_string())),
                    ("partnerschap_type", Value::String("GEEN".to_string())),
                ],
            ),
        ]);

        let reduced = store.reduce_to(date("2025-01-01"));
        assert_eq!(
            reduced[0].records[0].get("partnerschap_type"),
            Some(&Value::String("GEEN".to_string())),
            "de tijdas kan twee vastleggingen op één dag niet ordenen; \
             dan beslist de volgorde in de configuratie"
        );
    }

    #[test]
    fn het_filter_volgt_bij_gelijk_moment_dezelfde_regel_als_de_tijdreductie() {
        // `latest_recording` leunt hiervoor op de belofte van `max_by_key` dat
        // bij gelijke sleutel het laatste element wint. Zonder deze test zou een
        // andere formulering (`max_by`, eerst sorteren, omgekeerd doorlopen) het
        // antwoord van een bron-cel stil omdraaien, terwijl de tijdreductie
        // ernaast wél bewaakt blijft.
        let store = store(vec![
            event(
                "2024-07-01",
                &[
                    ("bsn", Value::String("1".to_string())),
                    ("partnerschap_type", Value::String("HUWELIJK".to_string())),
                ],
            ),
            event(
                "2024-07-01",
                &[
                    ("bsn", Value::String("1".to_string())),
                    ("partnerschap_type", Value::String("GEEN".to_string())),
                ],
            ),
        ]);

        let found = store
            .latest_recording(
                "relatie",
                "bsn",
                &Value::String("1".to_string()),
                &BTreeMap::new(),
                date("2025-01-01"),
            )
            .unwrap_or_else(|| panic!("er staan twee vastleggingen, dus er is er een de laatste"));
        assert_eq!(
            found.fields.get("partnerschap_type"),
            Some(&Value::String("GEEN".to_string())),
            "het kroniekfilter moet dezelfde vastlegging kiezen als de tijdreductie"
        );
    }

    #[test]
    fn de_laatste_waarde_volgt_dezelfde_tijdregel_als_het_filter() {
        // `last_value` kijkt over álle sleutelwaarden heen — welk onderwerp het
        // wordt, is wat de invuller van een formulier nog moet kiezen — maar
        // verder gelden dezelfde twee regels als overal: wat na het gevraagde
        // moment ligt telt niet mee, en bij twee vastleggingen op één dag wint
        // de laatste. Die laatste leunt op de belofte van `max_by_key` dat bij
        // gelijke sleutel het laatste element wint; een andere formulering zou
        // hier stil een ander voorstel opleveren dan de tijdreductie ernaast.
        let store = store(vec![
            event("2024-01-01", &[("bsn", Value::String("1".to_string()))]),
            event("2024-07-01", &[("bsn", Value::String("2".to_string()))]),
            event("2024-07-01", &[("bsn", Value::String("3".to_string()))]),
            event("2024-09-01", &[("bsn", Value::String("4".to_string()))]),
        ]);

        assert_eq!(
            store.last_value("relatie", "bsn", date("2024-07-01")),
            Some(&Value::String("3".to_string())),
            "over alle sleutels heen, en op één dag wint de laatste uit de configuratie"
        );
        assert_eq!(
            store.last_value("relatie", "bsn", date("2024-06-01")),
            Some(&Value::String("1".to_string())),
            "wat na het gevraagde moment ligt, bestaat voor deze vraag niet"
        );
        assert_eq!(
            store.last_value("relatie", "bsn", date("2023-12-31")),
            None,
            "hierover ligt nog niets; dat is een antwoord en geen fout"
        );
        assert_eq!(
            store.last_value("relatie", "partnerschap_type", date("2025-01-01")),
            None,
            "een veld dat nergens in de stroom ligt, levert niets op"
        );
        assert_eq!(
            store.last_value("onbekend", "bsn", date("2025-01-01")),
            None,
            "een stroom die de cel niet houdt evenmin"
        );
    }

    #[test]
    fn een_voorwaarde_op_een_veld_dat_de_vastlegging_niet_heeft_voldoet_niet() {
        // "Onbekend" is geen gelijkheid: een vastlegging die het veld niet draagt
        // doet niet mee, ook niet als ze op de tijdas de laatste zou zijn.
        let store = store(vec![
            event(
                "2023-03-01",
                &[
                    ("bsn", Value::String("1".to_string())),
                    ("partnerschap_type", Value::String("HUWELIJK".to_string())),
                ],
            ),
            event("2024-07-01", &[("bsn", Value::String("1".to_string()))]),
        ]);

        let found = store
            .latest_recording(
                "relatie",
                "bsn",
                &Value::String("1".to_string()),
                &BTreeMap::from([(
                    "partnerschap_type".to_string(),
                    Value::String("HUWELIJK".to_string()),
                )]),
                date("2025-01-01"),
            )
            .unwrap_or_else(|| panic!("de vastlegging van 2023-03-01 voldoet aan het filter"));
        assert_eq!(
            found.op_moment,
            date("2023-03-01"),
            "de latere vastlegging kent het veld niet en doet dus niet mee"
        );
    }
    /// Het schema van de teststroom: één gebeurtenis met twee velden.
    fn schema() -> Vec<GebeurtenisSchema> {
        vec![GebeurtenisSchema {
            name: "relatie_gewijzigd".to_string(),
            intake: Intake::Levering,
            grondslag: "eigen registratie".to_string(),
            fields: vec![
                SchemaVeld {
                    name: "bsn".to_string(),
                    value_type: ParameterType::String,
                },
                SchemaVeld {
                    name: "partnerschap_type".to_string(),
                    value_type: ParameterType::String,
                },
            ],
        }]
    }

    /// Dezelfde stroom, nu mét schema.
    fn store_met_schema(events: Vec<ChronicleEvent>) -> Result<ChronicleStore> {
        ChronicleStore::from_streams(
            "toeslagen",
            vec![ChronicleStream {
                stream: "relatie".to_string(),
                key: "bsn".to_string(),
                gebeurtenissen: schema(),
                events,
            }],
        )
    }

    /// Een gram met een eigen naam en eigen velden, op naam van de teststore.
    fn named(name: &str, fields: &[(&str, Value)]) -> ChronicleEvent {
        ChronicleEvent {
            name: name.to_string(),
            ..event("2023-03-01", fields)
        }
    }

    #[test]
    fn een_stroom_zonder_schema_toetst_niets() {
        // De toets komt erbij doordat iemand een schema opschrijft, en niet
        // doordat het platform er een verzint: een kroniek van een organisatie
        // die er nooit een schreef, is nog steeds een kroniek.
        let store = store(vec![named(
            "iets_heel_anders",
            &[("bsn", Value::String("1".to_string()))],
        )]);
        assert_eq!(store.len_of("relatie"), Some(1));
    }

    #[test]
    fn een_gebeurtenis_buiten_het_schema_wordt_geweigerd() {
        let error = store_met_schema(vec![named(
            "relatie_verwijderd",
            &[("bsn", Value::String("1".to_string()))],
        )])
        .expect_err("een naam die het schema niet kent hoort geweigerd te worden");
        assert!(
            matches!(error, SimulatorError::UnknownGebeurtenis { .. }),
            "verwachtte UnknownGebeurtenis, kreeg {error}"
        );
    }

    #[test]
    fn een_ontbrekend_veld_uit_het_schema_wordt_geweigerd() {
        // En dit is tevens wat een typfout in een veldnaam tegenhoudt: het
        // gedeclareerde veld ontbreekt dan, ook al staat er een veld te veel.
        let error = store_met_schema(vec![named(
            "relatie_gewijzigd",
            &[
                ("bsn", Value::String("1".to_string())),
                ("partnerschaps_type", Value::String("GEEN".to_string())),
            ],
        )])
        .expect_err("een gram dat een gedeclareerd veld mist hoort geweigerd te worden");
        match &error {
            SimulatorError::GebeurtenisVeldOntbreekt { field, .. } => {
                assert_eq!(field, "partnerschap_type");
            }
            other => panic!("verwachtte GebeurtenisVeldOntbreekt, kreeg {other}"),
        }
    }

    #[test]
    fn een_veld_van_het_verkeerde_type_wordt_geweigerd() {
        let error = store_met_schema(vec![named(
            "relatie_gewijzigd",
            &[
                ("bsn", Value::String("1".to_string())),
                ("partnerschap_type", Value::Int(3)),
            ],
        )])
        .expect_err("een waarde van een ander type hoort geweigerd te worden");
        assert!(
            matches!(error, SimulatorError::GebeurtenisVeldType { .. }),
            "verwachtte GebeurtenisVeldType, kreeg {error}"
        );
    }

    #[test]
    fn null_komt_door_elke_typetoets_heen() {
        // `null` is een uitspraak over het veld en geen ontbrekend feit
        // (RFC-036): de bron zegt dat er geen partner is. Een veld dat er
        // helemáál niet staat, wordt wél geweigerd — dat is het verschil.
        let store = store_met_schema(vec![named(
            "relatie_gewijzigd",
            &[
                ("bsn", Value::String("1".to_string())),
                ("partnerschap_type", Value::Null),
            ],
        )])
        .unwrap_or_else(|e| panic!("null hoort door de typetoets te komen: {e}"));
        assert_eq!(store.len_of("relatie"), Some(1));
    }

    #[test]
    fn een_gram_zonder_grondslag_krijgt_die_van_het_schema() {
        let store = store_met_schema(vec![named(
            "relatie_gewijzigd",
            &[
                ("bsn", Value::String("1".to_string())),
                ("partnerschap_type", Value::String("GEEN".to_string())),
            ],
        )])
        .unwrap_or_else(|e| panic!("teststore moet op te bouwen zijn: {e}"));
        let gram = store
            .last_recording("relatie")
            .unwrap_or_else(|| panic!("er hoort een gram te liggen"));
        assert_eq!(
            gram.grondslag, "eigen registratie",
            "de grondslag van het schema hoort in het gram te staan"
        );
    }

    #[test]
    fn een_eigen_grondslag_gaat_voor_die_van_het_schema() {
        // Een vastlegging die haar eigen grondslag noemt, weet beter dan het
        // schema waarop juist zíj berustte.
        let mut gram = named(
            "relatie_gewijzigd",
            &[
                ("bsn", Value::String("1".to_string())),
                ("partnerschap_type", Value::String("GEEN".to_string())),
            ],
        );
        gram.grondslag = "rechterlijke uitspraak".to_string();
        let store = store_met_schema(vec![gram])
            .unwrap_or_else(|e| panic!("teststore moet op te bouwen zijn: {e}"));
        assert_eq!(
            store
                .last_recording("relatie")
                .map(|found| found.grondslag.as_str()),
            Some("rechterlijke uitspraak")
        );
    }

    #[test]
    fn het_schema_telt_mee_als_gedeclareerd_veld_van_een_lege_stroom() {
        // Waarvoor het schema er ook is: een reductie over een stroom die nog
        // leeg is, hoort niet als typfout geweigerd te worden.
        let store = store_met_schema(Vec::new())
            .unwrap_or_else(|e| panic!("teststore moet op te bouwen zijn: {e}"));
        let declared = store.declared_fields();
        let fields = declared
            .get("relatie")
            .unwrap_or_else(|| panic!("de stroom hoort erin te staan"));
        assert!(
            fields.contains("partnerschap_type"),
            "een gedeclareerd veld hoort mee te tellen voordat er een gram ligt, kreeg {fields:?}"
        );
    }

    #[test]
    fn een_formulier_wordt_aan_hetzelfde_schema_gehouden() {
        let store = store_met_schema(Vec::new())
            .unwrap_or_else(|e| panic!("teststore moet op te bouwen zijn: {e}"));
        let veld = |name: &str, value_type| DocumentedParameter {
            name: name.to_string(),
            value_type,
            prefill: None,
        };
        assert!(store
            .check_form(
                "relatie",
                "relatie_gewijzigd",
                &[
                    veld("bsn", ParameterType::String),
                    veld("partnerschap_type", ParameterType::String),
                ],
            )
            .is_ok());
        assert!(
            store
                .check_form(
                    "relatie",
                    "relatie_gewijzigd",
                    &[veld("bsn", ParameterType::String)],
                )
                .is_err(),
            "een formulier zonder gedeclareerd veld hoort geweigerd te worden"
        );
        assert!(
            store
                .check_form(
                    "relatie",
                    "relatie_gewijzigd",
                    &[
                        veld("bsn", ParameterType::String),
                        veld("partnerschap_type", ParameterType::Number),
                    ],
                )
                .is_err(),
            "een formulierveld van een ander type dan het schema hoort geweigerd te worden"
        );
        assert!(
            store
                .check_form(
                    "relatie",
                    "relatie_verwijderd",
                    &[veld("bsn", ParameterType::String)]
                )
                .is_err(),
            "een naam die het schema niet kent hoort geweigerd te worden"
        );
    }

    #[test]
    fn uncovered_noemt_wat_er_ontbreekt() {
        let vereist = vec![GebeurtenisSchema {
            name: "relatie_gewijzigd".to_string(),
            intake: Intake::Levering,
            grondslag: String::new(),
            fields: vec![
                SchemaVeld {
                    name: "bsn".to_string(),
                    value_type: ParameterType::String,
                },
                SchemaVeld {
                    name: "bedrag".to_string(),
                    value_type: ParameterType::Amount,
                },
            ],
        }];
        assert!(
            uncovered(&schema(), &vereist)
                .iter()
                .any(|melding| melding.contains("bedrag")),
            "wie dit leest moet weten wát hij nog moet opschrijven"
        );
        assert!(
            uncovered(&[], &vereist).contains(&"relatie_gewijzigd".to_string()),
            "een stroom zonder schema dekt niets, en dan is de naam zelf het bezwaar"
        );
        assert!(
            uncovered(&schema(), &[]).is_empty(),
            "een gedeclareerd schema mag méér dragen dan gevraagd"
        );
    }
}
