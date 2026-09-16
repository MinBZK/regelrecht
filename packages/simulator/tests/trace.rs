//! De uitvoeringstrace in het receipt van een decretogram.
//!
//! Een decretogram *is* het RFC-013 Execution Receipt van het besluit (RFC-022
//! §1.2), en een receipt is er om de uitvoering te kunnen reproduceren. Zonder
//! trace staat er wel wát eruit kwam, maar niet langs welke artikelen — en dan
//! is een besluit na te rekenen maar niet na te lopen.
//!
//! Vier dingen staan hier vast, en de derde is de scherpe:
//!
//! 1. De trace zit in het receipt, op de plek die RFC-013 ervoor heeft
//!    (`results.trace`).
//! 2. Ze noemt de **artikelen** die geraakt zijn, per node met de regeling
//!    erbij. Dat een uitkomst uit artikel 2 komt en niet uit artikel 3 is wat
//!    een jurist erin komt zoeken.
//! 3. Twee uitvoeringen met dezelfde invoer leveren **dezelfde** trace, tot op
//!    de hash. Dat is wat reproduceerbaarheid betekent, en het is de reden dat
//!    de trace geen looptijden draagt: die verschillen per run.
//! 4. Een betaling wijst terug naar het decretogram waaruit ze volgt — cel,
//!    kroniek, plek en termijnnummer — zodat een lezer van het executogram bij
//!    het besluit en zijn trace uitkomt.
//!
//! De wereld is het echte wereldbestand en niet een fixture: wat hier langs komt,
//! is wat een deployment te zien krijgt.

use chrono::NaiveDate;
use regelrecht_simulator::{regulation_root, Value, World, WorldDefinition, BESCHIKKINGEN};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// De BSN van de persoon in het verhaal van de publieke wereld.
const BSN: &str = "999993653";

/// Het moment waarop de toekenning genomen wordt.
const BESLOTEN_OP: &str = "2024-03-01";

fn world_file() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("worlds")
        .join("publieke_wereld.yaml")
}

/// Een wereld waarin één toekenning genomen is.
///
/// Dezelfde opzet als in [`tests/receipt.rs`](receipt.rs), en met opzet niet
/// gedeeld: een integratietest hoort te lezen als wat ze doet.
fn met_een_toekenning() -> World {
    let path = world_file();
    let definition =
        WorldDefinition::load(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut world = World::from_definition(&definition, &regulation_root())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));

    world
        .act(
            "burger.aanvraag",
            &BTreeMap::from([
                ("bsn".to_string(), Value::String(BSN.to_string())),
                ("jaar".to_string(), Value::Int(2024)),
                (
                    "ondertekend_op".to_string(),
                    Value::String("2024-01-09".to_string()),
                ),
            ]),
        )
        .expect("de aanvraag hoort te lukken");
    world
        .advance(BESLOTEN_OP.parse().expect("een datum"))
        .expect("de klok hoort vooruit te kunnen");
    world
        .act(
            "toeslagen.toekenning",
            &BTreeMap::from([("bsn".to_string(), Value::String(BSN.to_string()))]),
        )
        .expect("het besluit hoort te lukken");
    world
}

/// De trace van de toekenning, zoals het receipt haar draagt.
fn trace_van_de_toekenning(world: &World) -> Value {
    let receipt = world
        .gram_receipt("toeslagen", BESCHIKKINGEN, 0)
        .expect("het decretogram hoort een receipt te dragen");
    receipt
        .sections
        .get("results")
        .and_then(Value::as_object)
        .and_then(|results| results.get("trace"))
        .cloned()
        .unwrap_or_else(|| {
            panic!(
                "het receipt hoort de uitvoeringstrace in `results.trace` te dragen (wel: {:?})",
                receipt.sections.keys().collect::<Vec<_>>()
            )
        })
}

/// Elke (regeling, artikel) die in deze trace voorkomt.
fn geraakte_artikelen(node: &Value, gezien: &mut BTreeSet<(String, String)>) {
    let Some(velden) = node.as_object() else {
        return;
    };
    if let (Some(regeling), Some(artikel)) = (
        velden.get("regulation").and_then(Value::as_str),
        velden.get("article").and_then(Value::as_str),
    ) {
        gezien.insert((regeling.to_string(), artikel.to_string()));
    }
    for kind in velden
        .get("children")
        .and_then(Value::as_array)
        .unwrap_or_default()
    {
        geraakte_artikelen(kind, gezien);
    }
}

/// Elke veldnaam die ergens in deze trace voorkomt.
fn veldnamen(node: &Value, gezien: &mut BTreeSet<String>) {
    match node {
        Value::Object(velden) => {
            for (naam, waarde) in velden {
                gezien.insert(naam.clone());
                veldnamen(waarde, gezien);
            }
        }
        Value::Array(items) => {
            for item in items {
                veldnamen(item, gezien);
            }
        }
        _ => {}
    }
}

