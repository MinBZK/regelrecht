//! Het schema van het decretogram: welk deel van een besluit volgt uit de wet?
//!
//! Een decretogram krijgt zijn velden uit drie bronnen — het lexogram, het
//! wereldbestand en het platform — en het schema in het beeld van de wereld zegt
//! per veld welke van de drie het is. Dat is een **meting** en geen weergave:
//! wat hier `wereldbestand` heet, is normatieve inhoud die buiten het recht staat
//! (RFC-022), en de vervolgstappen horen die velden naar `lexogram` te bewegen.
//!
//! Twee dingen staan hier vast:
//!
//! 1. **De hele lijst** van één besluit uit de publieke wereld. Niet een steekproef
//!    op een enkel veld: wie er een veld bij zet of er een weghaalt, verschuift de
//!    meting, en dan hoort deze test dat te zeggen.
//! 2. **Beleid is geen wet.** Een uitkomst die uit een uitvoeringsregel volgt,
//!    draagt `beleid` en niet `lexogram`. Wie die twee bij elkaar optelt, meet dat
//!    er meer uit het recht komt dan er uit het recht komt.

use regelrecht_simulator::{
    regulation_root, DecretogramField, FieldOrigin, GramKind, Herkomst, Scenario, World,
    WorldDefinition,
};
use std::path::{Path, PathBuf};

fn world_file() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("worlds")
        .join("publieke_wereld.yaml")
}

/// De publieke wereld zoals ze opgetuigd wordt, zonder dat er iets gebeurd is.
///
/// Met opzet geen besluit ervoor: het schema hoort te bestaan vóórdat er één
/// decretogram ligt. Kon het pas uit een genomen besluit afgeleid worden, dan zou
/// het een samenvatting van het verleden zijn en niet de vorm die vastligt.
fn publieke_wereld() -> World {
    let path = world_file();
    let definition =
        WorldDefinition::load(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    World::from_definition(&definition, &regulation_root())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Het schema van één besluit van één cel.
fn schema(world: &World, cell: &str, besluit: &str) -> Vec<DecretogramField> {
    world
        .snapshot()
        .cells
        .iter()
        .find(|snapshot| snapshot.id == cell)
        .unwrap_or_else(|| panic!("cel '{cell}' hoort in het beeld te staan"))
        .besluiten
        .iter()
        .find(|definition| definition.name == besluit)
        .unwrap_or_else(|| panic!("besluit '{besluit}' hoort in het beeld te staan"))
        .schema
        .clone()
}

/// Eén veld uit een schema.
fn veld<'a>(schema: &'a [DecretogramField], name: &str) -> &'a DecretogramField {
    schema
        .iter()
        .find(|field| field.name == name)
        .unwrap_or_else(|| panic!("veld '{name}' hoort in het schema te staan"))
}

