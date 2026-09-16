//! De namespace `chronolex` onder `produces.extensions`: één lezer, één keer.
//!
//! `produces.extensions` is in het law-model met opzet ondoorzichtig: het
//! document draagt het blok ongewijzigd mee en legt het niet uit (RFC-022 §3.2).
//! Wie een namespace leest, bezit hem — en dit is de onze. Alle andere
//! namespaces (`blauwe_knop` en wat er nog komt) blijven hier ongelezen en
//! ongemoeid; wat deze module streng maakt, geldt alleen voor [`CHRONOLEX`].
//!
//! **Eén lezer, en daarom deze module.** Wat het blok bevat, wordt op drie
//! plekken gebruikt: bij het optuigen van een cel, bij het uitrekenen van het
//! schema van een decretogram, en bij het besluit zelf. Zouden die drie het blok
//! elk op hun eigen manier uit de `BTreeMap` halen, dan kan er één zijn die iets
//! ziet wat de andere niet ziet — en dan glipt een regeling die denkt te
//! weigeren of te verplichten langs de plek die dat had moeten weigeren. Daarom
//! is er precies één parse-punt ([`ChronolexBlock::read`]) en precies één
//! struct, met [`serde(deny_unknown_fields)`](https://serde.rs/container-attrs.html#deny_unknown_fields)
//! erop.
//!
//! **De sleutellijst is gesloten.** Een namespace die geen mapping is, of een
//! sleutel die er niet in staat (`verplichting` in plaats van `verplichtingen`),
//! weigert bij het optuigen van de cel. Stil overslaan zou een wet laten zwijgen
//! waar ze spreekt: een artikel dat niets oplegt en een artikel met een typfout
//! zien er dan hetzelfde uit, en dat verschil is precies wat een lezer van het
//! gram nooit meer terugvindt.

use crate::cell::besluit::{ObligationDefinition, ObligationOrigin};
use crate::error::{Result, SimulatorError};
use regelrecht_engine::article::Produces;
use regelrecht_engine::Value;
use serde::Deserialize;
use std::collections::BTreeMap;

/// De namespace waarin de chronolexografie haar aanvullingen op `produces` legt.
pub const CHRONOLEX: &str = "chronolex";

/// De sleutel waaronder een artikel declareert wat zijn besluit oplegt.
pub const VERPLICHTINGEN: &str = "verplichtingen";

/// De sleutel waaronder een artikel declareert wanneer zijn besluit afwijst.
///
/// In het **lexogram** en niet in het wereldbestand. Wanneer een besluit een
/// afwijzing is, is werking van de wet: het hangt aan de uitkomst die het
/// artikel voortbrengt, en het geldt voor elke cel die dat artikel uitvoert.
/// Zou het in een besluit-definitie staan, dan konden twee uitvoerders dezelfde
/// wet verschillend laten weigeren zonder dat er aan de wet iets te zien was —
/// zie [`SimulatorError::AfwijzingWanneerInWereldbestand`].
pub const AFWIJZING_WANNEER: &str = "afwijzing_wanneer";

/// De sleutels die de namespace kent, op alfabet.
///
/// Voor in de melding: wie een blok schrijft dat geweigerd wordt, hoort te lezen
/// wat er dan wél mag staan. Eén lijst naast [`ChronolexBlock`], zodat een
/// sleutel erbij ook in de melding terechtkomt.
pub const KNOWN_KEYS: [&str; 2] = [AFWIJZING_WANNEER, VERPLICHTINGEN];

/// Het blok dat een uitvoerend artikel onder [`CHRONOLEX`] kan dragen.
///
/// Eigen struct en geen losse lookups, zodat `deny_unknown_fields` geldt en élke
/// lezing hetzelfde ziet. Een sleutel erbij is hier één regel — en daarmee
/// meteen bekend bij alle drie de lezers.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChronolexBlock {
    /// Wat dit artikel aan verplichtingen oplegt; leeg mag.
    #[serde(default)]
    pub(crate) verplichtingen: Vec<ObligationDefinition>,
    /// Wanneer het besluit op dit artikel een afwijzing is; leeg mag.
    ///
    /// Ongelezen bewaard: het optuigen toetst elke geladen versie en het besluit
    /// leest de versie die op dat moment gold, en die twee moeten er hetzelfde
    /// in zien. Wat erin staat wordt uitgepakt door [`afwijzing_wanneer`], op
    /// één plek, met een reden die een lezer verder helpt.
    #[serde(default)]
    pub(crate) afwijzing_wanneer: Option<Value>,
}