/// Een hash over de trace, om twee uitvoeringen te kunnen vergelijken.
///
/// Over de YAML-vorm en met SHA-256, dezelfde keuze als de hash over een
/// kroniekstroom in een decretogram.
fn hash(trace: &Value) -> String {
    let tekst = serde_yaml_ng::to_string(trace).expect("een trace is te serialiseren");
    format!("sha256:{:x}", Sha256::digest(tekst.as_bytes()))
}

/// Het besluit voert uit mét trace, en die trace komt in het receipt terecht.
#[test]
fn het_receipt_van_een_decretogram_draagt_de_uitvoeringstrace() {
    let world = met_een_toekenning();
    let trace = trace_van_de_toekenning(&world);
    let wortel = trace.as_object().expect("de trace is een boom van nodes");

    assert_eq!(
        wortel.get("node_type").and_then(Value::as_str),
        Some("article"),
        "de wortel van de trace is de uitvoering van de regeling zelf"
    );
    assert!(
        wortel
            .get("name")
            .and_then(Value::as_str)
            .is_some_and(|naam| naam.contains("wet_op_de_zorgtoeslag")),
        "de wortel noemt de uitgevoerde regeling: {:?}",
        wortel.get("name")
    );
    assert!(
        wortel
            .get("children")
            .and_then(Value::as_array)
            .is_some_and(|kinderen| !kinderen.is_empty()),
        "een uitvoering van deze regeling zet stappen, en die horen in de trace te staan"
    );
}

/// De scherpe: de trace noemt de artikelen die deze toekenning geraakt heeft.
///
/// Op naam vastgepind en niet geteld, want dat is het hele nut: een besluit
/// hoort terug te voeren op de artikelen die het droegen. De zorgtoeslag-
/// toekenning leunt op twee artikelen van haar eigen wet, op het artikel van de
/// AWIR dat het toeslagpartnerschap vaststelt, en op de ministeriële regeling
/// die de standaardpremie zet.
///
/// Verandert een van die wetten van indeling, dan valt deze test om. Dat is de
/// bedoeling: dan is het besluit langs een ander artikel gelopen, en dat hoort
/// iemand te zien in plaats van het stil te laten schuiven.
#[test]
fn de_trace_noemt_de_artikelen_die_geraakt_zijn() {
    let world = met_een_toekenning();
    let trace = trace_van_de_toekenning(&world);

    let mut geraakt = BTreeSet::new();
    geraakte_artikelen(&trace, &mut geraakt);

    let verwacht: BTreeSet<(String, String)> = [
        ("algemene_wet_inkomensafhankelijke_regelingen", "3"),
        ("regeling_standaardpremie", "1"),
        ("wet_op_de_zorgtoeslag", "2"),
        ("wet_op_de_zorgtoeslag", "3"),
    ]
    .into_iter()
    .map(|(regeling, artikel)| (regeling.to_string(), artikel.to_string()))
    .collect();

    assert_eq!(
        geraakt, verwacht,
        "de trace hoort precies de artikelen te noemen die deze toekenning droegen"
    );
}

/// Twee uitvoeringen met dezelfde invoer leveren dezelfde trace.
///
/// Dit is wat reproduceerbaarheid (RFC-013) van een trace vraagt, en het is
/// scherper dan "de uitkomst is gelijk": twee runs die langs een ander pad bij
/// hetzelfde bedrag uitkomen, zijn niet hetzelfde besluit.
#[test]
fn dezelfde_invoer_levert_dezelfde_trace() {
    let eerste = hash(&trace_van_de_toekenning(&met_een_toekenning()));
    let tweede = hash(&trace_van_de_toekenning(&met_een_toekenning()));

    assert_eq!(
        eerste, tweede,
        "dezelfde invoer hoort dezelfde trace op te leveren, tot op de hash"
    );
}

/// De trace draagt geen looptijden.
///
/// De engine kan ze meeschrijven (`duration_us` per node), en dat is precies wat
/// er niet in een gram hoort: het verschilt per run, dus het maakt de hash
/// hierboven waardeloos en de kroniek per run anders. De wandkloktijd van de
/// uitvoering staat één keer in het receipt, gelabeld als wat ze is; verder
/// draagt de trace geen tijd.
#[test]
fn de_trace_draagt_geen_looptijden() {
    let world = met_een_toekenning();
    let trace = trace_van_de_toekenning(&world);

    let mut namen = BTreeSet::new();
    veldnamen(&trace, &mut namen);

    assert!(
        !namen.contains("duration_us"),
        "een looptijd verschilt per run en hoort niet in een gram: {namen:?}"
    );
}