/// **De hele lijst**: elk veld van `zorgtoeslag_vaststelling`, met zijn type en
/// zijn herkomst.
///
/// De volgorde hoort erbij: eerst wat het besluit vaststelt, dan de omslag die
/// elk decretogram draagt. Een lezer die de tabel van boven naar beneden leest,
/// leest daarmee eerst waar het besluit over gaat.
#[test]
fn het_schema_van_de_vaststelling_ligt_vast() {
    let schema = schema(&publieke_wereld(), "toeslagen", "zorgtoeslag_vaststelling");
    let gemeten: Vec<(&str, Option<&str>, Option<&str>, Herkomst, bool)> = schema
        .iter()
        .map(|field| {
            (
                field.name.as_str(),
                field.value_type.as_deref(),
                field.unit.as_deref(),
                field.herkomst,
                field.gat,
            )
        })
        .collect();

    assert_eq!(
        gemeten,
        [
            // Wat het besluit vaststelt: de uitkomst die het besluit *is*,
            // vooraan, en daarna wat `outputs` erbij noemt.
            (
                "heeft_recht_op_zorgtoeslag",
                Some("boolean"),
                None,
                Herkomst::Lexogram,
                false
            ),
            (
                "hoogte_zorgtoeslag",
                Some("amount"),
                Some("eurocent"),
                Herkomst::Lexogram,
                false
            ),
            // De omslag. Drie velden erin komen niet uit een regeling: het
            // kenmerk waaronder de zaak loopt, de naam van de besluit-definitie
            // (en daarmee welke uitkomsten samen één gram vormen) en de
            // verplichtingen. Dat zijn de gaten.
            ("op_moment", Some("date"), None, Herkomst::Platform, false),
            (
                "zaakkenmerk",
                Some("string"),
                None,
                Herkomst::Wereldbestand,
                true
            ),
            (
                "besluit",
                Some("string"),
                None,
                Herkomst::Wereldbestand,
                true
            ),
            (
                "regulation",
                Some("string"),
                None,
                Herkomst::Platform,
                false
            ),
            (
                "regulation_valid_from",
                Some("date"),
                None,
                Herkomst::Platform,
                false
            ),
            (
                "executed_regulations",
                Some("array"),
                None,
                Herkomst::Platform,
                false
            ),
            (
                "competent_authority",
                Some("string"),
                None,
                Herkomst::Platform,
                false
            ),
            (
                "besloten_door",
                Some("string"),
                None,
                Herkomst::Platform,
                false
            ),
            (
                "legal_character",
                Some("string"),
                None,
                Herkomst::Platform,
                false
            ),
            // Wélk besluit dit is, en waarop het zou afketsen: dat zegt de wet
            // en niet het platform. Het rechtskarakter hierboven is altijd
            // `BESCHIKKING` — daarop weigert het besluit-pad zelf — maar
            // toekenning of afwijzing staat in `produces` van het artikel.
            (
                "decision_type",
                Some("string"),
                None,
                Herkomst::Lexogram,
                false
            ),
            (
                "afwijzingsgrond",
                Some("array"),
                None,
                Herkomst::Lexogram,
                false
            ),
            ("inputs", Some("object"), None, Herkomst::Platform, false),
            (
                "obligations",
                Some("array"),
                None,
                Herkomst::Wereldbestand,
                true
            ),
            (
                "chronicle_sources",
                Some("array"),
                None,
                Herkomst::Platform,
                false
            ),
            ("receipt", Some("object"), None, Herkomst::Platform, false),
        ],
        "het schema van dit besluit is de meting 'welk deel volgt uit de wet'; \
         wie hem verschuift, hoort dat hier te zien"
    );
}

/// Elke uitkomst noemt het **artikel** dat haar voortbrengt, met de versie.
#[test]
fn elke_uitkomst_noemt_haar_artikel() {
    let schema = schema(&publieke_wereld(), "toeslagen", "zorgtoeslag_vaststelling");
    for name in ["heeft_recht_op_zorgtoeslag", "hoogte_zorgtoeslag"] {
        let lexogram = veld(&schema, name)
            .lexogram
            .as_ref()
            .unwrap_or_else(|| panic!("uitkomst '{name}' hoort haar lexogram te noemen"));
        assert_eq!(lexogram.regulation, "wet_op_de_zorgtoeslag");
        assert_eq!(lexogram.article.as_deref(), Some("2"));
        assert!(
            lexogram.valid_from.is_some(),
            "zonder versie is een verwijzing naar een artikel geen verwijzing: \
             de wet verandert"
        );
        assert_eq!(lexogram.regulatory_layer, "WET");
    }
}

/// `competent_authority` en `legal_character` zijn platformvelden — elk
/// decretogram draagt ze — en de **wet** levert de waarde. Allebei staat erbij.
///
/// Het gezag volgt de RFC-002-volgorde die het besluit-pad zelf toepast: het
/// artikel dat de aansturende uitkomst voortbrengt, anders het document. De
/// zorgtoeslagwet declareert het op het document, en dan staat er `null` bij het
/// artikel — "de wet zegt het ergens" is iets anders dan "de wet zegt het hier".
#[test]
fn het_platform_noemt_waar_het_de_wet_leest() {
    let schema = schema(&publieke_wereld(), "toeslagen", "zorgtoeslag_vaststelling");

    let gezag = veld(&schema, "competent_authority");
    assert_eq!(gezag.herkomst, Herkomst::Platform);
    let lexogram = gezag
        .lexogram
        .as_ref()
        .expect("de regeling wijst een bevoegd gezag aan (RFC-002)");
    assert_eq!(lexogram.regulation, "wet_op_de_zorgtoeslag");
    assert_eq!(
        lexogram.article, None,
        "deze regeling declareert het gezag op het document"
    );

    let karakter = veld(&schema, "legal_character");
    assert_eq!(
        karakter
            .lexogram
            .as_ref()
            .and_then(|lexogram| lexogram.article.as_deref()),
        Some("2"),
        "het rechtskarakter komt uit `produces` van het artikel dat de \
         aansturende uitkomst voortbrengt"
    );
}