impl ChronolexBlock {
    /// Lees de namespace van dit `produces`.
    ///
    /// Een artikel zonder blok declareert niets, en dat is geen fout: niet elke
    /// beschikking legt iets op of wijst iets af. Een blok dat er wél staat maar
    /// niet klopt, is er wel een — het staat in de **wet**, dus wie het schrijft
    /// is niet dezelfde als wie het leest.
    pub(crate) fn read(produces: Option<&Produces>, origin: &ObligationOrigin) -> Result<Self> {
        let Some(block) = namespace(produces) else {
            return Ok(Self::default());
        };
        // Een namespace die geen mapping is, krijgt zijn eigen reden: de serde-
        // melding zou hier over een Rust-struct praten, en dat is niet wat er in
        // het wetsbestand staat.
        if !matches!(block, Value::Object(_)) {
            return Err(malformed(
                origin,
                format!(
                    "`{CHRONOLEX}` is een blok met sleutels en geen {}",
                    block.type_name()
                ),
            ));
        }
        // Via serde en niet met de hand uit elkaar gehaald: het blok is gewone
        // YAML, en `deny_unknown_fields` op de weg erheen is wat een typfout
        // tegenhoudt in plaats van hem stil te laten verdwijnen.
        serde_yaml_ng::to_value(block)
            .and_then(serde_yaml_ng::from_value)
            .map_err(|source| malformed(origin, source.to_string()))
    }

    /// Draagt dit `produces` de namespace, wat er ook in staat?
    ///
    /// Voor het optuigen: een artikel dat het blok draagt moet gelezen worden,
    /// ook als het leeg blijkt. Wat er niet is, hoeft niet nagelopen te worden.
    pub(crate) fn declared_in(produces: Option<&Produces>) -> bool {
        namespace(produces).is_some()
    }
}

/// De `chronolex`-waarde van dit `produces`, ongelezen.
fn namespace(produces: Option<&Produces>) -> Option<&Value> {
    produces?.extensions.as_ref()?.get(CHRONOLEX)
}

/// De weigering, met de bekende sleutels erbij.
///
/// [`ObligationOrigin::describe`] noemt regeling, artikel en versie: een lezer
/// die deze melding krijgt, moet het blok kunnen vinden zonder te weten welke
/// cel hem opriep.
fn malformed(origin: &ObligationOrigin, reason: String) -> SimulatorError {
    SimulatorError::MalformedChronolexBlock {
        origin: origin.describe(),
        reason,
        known: KNOWN_KEYS.join(", "),
    }
}

