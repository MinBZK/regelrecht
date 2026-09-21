//! Het lexogram: de regelingen uit `REGULATION_PATH`, geladen in de engine.

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
}