/// Het besluittype en de afwijzingsgrond zijn velden van de **wet**.
///
/// Het rechtskarakter is een platformveld: elk decretogram draagt `BESCHIKKING`,
/// en het besluit-pad weigert elke andere waarde. Wélk besluit het is, ligt
/// andersom — `produces.decision_type` zegt wat het is als het doorgaat, en
/// `produces.extensions.chronolex.afwijzing_wanneer` wanneer het een afwijzing
/// wordt. Het platform kiest daar niets in, dus beide velden horen naar het
/// artikel te wijzen dat ze declareert.
#[test]
fn het_besluittype_en_de_afwijzingsgrond_komen_uit_de_wet() {
    let schema = schema(&publieke_wereld(), "toeslagen", "zorgtoeslag_vaststelling");

    for name in ["decision_type", "afwijzingsgrond"] {
        let field = veld(&schema, name);
        assert_eq!(field.herkomst, Herkomst::Lexogram, "veld '{name}'");
        assert!(
            !field.gat,
            "veld '{name}' is geen gat: de wet declareert het"
        );
        let lexogram = field
            .lexogram
            .as_ref()
            .unwrap_or_else(|| panic!("veld '{name}' hoort zijn artikel te noemen"));
        assert_eq!(lexogram.regulation, "wet_op_de_zorgtoeslag");
        assert_eq!(
            lexogram.article.as_deref(),
            Some("2"),
            "veld '{name}' hoort te wijzen naar het artikel dat de aansturende \
             uitkomst voortbrengt"
        );
    }

    // De toelichting noemt wat er in het gram komt te staan: het type voor de
    // gewone afloop, en de voorwaarde waarop het een afwijzing wordt.
    let type_toelichting = veld(&schema, "decision_type")
        .toelichting
        .as_deref()
        .expect("het besluittype hoort te zeggen wat het kan worden");
    assert!(
        type_toelichting.contains("TOEKENNING") && type_toelichting.contains("AFWIJZING"),
        "de toelichting hoort beide afloopmogelijkheden te noemen: {type_toelichting}"
    );

    let grond_toelichting = veld(&schema, "afwijzingsgrond")
        .toelichting
        .as_deref()
        .expect("de afwijzingsgrond hoort haar voorwaarden te noemen");
    assert!(
        grond_toelichting.contains("heeft_recht_op_zorgtoeslag = false"),
        "de toelichting hoort de voorwaarde uit de wet te noemen: {grond_toelichting}"
    );
}

/// Een regeling die niet afwijst, laat het veld leeg — en dat is geen gat.
///
/// Dat een wet geen weigering kent, is geen norm die ze had moeten stellen. Het
/// veld staat er wel: elk decretogram draagt het, leeg.
#[test]
fn een_regeling_zonder_afwijzing_laat_de_grond_aan_het_platform() {
    let schema = schema(&beleidswereld(), "uitvoerder", "tegemoetkoming");

    let grond = veld(&schema, "afwijzingsgrond");
    assert_eq!(grond.herkomst, Herkomst::Platform);
    assert!(!grond.gat, "geen afwijzingsgrond is geen ontbrekende norm");
    assert_eq!(grond.lexogram, None, "er is geen artikel om naar te wijzen");
}

