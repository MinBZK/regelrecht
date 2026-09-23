//! Het lexogram: de regelingen uit `REGULATION_PATH`, geladen in de engine.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use regelrecht_engine::{Article, LawExecutionService};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::stroom::GeladenRegeling;

/// Het corpus zoals de runtime het laadde: de engine met alle regelingen, en
/// de inventaris ervan voor het receipt van een besluit (RFC-013).
pub struct Corpus {
    pub service: LawExecutionService,
    pub regelingen: Vec<GeladenRegeling>,
}

/// Laad elke regeling (een YAML-bestand met `$id` en `articles`) onder een
/// map. Andere YAML-bestanden (scenario's, notities) worden overgeslagen;
/// een regeling die de engine niet laadt is een fout.
pub fn laad(map: &Path) -> Result<Corpus, Vec<String>> {
    if !map.is_dir() {
        return Err(vec![format!("{}: geen map", map.display())]);
    }
    let mut service = LawExecutionService::new();
    let mut geladen: Vec<GeladenRegeling> = Vec::new();
    let mut fouten = Vec::new();
    let mut bestanden: Vec<_> = WalkDir::new(map)
        .into_iter()
        .filter_entry(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .filter(|p| p.extension().is_some_and(|x| x == "yaml" || x == "yml"))
        .collect();
    bestanden.sort();
    for pad in bestanden {
        let Ok(tekst) = std::fs::read_to_string(&pad) else {
            fouten.push(format!("{}: niet te lezen", pad.display()));
            continue;
        };
        let is_regeling = serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&tekst)
            .ok()
            .is_some_and(|d| d.get("$id").is_some() && d.get("articles").is_some());
        if !is_regeling {
            continue;
        }
        if let Err(e) = service.load_law(&tekst) {
            fouten.push(format!("{}: {e}", pad.display()));
            continue;
        }
        geladen.push(inventariseer(&pad, &tekst));
    }
    geladen.sort();
    if fouten.is_empty() {
        Ok(Corpus {
            service,
            regelingen: geladen,
        })
    } else {
        Err(fouten)
    }
}

/// Een geladen regeling voor het receipt: haar `$id`, vanaf wanneer die
/// versie geldt en de hash van het bestand zoals gelezen. De bestandsnaam is
/// in het corpus de ingangsdatum; staat die er niet, dan telt `valid_from` of
/// `publication_date` uit het bestand zelf.
fn inventariseer(pad: &Path, tekst: &str) -> GeladenRegeling {
    let doc: serde_yaml_ng::Value = serde_yaml_ng::from_str(tekst).unwrap_or_default();
    let lees = |sleutel: &str| {
        doc.get(sleutel)
            .and_then(serde_yaml_ng::Value::as_str)
            .map(str::to_string)
    };
    let stam = pad
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .filter(|s| s.len() == 10 && s.split('-').count() == 3);
    GeladenRegeling {
        id: lees("$id").unwrap_or_default(),
        valid_from: stam
            .or_else(|| lees("valid_from"))
            .or_else(|| lees("publication_date"))
            .unwrap_or_default(),
        sha256: hex::encode(Sha256::digest(tekst.as_bytes())),
    }
}

/// Het artikel achter een grondslag `<regeling>#<artikel>`.
pub fn artikel<'s>(
    service: &'s LawExecutionService,
    grondslag: &str,
) -> Result<&'s Article, String> {
    let (regeling, nummer) = grondslag.split_once('#').ok_or_else(|| {
        format!("grondslag '{grondslag}' heeft niet de vorm <regeling>#<artikel>")
    })?;
    let law = service
        .resolver()
        .get_law(regeling)
        .ok_or_else(|| format!("grondslag '{grondslag}': regeling '{regeling}' is niet geladen"))?;
    law.find_article_by_number(nummer).ok_or_else(|| {
        format!("grondslag '{grondslag}': regeling '{regeling}' heeft geen artikel {nummer}")
    })
}

/// De parameters van een artikel en van elk artikel dat het via een invoer
/// aanroept, transitief: een invoer met `source.output` wijst naar het artikel
/// met die uitkomst, in `source.regulation` of in dezelfde regeling. Een
/// parameter die alleen een aangeroepen artikel declareert, telt mee: de
/// engine geeft hem door als de invoer geen eigen `parameters` meegeeft.
pub fn transitieve_parameters(
    service: &LawExecutionService,
    regeling: &str,
    artikel: &Article,
) -> BTreeSet<String> {
    let mut parameters = BTreeSet::new();
    let mut gezien: BTreeSet<(String, String)> = BTreeSet::new();
    let mut te_doen: Vec<(String, &Article)> = vec![(regeling.to_string(), artikel)];
    while let Some((law, a)) = te_doen.pop() {
        if !gezien.insert((law.clone(), a.number.clone())) {
            continue;
        }
        parameters.extend(a.get_parameters().iter().map(|p| p.name.clone()));
        for invoer in a.get_inputs() {
            let Some(bron) = &invoer.source else { continue };
            let Some(output) = &bron.output else { continue };
            let doel = bron.regulation.clone().unwrap_or_else(|| law.clone());
            if let Some(volgend) = service
                .resolver()
                .get_article_by_output(&doel, output, None)
            {
                te_doen.push((doel, volgend));
            }
        }
    }
    parameters
}

