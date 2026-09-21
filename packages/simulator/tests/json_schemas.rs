//! De JSON-schema's in `schema/v0.6.0/` tegen wat deze crate werkelijk maakt.
//!
//! Drie schema's, drie toetsen:
//!
//! - **`gram.json`** — elk gram uit elk scenario dat draait, zoals het in het
//!   beeld van de wereld staat;
//! - **`world-snapshot.json`** — het beeld van elk van die runs, en de
//!   vastgepinde fixture `tests/fixtures/snapshot.json`;
//! - **`chronicle.json`** — elke kroniekstroom-definitie in elk scenario- en
//!   wereldbestand.
//!
//! Een schema dat alleen zegt wat er al staat, bewijst weinig; daarom staat hier
//! ook dat het de fouten weigert waarvoor het er is, en dat de varianten die het
//! onderscheidt in de scenario's ook echt voorkomen — anders zou een voorwaarde
//! in het schema nooit afgaan en toch groen staan.

#[path = "common/schema_contract.rs"]
mod schema_contract;

use regelrecht_simulator::{regulation_root, Scenario};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Alle YAML-bestanden onder `dir`, ook in submappen, gesorteerd.
///
/// Een map die niet te lezen is laat de toets vallen in plaats van haar stil
/// over te slaan: een scenario dat niet meedoet, valideert ook niet.
fn yaml_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = walkdir::WalkDir::new(dir)
        .into_iter()
        .map(|entry| entry.unwrap_or_else(|e| panic!("kan {} niet doorlopen: {e}", dir.display())))
        .filter(|entry| entry.file_type().is_file())
        .map(walkdir::DirEntry::into_path)
        .filter(|path| path.extension().is_some_and(|ext| ext == "yaml"))
        .collect();
    files.sort();
    files
}

/// De scenario's die draaien: alles onder `scenarios/` behalve `geweigerd/`,
/// waarvan het optuigen met opzet faalt. `negatief/` draait wel — het faalt pas
/// op de invarianten, en het beeld dat het achterlaat is even goed een beeld.
fn runnable_scenarios() -> Vec<PathBuf> {
    yaml_files(&crate_dir().join("scenarios"))
        .into_iter()
        .filter(|path| {
            !path
                .components()
                .any(|part| part.as_os_str() == "geweigerd")
        })
        .collect()
}

/// Het beeld na een run, als JSON.
fn snapshot_of(path: &Path) -> Value {
    let scenario = Scenario::load(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let run = scenario
        .run(&regulation_root())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::to_value(&run.snapshot).unwrap_or_else(|e| {
        panic!(
            "{}: het beeld moet naar JSON te schrijven zijn: {e}",
            path.display()
        )
    })
}

/// De naam van een scenario voor een melding: het pad onder de crate.
fn label(path: &Path) -> String {
    path.strip_prefix(crate_dir())
        .unwrap_or(path)
        .display()
        .to_string()
}

/// Welke variant van `gram.json` een gram raakt, om te kunnen zeggen dat elke
/// voorwaarde in het schema minstens één keer afging.
fn variant(gram: &Value) -> String {
    let kind = gram["kind"].as_str().unwrap_or("?");
    let name = gram["name"].as_str().unwrap_or("");
    let stage = &gram["fields"]["stage"];
    match kind {
        "decretogram" if stage["origin"]["herkomst"] == "besluit" => {
            format!("decretogram {}", stage["value"].as_str().unwrap_or("?"))
        }
        "decretogram" => "decretogram zonder besluit-pad".to_string(),
        "executogram" if name.starts_with("betaling_") || name.starts_with("terugvordering_") => {
            format!("executogram {name}")
        }
        other => other.to_string(),
    }
}

#[test]
fn elk_gram_en_elk_beeld_uit_elk_scenario_valideert() {
    let scenarios = runnable_scenarios();
    assert!(
        !scenarios.is_empty(),
        "zonder scenario bewijst deze toets niets"
    );

    let mut seen = BTreeSet::new();
    for path in &scenarios {
        let snapshot = snapshot_of(path);
        schema_contract::assert_snapshot_valid(&snapshot, &label(path));
        for (_, gram) in schema_contract::grams(&snapshot) {
            seen.insert(variant(gram));
        }
    }

    for expected in [
        "decretogram BESLUIT",
        "decretogram BEKENDMAKING",
        "decretogram zonder besluit-pad",
        "executogram",
        "executogram betaling_gedaan",
        "executogram betaling_gemeld",
        "executogram terugvordering_gedaan",
        "executogram terugvordering_gemeld",
    ] {
        assert!(
            seen.contains(expected),
            "geen enkel scenario levert een gram van de variant '{expected}'; \
             dan toetst gram.json die tak niet. Gezien: {seen:?}"
        );
    }
}

/// Het pad van de vastgepinde fixture: het beeld van de publieke wereld.
fn fixture_path() -> PathBuf {
    crate_dir()
        .join("tests")
        .join("fixtures")
        .join("snapshot.json")
}

/// De vastgepinde fixture, als JSON.
fn fixture() -> Value {
    let path = fixture_path();
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("kan {} niet lezen: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{} is geen JSON: {e}", path.display()))
}