/// Een besluit dat verplichtingen oplegt, draagt ze elk als **gat**, en het
/// bedrag waarnaar ze wijzen draagt zijn eigen lexogram.
///
/// Dat verschil is de hele reden dat de verplichting apart in het schema staat:
/// *hoeveel* er betaald moet worden zegt de wet, *dat* er betaald moet worden en
/// in welk ritme zegt het wereldbestand.
#[test]
fn een_verplichting_is_een_gat_en_haar_bedrag_niet() {
    let schema = schema(&publieke_wereld(), "toeslagen", "zorgtoeslag_toekenning");

    let verplichting = veld(&schema, "obligations[0]");
    assert_eq!(verplichting.herkomst, Herkomst::Wereldbestand);
    assert_eq!(
        verplichting.value_type.as_deref(),
        Some("object"),
        "één verplichting is een bedrag, een betaler en een ritme; de reeks \
         termijnen die eruit volgt staat in het veld 'obligations'"
    );
    assert!(
        verplichting.gat,
        "geen enkel artikel dekt deze verplichting"
    );
    let toelichting = verplichting
        .toelichting
        .as_deref()
        .expect("een gat hoort te zeggen wat het wereldbestand dan wél zegt");
    assert!(
        toelichting.contains("hoogte_zorgtoeslag"),
        "de verplichting hoort naar de uitkomst te wijzen die haar bedrag levert: {toelichting}"
    );

    let bedrag = veld(&schema, "hoogte_zorgtoeslag");
    assert_eq!(bedrag.herkomst, Herkomst::Lexogram);
    assert!(!bedrag.gat);
}

/// De wereld van de tweede meting: één cel die een **uitvoeringsregel**
/// uitvoert.
///
/// In het bestand en niet in `worlds/`: die wereld is er om bespeeld te worden,
/// deze om een eigenschap van de opstelling te meten. `test_uitvoeringsregel` is
/// een testregeling naast het corpus (zie `fixtures/regulation/`) die
/// `regulatory_layer: UITVOERINGSBELEID` draagt en verder alles heeft wat een
/// regeling heeft.
fn beleidswereld() -> World {
    let yaml = r"
clock:
  start: 2024-01-01

cells:
  - id: uitvoerder
    identity: Uitvoerder
    laws:
      - test_uitvoeringsregel
    chronicles:
      - stream: aanmerkingen
        key: bsn
        events:
          - name: aanmerking_vastgesteld
            intake: levering
            recording_actor: uitvoerder
            grondslag: eigen vaststelling
            op_moment: 2024-01-01
            fields:
              bsn: '999993653'
              komt_in_aanmerking: true

    besluit_definitions:
      - name: tegemoetkoming
        doc: de tegemoetkoming die de uitvoeringsregel toekent
        regulation: test_uitvoeringsregel
        output: tegemoetkoming_toegekend
        outputs:
          - hoogte_tegemoetkoming
        zaakkenmerk: tegemoetkoming/{bsn}
        params:
          - name: bsn
            type: string
        inputs:
          bsn:
            param: bsn
          komt_in_aanmerking:
            from_chronicle: aanmerkingen
            field: komt_in_aanmerking
";
    let definition = WorldDefinition::from_yaml(yaml)
        .unwrap_or_else(|e| panic!("het wereldbestand van deze test hoort te lezen: {e}"));
    World::from_definition(&definition, &regulation_root())
        .unwrap_or_else(|e| panic!("de wereld van deze test hoort op te tuigen: {e}"))
}

/// **Beleid is geen wet.** Een uitkomst uit een uitvoeringsregel draagt
/// `beleid`, met dezelfde verwijzing als een uitkomst uit een wet.
#[test]
fn een_uitkomst_uit_een_uitvoeringsregel_heet_beleid() {
    let schema = schema(&beleidswereld(), "uitvoerder", "tegemoetkoming");

    for name in ["tegemoetkoming_toegekend", "hoogte_tegemoetkoming"] {
        let field = veld(&schema, name);
        assert_eq!(
            field.herkomst,
            Herkomst::Beleid,
            "'{name}' komt uit een uitvoeringsregel en niet uit de wet"
        );
        assert!(
            !field.gat,
            "beleid is geen gat: er staat een regeling achter, alleen niet de wet"
        );
        let lexogram = field
            .lexogram
            .as_ref()
            .unwrap_or_else(|| panic!("'{name}' hoort haar regeling te noemen"));
        assert_eq!(lexogram.regulation, "test_uitvoeringsregel");
        assert_eq!(lexogram.article.as_deref(), Some("1"));
        assert_eq!(
            lexogram.regulatory_layer, "UITVOERINGSBELEID",
            "dat dit beleid is, hoort uit de regeling te blijken en niet uit het label"
        );
    }

    assert_eq!(
        veld(&schema, "hoogte_tegemoetkoming").unit.as_deref(),
        Some("eurocent"),
        "een bedrag zonder eenheid is een getal"
    );
}

