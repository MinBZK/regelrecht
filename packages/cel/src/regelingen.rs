//! Het lexogram: de regelingen uit `REGULATION_PATH`, geladen in de engine.

use std::collections::BTreeSet;
use std::path::Path;

use regelrecht_engine::{Article, LawExecutionService};
use walkdir::WalkDir;

/// Laad elke regeling (een YAML-bestand met `$id` en `articles`) onder een
/// map. Andere YAML-bestanden (scenario's, notities) worden overgeslagen;
/// een regeling die de engine niet laadt is een fout.
pub fn laad(map: &Path) -> Result<LawExecutionService, Vec<String>> {
    if !map.is_dir() {
        return Err(vec![format!("{}: geen map", map.display())]);
    }
    let mut service = LawExecutionService::new();
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
        }
    }
    if fouten.is_empty() {
        Ok(service)
    } else {
        Err(fouten)
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn fixtures() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/regulation")
    }

    #[test]
    fn laadt_de_testregelingen() {
        let s = laad(&fixtures()).unwrap();
        assert!(s.has_law("testregeling_aanvraag"));
        assert!(s.has_law("testregeling_awb"));
    }

    #[test]
    fn grondslag_naar_artikel() {
        let s = laad(&fixtures()).unwrap();
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
        let s = laad(&fixtures()).unwrap();
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
}