#[test]
fn de_vastgepinde_fixture_valideert() {
    schema_contract::assert_snapshot_valid(&fixture(), &label(&fixture_path()));
}

/// Elke kroniekstroom-definitie in een wereld- of scenariobestand.
fn stream_definitions(path: &Path) -> Vec<(String, Value)> {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("kan {} niet lezen: {e}", path.display()));
    let document: Value = serde_yaml_ng::from_str(&text)
        .unwrap_or_else(|e| panic!("{} is geen YAML: {e}", path.display()));
    let mut found = Vec::new();
    for cell in document["cells"].as_array().into_iter().flatten() {
        for (index, stream) in cell["chronicles"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
        {
            found.push((
                format!(
                    "{}: cel '{}', kroniek {index}",
                    label(path),
                    cell["id"].as_str().unwrap_or("?")
                ),
                stream.clone(),
            ));
        }
    }
    found
}

#[test]
fn elke_kroniekstroom_definitie_valideert() {
    let files: Vec<PathBuf> = yaml_files(&crate_dir().join("scenarios"))
        .into_iter()
        .chain(yaml_files(&crate_dir().join("worlds")))
        .collect();

    let mut checked = 0;
    let mut with_schema = 0;
    for path in &files {
        for (what, stream) in stream_definitions(path) {
            schema_contract::assert_valid(schema_contract::chronicle(), &stream, &what);
            checked += 1;
            if stream["gebeurtenissen"]
                .as_array()
                .is_some_and(|g| !g.is_empty())
            {
                with_schema += 1;
            }
        }
    }
    assert!(checked > 0, "geen enkele kroniekstroom gevonden");
    assert!(
        with_schema > 0,
        "geen enkele stroom met een gebeurtenisschema; dan toetst chronicle.json dat deel niet"
    );
}

/// Het eerste gram van de variant, uit de vastgepinde fixture.
fn fixture_gram(wanted: &str) -> Value {
    schema_contract::grams(&fixture())
        .into_iter()
        .map(|(_, gram)| gram.clone())
        .find(|gram| variant(gram) == wanted)
        .unwrap_or_else(|| panic!("de fixture draagt geen gram van de variant '{wanted}'"))
}

fn assert_rejected(value: &Value, validator: &jsonschema::Validator, why: &str) {
    assert!(
        !schema_contract::errors(validator, value).is_empty(),
        "het schema hoort te weigeren: {why}"
    );
}

