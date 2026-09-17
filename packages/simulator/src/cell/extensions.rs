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

use crate::cell::besluit::{ObligationDefinition, ObligationOrigin, Vervanging};
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

/// De sleutel waaronder een artikel declareert dat zijn beschikking in de plaats
/// komt van wat er over dezelfde zaak nog openstond.
///
/// Ook dit staat in het **lexogram**: of een vaststelling het voorschot vervangt,
/// is recht (Awir art. 19 jo. art. 24, tweede lid) en geen keuze van de
/// uitvoerder. Zonder deze sleutel blijft staan wat er staat — een verplichting
/// is niet in te trekken.
pub const VERVANGT_OPENSTAANDE_TERMIJNEN: &str = "vervangt_openstaande_termijnen";

/// De sleutel waaronder een artikel declareert welke uitkomsten van zijn eigen
/// regeling pas bij een latere stage van de procedure vaststaan.
///
/// Per stage een lijst uitkomstnamen: `{ BEKENDMAKING: [besluit_tijdig] }`. Ook
/// dit staat in het **lexogram**: dat een uitkomst van de bekendmaking afhangt
/// (bijvoorbeeld "tijdig beslist" gemeten op de dag van bekendmaking), is werking
/// van die regeling, en een hook kan het niet zeggen — een hook vuurt op elke
/// beschikking van hetzelfde soort, en dit hoort alleen bij het besluit dat dit
/// artikel voortbrengt.
pub const STAGE_UITKOMSTEN: &str = "stage_uitkomsten";

/// De sleutels die de namespace kent, op alfabet.
///
/// Voor in de melding: wie een blok schrijft dat geweigerd wordt, hoort te lezen
/// wat er dan wél mag staan. Eén lijst naast [`ChronolexBlock`], zodat een
/// sleutel erbij ook in de melding terechtkomt.
pub const KNOWN_KEYS: [&str; 4] = [
    AFWIJZING_WANNEER,
    STAGE_UITKOMSTEN,
    VERPLICHTINGEN,
    VERVANGT_OPENSTAANDE_TERMIJNEN,
];

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
    /// Wanneer het besluit op dit artikel een afwijzing is.
    ///
    /// Ongelezen bewaard: het optuigen toetst elke geladen versie en het besluit
    /// leest de versie die op dat moment gold, en die twee moeten er hetzelfde
    /// in zien. Wat erin staat wordt uitgepakt door [`afwijzing_wanneer`], op
    /// één plek, met een reden die een lezer verder helpt.
    ///
    /// `None` betekent: de sleutel staat er niet, en dan declareert het artikel
    /// geen afwijzing. Een sleutel die er wél staat maar leeg blijft, is iets
    /// anders — zie [`present`].
    #[serde(default, deserialize_with = "present")]
    pub(crate) afwijzing_wanneer: Option<Value>,
    /// Dat een beschikking op dit artikel in de plaats komt van wat er over
    /// dezelfde zaak nog openstond; weggelaten mag.
    ///
    /// Getypeerd en niet ongelezen bewaard, anders dan
    /// [`Self::afwijzing_wanneer`]: er staat één sleutel in met één betekenis,
    /// dus er valt niets uit te pakken waar een eigen reden bij hoort.
    ///
    /// `None` betekent ook hier: de sleutel staat er niet. Een sleutel die er
    /// wél staat maar leeg blijft, is iets anders — zie [`vervanging`].
    #[serde(default, deserialize_with = "vervanging")]
    pub(crate) vervangt_openstaande_termijnen: Option<Vervanging>,
    /// Per stage de uitkomsten van deze regeling die bij die stage in het gram
    /// komen; leeg mag.
    ///
    /// Alleen stages ná het besluit hebben betekenis — wat bij het besluit
    /// vaststaat, is een gewone uitkomst van het besluit — en van die stages
    /// voert het platform er nu één uit ([`crate::cell::besluit::STAGE_BEKENDMAKING`]).
    /// Een andere stage wordt bij het optuigen geweigerd, zie
    /// [`Self::check_stage_uitkomsten`].
    #[serde(default)]
    pub(crate) stage_uitkomsten: BTreeMap<String, Vec<String>>,
}

/// Lees een sleutel die er staat, ook als er niets achter staat.
///
/// `Option<Value>` zou `afwijzing_wanneer:` zonder waarde als `None` lezen — als
/// "de sleutel staat er niet" — en dan zou een artikel dat zegt af te wijzen
/// zonder te zeggen wanneer, stil nooit afwijzen. Dat is precies het stille
/// overslaan dat deze module opheft: een lege sleutel is een blok dat niet af
/// is, en die hoort te vallen waar hij uitgepakt wordt
/// ([`afwijzing_wanneer`]), met de reden erbij. Deze lezer wordt alleen
/// aangeroepen als de sleutel er staat, dus een blok zonder de sleutel blijft
/// `None` via `serde(default)`.
fn present<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<Value>, D::Error> {
    Value::deserialize(deserializer).map(Some)
}