/// Een parameter die de aanroeper van een artikel moet leveren, met het
/// artikel dat hem declareert.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Benodigd {
    pub naam: String,
    /// `<regeling>#<artikel>`.
    pub artikel: String,
    #[serde(rename = "type")]
    pub soort: Value,
    pub nullable: bool,
    /// De omschrijving uit de regeling, met de herkomst volgens het model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub omschrijving: Option<String>,
}

/// De parameters die de aanroeper van een artikel moet leveren: die van het
/// artikel zelf, en die van elk artikel in dezelfde regeling dat het via een
/// invoer zonder eigen `parameters` aanroept, transitief; zo'n aanroep deelt
/// de parameters. Een invoer met `parameters` bindt de parameters van het
/// aangeroepen artikel zelf, en een aanroep van een andere regeling krijgt
/// alleen wat `parameters` meegeeft; die vraagt de aanroeper niet. Per naam
/// het eerste artikel dat hem declareert.
pub fn benodigde_parameters(
    service: &LawExecutionService,
    regeling: &str,
    artikel: &Article,
) -> BTreeMap<String, Benodigd> {
    let mut uit: BTreeMap<String, Benodigd> = BTreeMap::new();
    let mut gezien: BTreeSet<(String, String)> = BTreeSet::new();
    let mut te_doen: Vec<(String, &Article)> = vec![(regeling.to_string(), artikel)];
    while let Some((law, a)) = te_doen.pop() {
        if !gezien.insert((law.clone(), a.number.clone())) {
            continue;
        }
        for p in a.get_parameters() {
            uit.entry(p.name.clone()).or_insert_with(|| Benodigd {
                naam: p.name.clone(),
                artikel: format!("{law}#{}", a.number),
                soort: serde_json::to_value(p.param_type).unwrap_or(Value::Null),
                nullable: p.is_nullable(),
                omschrijving: p.description.clone(),
            });
        }
        for invoer in a.get_inputs() {
            let Some(bron) = &invoer.source else { continue };
            let Some(output) = &bron.output else { continue };
            let doel = bron.regulation.clone().unwrap_or_else(|| law.clone());
            if doel != law || bron.parameters.as_ref().is_some_and(|p| !p.is_empty()) {
                continue;
            }
            if let Some(volgend) = service
                .resolver()
                .get_article_by_output(&doel, output, None)
            {
                te_doen.push((doel, volgend));
            }
        }
    }
    uit
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn fixtures() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/regulation")
    }

    #[test]
    fn laadt_de_testregelingen() {
        let c = laad(&fixtures()).unwrap();
        assert!(c.service.has_law("testregeling_aanvraag"));
        assert!(c.service.has_law("testregeling_awb"));
        // De inventaris voor het receipt: elke regeling met haar hash.
        assert!(c.regelingen.iter().any(|r| r.id == "testregeling_aanvraag"
            && r.valid_from == "2025-01-01"
            && r.sha256.len() == 64));
    }

    #[test]
    fn grondslag_naar_artikel() {
        let s = laad(&fixtures()).unwrap().service;
        let a = artikel(&s, "testregeling_aanvraag#1").unwrap();
        assert!(a.get_parameters().iter().any(|p| p.name == "bevat_naam"));
        assert!(artikel(&s, "testregeling_aanvraag#9")
            .unwrap_err()
            .contains("geen artikel 9"));
        assert!(artikel(&s, "onbekend#1")
            .unwrap_err()
            .contains("niet geladen"));
        assert!(artikel(&s, "zonder_hekje").is_err());
    }

    #[test]
    fn transitieve_parameters_volgen_de_invoer() {
        let s = laad(&fixtures()).unwrap().service;
        let a = artikel(&s, "testregeling_afnemer#1").unwrap();
        let p = transitieve_parameters(&s, "testregeling_afnemer", a);
        // Eigen parameters, die van artikel 2 (zelfde regeling, geen binding)
        // en die van de testregeling register (andere regeling).
        for naam in [
            "bevat_aanduiding",
            "zetels_op_lijst",
            "is_ingeschreven_in_register",
        ] {
            assert!(p.contains(naam), "{naam} ontbreekt in {p:?}");
        }
        assert!(!p.contains("datum_vaststelling"));
    }

    #[test]
    fn benodigde_parameters_zonder_wat_een_invoer_bindt() {
        let s = laad(&fixtures()).unwrap().service;
        let a = artikel(&s, "testregeling_afnemer#1").unwrap();
        let p = benodigde_parameters(&s, "testregeling_afnemer", a);
        // Artikel 2 wordt zonder parameters aangeroepen: zijn parameter telt.
        assert_eq!(p["zetels_op_lijst"].artikel, "testregeling_afnemer#2");
        assert_eq!(p["datum_mededeling"].soort, serde_json::json!("date"));
        // De testregeling register krijgt haar parameters van artikel 1.
        assert!(!p.contains_key("is_ingeschreven_in_register"));
    }
}