/// De belofte tegen een gram dat er werkelijk ligt: elk veld van een genomen
/// besluit staat in het schema van dat besluit.
///
/// De toets hieronder houdt het schema tegen een lijst in deze test, en een lijst
/// in een test groeit niet vanzelf mee. Deze speelt het volledige verhaal van de
/// publieke wereld af en houdt het schema tegen de decretogrammen die daaruit
/// komen: krijgt een gram er ooit een veld bij zonder dat het schema het noemt,
/// dan valt deze test en niet pas de lezer van het beeld.
///
/// Eén kant op, en twee dingen overgeslagen, want het **beeld** toont een gram
/// niet veld voor veld zoals het gram het draagt (zie `snapshot.rs`): het laat
/// `receipt` en `inputs` weg, en het zet elke input los, elk met haar eigen
/// herkomst. Het schema beschrijft het gram zelf — waarin de inputs in één veld
/// bij elkaar zitten — dus de losse inputs horen er niet in en worden hier
/// overgeslagen op hun herkomst.
#[test]
fn elk_veld_van_een_genomen_besluit_staat_in_zijn_schema() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("scenarios")
        .join("toeslagen_volledig_verhaal.yaml");
    let scenario = Scenario::load(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let run = scenario
        .run(&regulation_root())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert!(run.passed(), "{}:\n{}", path.display(), run.report());

    let mut gewogen = 0;
    for cell in &run.snapshot.cells {
        for chronicle in &cell.chronicles {
            for gram in &chronicle.grams {
                if gram.kind != GramKind::Decretogram {
                    continue;
                }
                // Een cel zonder engine kan zelf een vaststelling in haar kroniek
                // leggen; dat gram is even goed een decretogram, maar het komt
                // niet uit een besluit-definitie en heeft dus geen schema.
                let Some(besluit) = gram
                    .fields
                    .get("besluit")
                    .and_then(|field| field.value.as_str())
                else {
                    continue;
                };
                let schema = &cell
                    .besluiten
                    .iter()
                    .find(|definition| definition.name == besluit)
                    .unwrap_or_else(|| panic!("besluit '{besluit}' hoort in het beeld te staan"))
                    .schema;
                let namen: Vec<&str> = schema.iter().map(|field| field.name.as_str()).collect();
                for (veld, waarde) in &gram.fields {
                    if matches!(waarde.origin, FieldOrigin::BesluitInput { .. }) {
                        continue;
                    }
                    assert!(
                        namen.contains(&veld.as_str()),
                        "het gram van '{besluit}' draagt '{veld}', en het schema \
                         belooft dat niet: {namen:?}"
                    );
                }
                gewogen += 1;
            }
        }
    }
    assert!(
        gewogen > 0,
        "zonder een genomen besluit weegt deze toets niets"
    );
}

/// Elk veld dat het gram draagt, staat in het schema — en geen veld meer.
///
/// De toets die de twee aan elkaar bindt: het schema is een belofte over de vorm
/// van het gram, en een belofte die een veld vergeet of er een verzint, is geen
/// belofte. Zie `tests/snapshot.rs` voor de vorm van het gram zelf.
#[test]
fn het_schema_dekt_elk_veld_van_het_gram() {
    let world = publieke_wereld();
    for cell in world.snapshot().cells {
        for besluit in cell.besluiten {
            let namen: Vec<&str> = besluit
                .schema
                .iter()
                .map(|field| field.name.as_str())
                .collect();
            for verplicht in [
                "zaakkenmerk",
                "besluit",
                "regulation",
                "regulation_valid_from",
                "executed_regulations",
                "competent_authority",
                "besloten_door",
                "legal_character",
                "inputs",
                "obligations",
                "chronicle_sources",
                "receipt",
                "op_moment",
            ] {
                assert!(
                    namen.contains(&verplicht),
                    "elk decretogram draagt '{verplicht}', dus het schema van \
                     '{}' hoort het te noemen: {namen:?}",
                    besluit.name
                );
            }
            assert_eq!(
                namen.len(),
                namen
                    .iter()
                    .collect::<std::collections::BTreeSet<_>>()
                    .len(),
                "een veldnaam twee keer in het schema van '{}' zou twee herkomsten \
                 voor één waarde beloven",
                besluit.name
            );
        }
    }
}