/// De afwijzingsvoorwaarden uit het blok: per uitkomst de waarde die afwijst.
///
/// Streng, en met een reden die een lezer verder helpt. Het blok staat in een
/// **wet**, dus wie het schrijft is niet dezelfde als wie het leest; een blok met
/// een getal erin dat stil als "geen voorwaarde" zou eindigen, zet de weigering
/// uit zonder dat er iets te zien is.
pub(crate) fn afwijzing_wanneer(
    block: &Value,
) -> std::result::Result<BTreeMap<String, bool>, String> {
    let Value::Object(entries) = block else {
        return Err(format!(
            "`{AFWIJZING_WANNEER}` is een toewijzing van uitkomst naar `true` of \
             `false`, en geen {}",
            block.type_name()
        ));
    };
    let mut conditions = BTreeMap::new();
    for (name, value) in entries {
        let Some(value) = value.as_bool() else {
            return Err(format!(
                "`{AFWIJZING_WANNEER}` geeft uitkomst '{name}' de waarde {value} ({}); \
                 een afwijzingsvoorwaarde vergelijkt met `true` of `false`",
                value.type_name()
            ));
        };
        conditions.insert(name.clone(), value);
    }
    Ok(conditions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use regelrecht_engine::Article;

    fn origin() -> ObligationOrigin {
        ObligationOrigin {
            regulation: "test_regeling".to_string(),
            valid_from: Some("2024-01-01".to_string()),
            article: "2".to_string(),
        }
    }

    fn artikel(block: &str) -> Article {
        serde_yaml_ng::from_str(&format!(
            "number: '2'\nurl: https://example.com/wet#Artikel2\ntext: tekst\n\
             machine_readable:\n  execution:\n    produces:\n      \
             legal_character: BESCHIKKING\n{block}"
        ))
        .unwrap_or_else(|e| panic!("testartikel moet parsen: {e}"))
    }

    fn produces(article: &Article) -> Option<&Produces> {
        article.get_execution_spec()?.produces.as_ref()
    }

    fn lees(block: &str) -> Result<ChronolexBlock> {
        let article = artikel(block);
        ChronolexBlock::read(produces(&article), &origin())
    }

    /// Een artikel zonder namespace declareert niets, en dat is geen fout.
    #[test]
    fn een_artikel_zonder_blok_declareert_niets() {
        let block = lees("").unwrap_or_else(|e| panic!("een artikel zonder blok mag: {e}"));
        assert!(block.verplichtingen.is_empty());
        assert!(block.afwijzing_wanneer.is_none());
    }

    /// Beide sleutels komen uit dezelfde lezing, getypeerd.
    #[test]
    fn beide_sleutels_komen_uit_een_lezing() {
        let block = lees(
            "      extensions:\n        chronolex:\n          \
             afwijzing_wanneer:\n            heeft_recht: false\n          \
             verplichtingen:\n            - soort: betaling\n              \
             bedrag: $bedrag\n              ritme: ineens\n              \
             grondslag: art. 2\n",
        )
        .unwrap_or_else(|e| panic!("een blok met beide sleutels hoort te lezen: {e}"));
        assert_eq!(block.verplichtingen.len(), 1);
        let conditions = block
            .afwijzing_wanneer
            .as_ref()
            .map(afwijzing_wanneer)
            .transpose()
            .unwrap_or_else(|e| panic!("de voorwaarde hoort te lezen: {e}"))
            .unwrap_or_default();
        assert_eq!(
            conditions,
            BTreeMap::from([("heeft_recht".to_string(), false)])
        );
    }

    /// Een typfout in een sleutel is geen artikel dat niets oplegt.
    #[test]
    fn een_onbekende_sleutel_wordt_geweigerd() {
        let err = lees("      extensions:\n        chronolex:\n          verplichtignen: []\n")
            .expect_err("een typfout in een sleutel hoort te falen");
        let melding = err.to_string();
        for deel in ["test_regeling", "artikel 2", "2024-01-01", "verplichtingen"] {
            assert!(
                melding.contains(deel),
                "de melding hoort '{deel}' te noemen, kreeg: {melding}"
            );
        }
    }

    /// Een namespace die geen mapping is, krijgt zijn eigen reden.
    #[test]
    fn een_namespace_die_geen_mapping_is_wordt_geweigerd() {
        let err = lees("      extensions:\n        chronolex: []\n")
            .expect_err("een namespace die geen mapping is hoort te falen");
        let melding = err.to_string();
        assert!(
            melding.contains("geen array"),
            "de melding hoort te zeggen wat er dan wél staat, kreeg: {melding}"
        );
        assert!(
            melding.contains(AFWIJZING_WANNEER) && melding.contains(VERPLICHTINGEN),
            "de melding hoort de bekende sleutels te noemen, kreeg: {melding}"
        );
    }

    /// Een andere namespace onder `extensions` blijft ongelezen en ongemoeid.
    #[test]
    fn een_andere_namespace_blijft_ongemoeid() {
        let block = lees("      extensions:\n        blauwe_knop:\n          wat_dan_ook: 3\n")
            .unwrap_or_else(|e| panic!("een vreemde namespace hoort te mogen: {e}"));
        assert!(block.verplichtingen.is_empty());
    }

    #[test]
    fn een_blok_leest_als_uitkomst_naar_ja_of_nee() {
        let block: Value = serde_yaml_ng::from_str("heeft_recht: false\nis_verzekerde: true")
            .unwrap_or_else(|e| panic!("testblok moet parsen: {e}"));
        assert_eq!(
            afwijzing_wanneer(&block)
                .unwrap_or_else(|e| panic!("een gewoon blok moet te lezen zijn: {e}")),
            BTreeMap::from([
                ("heeft_recht".to_string(), false),
                ("is_verzekerde".to_string(), true),
            ]),
            "meer dan één voorwaarde is een of, en ze staan er alle twee"
        );
    }

    #[test]
    fn een_blok_met_iets_anders_dan_ja_of_nee_is_niet_te_lezen() {
        let block: Value = serde_yaml_ng::from_str("heeft_recht: 0")
            .unwrap_or_else(|e| panic!("testblok moet parsen: {e}"));
        let reason = afwijzing_wanneer(&block).expect_err("een getal is geen ja-of-nee");
        assert!(
            reason.contains("heeft_recht") && reason.contains("de waarde 0"),
            "de reden hoort te noemen wat er staat: {reason}"
        );
    }
}