/// Een betaling wijst terug naar het decretogram waaruit ze volgt.
///
/// Cel, kroniek, plek en termijnnummer — samen genoeg om bij het besluit uit te
/// komen zonder de kroniek van de besluitende cel af te lopen. Dat de
/// verwijzing ook echt ergens op uitkomt, staat hier vast door hem te volgen:
/// het gram waar hij naar wijst is het besluit dat deze betaling opdroeg, en het
/// draagt de trace.
#[test]
fn een_betaling_wijst_naar_het_besluit_en_zijn_trace() {
    let mut world = met_een_toekenning();
    world
        .advance("2024-07-01".parse().expect("een datum"))
        .expect("de klok hoort langs de eerste termijnen te kunnen");

    let beeld = world.snapshot();
    let betaling = beeld
        .cells
        .iter()
        .flat_map(|cel| {
            cel.chronicles
                .iter()
                .flat_map(move |kroniek| kroniek.grams.iter().map(move |gram| (cel, kroniek, gram)))
        })
        .find(|(_, kroniek, gram)| kroniek.stream == "betalingen" && gram.name == "betaling")
        .map(|(_, _, gram)| gram)
        .expect("na drie maanden is er ten minste één termijn betaald");

    let veld = |naam: &str| {
        betaling
            .fields
            .get(naam)
            .map(|veld| veld.value.clone())
            .unwrap_or_else(|| {
                panic!(
                    "de betaling hoort '{naam}' te dragen (wel: {:?})",
                    betaling.fields.keys().collect::<Vec<_>>()
                )
            })
    };

    assert_eq!(veld("besluit_cel"), Value::String("toeslagen".to_string()));
    assert_eq!(
        veld("besluit_kroniek"),
        Value::String(BESCHIKKINGEN.to_string())
    );
    assert_eq!(
        veld("besluit_op_moment"),
        Value::String(BESLOTEN_OP.to_string())
    );
    assert_eq!(
        veld("volgnummer"),
        Value::Int(1),
        "de eerste termijn draagt haar termijnnummer"
    );

    // En de verwijzing volgen: dit is waar een lezer van de betaling uitkomt.
    let cel = veld("besluit_cel");
    let cel = cel.as_str().expect("de cel staat er als tekst");
    let kroniek = veld("besluit_kroniek");
    let kroniek = kroniek.as_str().expect("de kroniek staat er als tekst");
    let plek = veld("besluit_gram")
        .as_int()
        .expect("de plek staat er als getal");
    let plek = usize::try_from(plek).expect("een plek is niet negatief");

    let receipt = world
        .gram_receipt(cel, kroniek, plek)
        .expect("de verwijzing van de betaling hoort op een decretogram uit te komen");
    assert_eq!(
        receipt.gram.besluit.as_deref(),
        veld("besluit").as_str(),
        "de verwijzing hoort op het besluit uit te komen dat deze betaling opdroeg"
    );
    assert!(
        receipt
            .sections
            .get("results")
            .and_then(Value::as_object)
            .is_some_and(|results| results.contains_key("trace")),
        "en dat besluit draagt de trace waarlangs het tot stand kwam"
    );
}

/// Een tweede besluit wijst naar zijn éigen gram.
///
/// De plek van het decretogram wordt ingevuld nádat het gram ligt, en in een
/// wereld met één besluit is dat nul — precies de waarde die er ook zou staan als
/// er helemaal niets werd ingevuld. Daarmee zou elke andere manier om aan die
/// plek te komen even goed lijken: hem vooraf raden, of hem gewoon op nul laten.
/// Deze wereld neemt daarom hetzelfde besluit een tweede keer, want pas vanaf het
/// tweede gram in de kroniek zegt de plek iets.
#[test]
fn het_tweede_besluit_wijst_naar_zijn_eigen_gram() {
    let mut world = met_een_toekenning();
    let opnieuw: NaiveDate = "2024-06-01".parse().expect("een datum");
    world
        .advance(opnieuw)
        .expect("de klok hoort vooruit te kunnen");
    let tweede = world
        .decide(
            "toeslagen",
            "zorgtoeslag_toekenning",
            &BTreeMap::from([("bsn".to_string(), Value::String(BSN.to_string()))]),
            opnieuw,
        )
        .expect("hetzelfde besluit hoort nog eens genomen te kunnen worden");

    // Het tweede decretogram ligt op plek één, en daar wijzen zijn termijnen ook
    // naar: dát is wat een verwijzing die op nul blijft staan fout zou maken.
    assert!(
        !tweede.decretogram.obligations.is_empty(),
        "dit besluit hoort termijnen op te leggen, anders bewijst deze test niets"
    );
    for due in &tweede.decretogram.obligations {
        assert_eq!(
            due.besluit_gram, 1,
            "een termijn hoort naar het gram te wijzen waarin ze staat"
        );
    }

    let gram = world
        .gram_receipt("toeslagen", BESCHIKKINGEN, 1)
        .expect("er hoort een tweede decretogram te liggen");
    assert_eq!(gram.gram.besluit.as_deref(), Some("zorgtoeslag_toekenning"));
}