#[test]
fn het_gramschema_weigert_een_decretogram_zonder_zijn_vaste_velden() {
    let gram = fixture_gram("decretogram BESLUIT");
    schema_contract::assert_valid(schema_contract::gram(), &gram, "het decretogram zelf");

    for veld in [
        "competent_authority",
        "legal_character",
        "regulation",
        "regulation_valid_from",
        "chronicle_sources",
        "obligations",
        "zaakkenmerk",
    ] {
        let mut zonder = gram.clone();
        zonder["fields"]
            .as_object_mut()
            .expect("fields is een object")
            .remove(veld);
        assert_rejected(
            &zonder,
            schema_contract::gram(),
            &format!("een decretogram zonder '{veld}'"),
        );
    }

    let mut geen_beschikking = gram.clone();
    geen_beschikking["fields"]["legal_character"]["value"] = json!("BELEIDSREGEL");
    assert_rejected(
        &geen_beschikking,
        schema_contract::gram(),
        "een decretogram dat geen beschikking is",
    );

    let mut verkeerd_kanaal = gram;
    verkeerd_kanaal["intake"] = json!("levering");
    assert_rejected(
        &verkeerd_kanaal,
        schema_contract::gram(),
        "een decretogram dat niet via eigen_besluit ontstond",
    );
}

#[test]
fn het_gramschema_weigert_een_betaling_zonder_verwijzing_of_volgnummer() {
    let gram = fixture_gram("executogram betaling_gedaan");
    schema_contract::assert_valid(schema_contract::gram(), &gram, "de betaling zelf");

    for veld in ["volgnummer", "besluit", "zaakkenmerk", "bedrag"] {
        let mut zonder = gram.clone();
        zonder["fields"]
            .as_object_mut()
            .expect("fields is een object")
            .remove(veld);
        assert_rejected(
            &zonder,
            schema_contract::gram(),
            &format!("een betaling zonder '{veld}'"),
        );
    }

    let mut halve_verwijzing = gram.clone();
    halve_verwijzing["fields"]
        .as_object_mut()
        .expect("fields is een object")
        .remove("besluit_gram");
    assert_rejected(
        &halve_verwijzing,
        schema_contract::gram(),
        "een verwijzing naar het besluit zonder de plek in de kroniek",
    );

    let mut nulde_termijn = gram.clone();
    nulde_termijn["fields"]["volgnummer"]["value"] = json!(0);
    assert_rejected(
        &nulde_termijn,
        schema_contract::gram(),
        "een termijn met volgnummer 0",
    );

    let mut onbekende_soort = gram;
    onbekende_soort["kind"] = json!("chronogram");
    assert_rejected(
        &onbekende_soort,
        schema_contract::gram(),
        "een vierde soort gram",
    );
}

#[test]
fn het_gramschema_weigert_een_bekendmaking_zonder_haar_vaste_velden() {
    let snapshot = snapshot_of(&crate_dir().join("scenarios").join("bekendmaking.yaml"));
    let gram = schema_contract::grams(&snapshot)
        .into_iter()
        .map(|(_, gram)| gram.clone())
        .find(|gram| variant(gram) == "decretogram BEKENDMAKING")
        .expect("het scenario van de bekendmaking legt een bekendmaking vast");
    schema_contract::assert_valid(schema_contract::gram(), &gram, "de bekendmaking zelf");

    for veld in [
        "bekendgemaakt_door",
        "besluit_gram",
        "hooks",
        "termijnen_vervallen_door",
    ] {
        let mut zonder = gram.clone();
        zonder["fields"]
            .as_object_mut()
            .expect("fields is een object")
            .remove(veld);
        assert_rejected(
            &zonder,
            schema_contract::gram(),
            &format!("een bekendmaking zonder '{veld}'"),
        );
    }
}

#[test]
fn het_beeldschema_toetst_de_grammen_erin() {
    let mut snapshot = fixture();

    // Het eerste decretogram uit het besluit-pad, zonder bevoegd gezag: het beeld
    // verwijst voor zijn grammen naar gram.json, dus ook het beeld hoort dit te
    // weigeren.
    let gram = snapshot["cells"]
        .as_array_mut()
        .expect("cells is een lijst")
        .iter_mut()
        .flat_map(|cell| {
            cell["chronicles"]
                .as_array_mut()
                .expect("chronicles is een lijst")
                .iter_mut()
        })
        .flat_map(|chronicle| {
            chronicle["grams"]
                .as_array_mut()
                .expect("grams is een lijst")
                .iter_mut()
        })
        .find(|gram| variant(gram) == "decretogram BESLUIT")
        .expect("de fixture draagt een besluit");
    gram["fields"]
        .as_object_mut()
        .expect("fields is een object")
        .remove("competent_authority");

    assert_rejected(
        &snapshot,
        schema_contract::world_snapshot(),
        "een beeld met een decretogram zonder bevoegd gezag",
    );
}