/// Lees een vervanging die er staat, en weiger er een die leeg blijft.
///
/// Om dezelfde reden als bij [`present`]: `Option<Vervanging>` zou
/// `vervangt_openstaande_termijnen:` zonder waarde als `None` lezen — als "de
/// sleutel staat er niet" — en dan zou een artikel dat zegt het voorschot te
/// vervangen, stil niets vervangen. Dat is het stille overslaan dat deze module
/// opheft, en het gaat hier over termijnen die wél of niet uitbetaald worden.
///
/// Anders dan bij `afwijzing_wanneer` valt het hier meteen: er is één sleutel met
/// één betekenis, dus er is geen later moment waarop het blok uitgepakt wordt en
/// een eigen reden zou kunnen krijgen. De reden staat daarom in deze melding.
fn vervanging<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<Vervanging>, D::Error> {
    let raw = serde_yaml_ng::Value::deserialize(deserializer)?;
    if raw.is_null() {
        return Err(serde::de::Error::custom(format!(
            "`{VERVANGT_OPENSTAANDE_TERMIJNEN}` staat er zonder te zeggen waarop \
             het vervallen berust; een termijn die zonder grondslag vervalt, is \
             een belofte die zonder wet verdwijnt"
        )));
    }
    serde_yaml_ng::from_value(raw)
        .map(Some)
        .map_err(serde::de::Error::custom)
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

    /// De uitkomsten die dit blok voor één stage declareert.
    pub(crate) fn stage_uitkomsten_voor(&self, stage: &str) -> &[String] {
        self.stage_uitkomsten.get(stage).map_or(&[], Vec::as_slice)
    }

    /// Toets `stage_uitkomsten` tegen de stages die het platform uitvoert en de
    /// uitkomsten die deze versie van de regeling kent.
    ///
    /// Bij het optuigen, om dezelfde reden als de gesloten sleutellijst: een
    /// stage met een typfout of een uitkomst die de regeling niet kent, levert
    /// anders een bekendmaking op die stil niets extra's draagt — en dan zegt de
    /// wet iets wat het gram nooit laat zien.
    pub(crate) fn check_stage_uitkomsten(
        &self,
        origin: &ObligationOrigin,
        regulation_outputs: &std::collections::BTreeSet<String>,
    ) -> Result<()> {
        use crate::cell::besluit::STAGE_BEKENDMAKING;
        for (stage, outputs) in &self.stage_uitkomsten {
            if stage != STAGE_BEKENDMAKING {
                return Err(malformed(
                    origin,
                    format!(
                        "`{STAGE_UITKOMSTEN}` noemt stage '{stage}'; het platform voert na het \
                         besluit alleen de stage {STAGE_BEKENDMAKING} uit, en een uitkomst van \
                         een stage die nooit draait, komt nooit in een gram"
                    ),
                ));
            }
            for output in outputs {
                if !regulation_outputs.contains(output) {
                    return Err(malformed(
                        origin,
                        format!(
                            "`{STAGE_UITKOMSTEN}.{stage}` noemt '{output}', en die uitkomst kent \
                             deze versie van de regeling niet"
                        ),
                    ));
                }
            }
        }
        Ok(())
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
    use std::collections::BTreeSet;

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

    fn stage_blok(stage: &str, output: &str) -> Result<ChronolexBlock> {
        lees(&format!(
            "      extensions:\n        chronolex:\n          stage_uitkomsten:\n            \
             {stage}:\n              - {output}\n"
        ))
    }

    /// `stage_uitkomsten` leest per stage, en de toets laat een bekende uitkomst
    /// op de stage BEKENDMAKING door.
    #[test]
    fn een_stage_uitkomst_leest_en_klopt() {
        let block = stage_blok("BEKENDMAKING", "tijdig")
            .unwrap_or_else(|e| panic!("een stage-uitkomst hoort te lezen: {e}"));
        assert_eq!(block.stage_uitkomsten_voor("BEKENDMAKING"), ["tijdig"]);
        block
            .check_stage_uitkomsten(&origin(), &BTreeSet::from(["tijdig".to_string()]))
            .unwrap_or_else(|e| panic!("een bekende uitkomst hoort door te komen: {e}"));
    }

    /// Een stage die het platform niet uitvoert, levert nooit een gram op: dat
    /// valt bij het optuigen, en niet stil.
    #[test]
    fn een_stage_uitkomst_op_een_andere_stage_wordt_geweigerd() {
        let block = stage_blok("BEZWAAR", "tijdig")
            .unwrap_or_else(|e| panic!("het blok zelf is leesbaar: {e}"));
        let err = block
            .check_stage_uitkomsten(&origin(), &BTreeSet::from(["tijdig".to_string()]))
            .expect_err("een stage na de bekendmaking hoort geweigerd te worden");
        assert!(err.to_string().contains("BEZWAAR"), "kreeg: {err}");
    }

    /// Een uitkomst die de regeling niet kent, is een typfout.
    #[test]
    fn een_onbekende_stage_uitkomst_wordt_geweigerd() {
        let block = stage_blok("BEKENDMAKING", "tijdg")
            .unwrap_or_else(|e| panic!("het blok zelf is leesbaar: {e}"));
        let err = block
            .check_stage_uitkomsten(&origin(), &BTreeSet::from(["tijdig".to_string()]))
            .expect_err("een onbekende uitkomst hoort geweigerd te worden");
        assert!(err.to_string().contains("'tijdg'"), "kreeg: {err}");
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

    /// Een vervanging zonder grondslag is geen vervanging die er niet staat.
    ///
    /// `vervangt_openstaande_termijnen:` zonder waarde is een artikel dat zegt
    /// het voorschot te vervangen zonder te zeggen waarop dat berust. Zou het
    /// als "geen sleutel" gelezen worden, dan liepen de openstaande termijnen
    /// stil door naast het slotbedrag.
    #[test]
    fn een_lege_vervanging_wordt_geweigerd() {
        let err = lees(
            "      extensions:\n        chronolex:\n          \
             vervangt_openstaande_termijnen:\n",
        )
        .expect_err("een vervanging zonder grondslag hoort te falen");
        let melding = err.to_string();
        for deel in [
            "test_regeling",
            "artikel 2",
            VERVANGT_OPENSTAANDE_TERMIJNEN,
            "grondslag",
        ] {
            assert!(
                melding.contains(deel),
                "de melding hoort '{deel}' te noemen, kreeg: {melding}"
            );
        }
    }

    /// En een vervanging mét grondslag komt gewoon door, uit dezelfde lezing.
    #[test]
    fn een_vervanging_met_grondslag_leest() {
        let block = lees(
            "      extensions:\n        chronolex:\n          \
             vervangt_openstaande_termijnen:\n            grondslag: art. 19\n",
        )
        .unwrap_or_else(|e| panic!("een vervanging met grondslag hoort te lezen: {e}"));
        assert_eq!(
            block
                .vervangt_openstaande_termijnen
                .as_ref()
                .map(|vervanging| vervanging.grondslag.as_str()),
            Some("art. 19")
        );
    }

    /// Een sleutel die er staat maar leeg blijft, is geen sleutel die er niet staat.
    ///
    /// `afwijzing_wanneer:` zonder waarde is een artikel dat zegt af te wijzen
    /// zonder te zeggen wanneer. Zou het als "geen sleutel" gelezen worden, dan
    /// wees het stil nooit iemand af — en dat is precies het stille overslaan
    /// dat deze module opheft. Het valt waar het blok uitgepakt wordt, met de
    /// reden erbij.
    #[test]
    fn een_lege_afwijzing_wanneer_is_geen_ontbrekende() {
        let block = lees("      extensions:\n        chronolex:\n          afwijzing_wanneer:\n")
            .unwrap_or_else(|e| panic!("het blok zelf is te lezen: {e}"));
        let value = block
            .afwijzing_wanneer
            .as_ref()
            .unwrap_or_else(|| panic!("een sleutel die er staat, hoort er te staan"));
        let reason = afwijzing_wanneer(value).expect_err("een lege voorwaarde hoort te falen");
        assert!(
            reason.contains("geen null"),
            "de reden hoort te zeggen wat er staat: {reason}"
        );
    }

    /// De sleutels in de melding zijn die van de struct, en blijven dat.
    ///
    /// [`KNOWN_KEYS`] staat naast [`ChronolexBlock`] en niet erin: een veld erbij
    /// in de struct zonder een regel erbij in de lijst zou een melding opleveren
    /// die liegt over wat er dan wél mag staan. Serde noemt zelf wat ze verwacht;
    /// hier worden die twee tegen elkaar gehouden.
    #[test]
    fn de_sleutels_in_de_melding_zijn_die_van_de_struct() {
        let err = lees("      extensions:\n        chronolex:\n          onbekend: 1\n")
            .expect_err("een onbekende sleutel hoort te falen");
        let melding = err.to_string();
        let (_, verwacht) = melding
            .split_once("expected ")
            .unwrap_or_else(|| panic!("serde hoort te noemen wat ze verwacht: {melding}"));
        let uit_de_struct: BTreeSet<&str> = verwacht.split('`').skip(1).step_by(2).collect();
        assert_eq!(
            uit_de_struct,
            KNOWN_KEYS.iter().copied().collect::<BTreeSet<&str>>(),
            "de lijst naast de struct hoort die van de struct te zijn: {melding}"
        );
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