/// Een lexogram is de regeling zelf en ligt in geen enkele kroniek (RFC-022
/// §1.1). `gram.json` noemt de soort voor het vocabulaire; het beeld hoort een
/// kroniek die er een draagt te weigeren, en een verwijzing ernaar ook.
#[test]
fn het_beeldschema_weigert_een_lexogram_in_een_kroniek() {
    let snapshot = fixture();
    schema_contract::assert_valid(schema_contract::world_snapshot(), &snapshot, "de fixture");

    let mut in_de_kroniek = snapshot.clone();
    let gram = in_de_kroniek["cells"]
        .as_array_mut()
        .expect("cells is een lijst")
        .iter_mut()
        .flat_map(|cell| {
            cell["chronicles"]
                .as_array_mut()
                .expect("chronicles is een lijst")
                .iter_mut()
        })
        .flat_map(|chronicle| {
            chronicle["grams"]
                .as_array_mut()
                .expect("grams is een lijst")
                .iter_mut()
        })
        .find(|gram| gram["kind"] == "executogram")
        .expect("de fixture draagt een executogram");
    gram["kind"] = json!("lexogram");
    assert_rejected(
        &in_de_kroniek,
        schema_contract::world_snapshot(),
        "een lexogram in een kroniek",
    );

    let mut in_het_journaal = snapshot;
    let verwijzing = in_het_journaal["journal"]
        .as_array_mut()
        .expect("journal is een lijst")
        .iter_mut()
        .find_map(|entry| {
            entry["grams"]
                .as_array_mut()
                .and_then(|grams| grams.first_mut())
        })
        .expect("het journaal van de fixture wijst naar een gram");
    verwijzing["kind"] = json!("lexogram");
    assert_rejected(
        &in_het_journaal,
        schema_contract::world_snapshot(),
        "een journaalregel die naar een lexogram in een kroniek wijst",
    );
}

#[test]
fn het_kroniekschema_weigert_een_typfout() {
    let stroom = json!({
        "stream": "betalingen",
        "key": "zaakkenmerk",
        "gebeurtenissen": [{
            "name": "betaling_gedaan",
            "intake": "betaling",
            "fields": [{ "name": "bedrag", "type": "amount" }]
        }]
    });
    schema_contract::assert_valid(schema_contract::chronicle(), &stroom, "de stroom zelf");

    let mut onbekende_sleutel = stroom.clone();
    onbekende_sleutel["gebeurtenisen"] = onbekende_sleutel["gebeurtenissen"].take();
    assert_rejected(
        &onbekende_sleutel,
        schema_contract::chronicle(),
        "een onbekende sleutel in de stroom",
    );

    let mut onbekend_kanaal = stroom.clone();
    onbekend_kanaal["gebeurtenissen"][0]["intake"] = json!("post");
    assert_rejected(
        &onbekend_kanaal,
        schema_contract::chronicle(),
        "een kanaal dat er niet is",
    );

    let mut onbekend_type = stroom.clone();
    onbekend_type["gebeurtenissen"][0]["fields"][0]["type"] = json!("euro");
    assert_rejected(
        &onbekend_type,
        schema_contract::chronicle(),
        "een veldtype dat er niet is",
    );

    let mut zonder_sleutel = stroom;
    zonder_sleutel
        .as_object_mut()
        .expect("de stroom is een object")
        .remove("key");
    assert_rejected(
        &zonder_sleutel,
        schema_contract::chronicle(),
        "een stroom zonder sleutelveld",
    );
}
