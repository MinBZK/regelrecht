//! Het lexogram: de regelingen uit `REGULATION_PATH`, geladen in de engine.
//!
//! Waarom een eigen lader en niet `RuleResolver::load_from_directory` van de
//! engine: die is ruimhartig (een regeling die niet laadt wordt een
//! waarschuwing en overgeslagen), terwijl de runtime dan niet mag starten,
//! want een besluit zou anders op een onvolledig corpus rusten. Ook houdt de
//! runtime per regeling de tekst vast voor de hash in het receipt (RFC-013),
//! en slaat ze YAML-bestanden over die geen regeling zijn (scenario's,
//! notities) in plaats van ze als mislukte regeling te melden. Het laden van
//! een enkele regeling is wel die van de engine (`load_law`).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use regelrecht_engine::{Article, LawExecutionService};
use regelrecht_law_model::{ParameterType, TypeSpec};
use serde::Serialize;
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::gram::GeladenRegeling;

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
    let mut bestanden = Vec::new();
    for item in WalkDir::new(map)
        .into_iter()
        .filter_entry(|e| !e.file_name().to_string_lossy().starts_with('.'))
    {
        match item {
            Ok(e) if e.file_type().is_file() => bestanden.push(e.into_path()),
            Ok(_) => {}
            // Een map of bestand dat niet te lezen is: een regeling kan er
            // ontbreken, dus dat is een fout.
            Err(e) => fouten.push(format!("{}: {e}", map.display())),
        }
    }
    bestanden.retain(|p| p.extension().is_some_and(|x| x == "yaml" || x == "yml"));
    bestanden.sort();
    for pad in bestanden {
        let tekst = match std::fs::read_to_string(&pad) {
            Ok(t) => t,
            Err(e) => {
                fouten.push(format!("{}: {e}", pad.display()));
                continue;
            }
        };
        let doc = match serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&tekst) {
            Ok(d) => d,
            Err(e) => {
                // Geen YAML, dus geen regeling; wel het melden waard.
                tracing::warn!(pad = %pad.display(), "geen geldige YAML, overgeslagen: {e}");
                continue;
            }
        };
        if doc.get("$id").is_none() || doc.get("articles").is_none() {
            continue;
        }
        match service.load_law(&tekst) {
            Ok(id) => {
                // De engine laadt een regeling ook met een ongeldige `origin`
                // (die leest ze niet); de runtime start dan niet, met bestand,
                // artikel en parameter in de melding (RFC-043).
                if let Some(law) = service.resolver().get_law(&id) {
                    fouten.extend(
                        crate::origin::valideer(law)
                            .into_iter()
                            .map(|f| format!("{}: {f}", pad.display())),
                    );
                }
                geladen.push(inventariseer(&pad, &tekst, &doc, id));
            }
            Err(e) => fouten.push(format!("{}: {e}", pad.display())),
        }
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
fn inventariseer(
    pad: &Path,
    tekst: &str,
    doc: &serde_yaml_ng::Value,
    id: String,
) -> GeladenRegeling {
    let lees = |sleutel: &str| {
        doc.get(sleutel)
            .and_then(serde_yaml_ng::Value::as_str)
            .map(str::to_string)
    };
    let stam = pad
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .filter(|s| s.len() == 10 && s.split('-').count() == 3);
    let valid_from = stam
        .or_else(|| lees("valid_from"))
        .or_else(|| lees("publication_date"));
    if valid_from.is_none() {
        tracing::warn!(regeling = %id, bestand = %pad.display(), "regeling zonder datum in bestandsnaam, valid_from of publication_date: het receipt noemt geen versie");
    }
    GeladenRegeling {
        id,
        valid_from: valid_from.unwrap_or_default(),
        sha256: hex::encode(Sha256::digest(tekst.as_bytes())),
    }
}

/// Een grondslag, ontleed: `<regeling>#<artikel>` of
/// `<regeling>#<artikel> lid <n>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grondslag<'g> {
    pub regeling: &'g str,
    pub artikel: &'g str,
    pub lid: Option<&'g str>,
}

/// Of een tekst een lidnummer is: cijfers, eventueel met een letter.
fn is_lidnummer(tekst: &str) -> bool {
    let cijfers = tekst.trim_end_matches(|c: char| c.is_ascii_lowercase());
    !cijfers.is_empty()
        && cijfers.bytes().all(|b| b.is_ascii_digit())
        && tekst.len() - cijfers.len() <= 1
}

/// Ontleed een grondslag. Een artikelnummer kan een spatie hebben
/// (`kieswet#G 1`), dus het lid staat achter de laatste ` lid `, en alleen als
/// daar een lidnummer staat.
pub fn ontleed(grondslag: &str) -> Result<Grondslag<'_>, String> {
    let (regeling, rest) = grondslag.split_once('#').ok_or_else(|| {
        format!("grondslag '{grondslag}' heeft niet de vorm <regeling>#<artikel>")
    })?;
    let (artikel, lid) = match rest.rsplit_once(" lid ") {
        Some((artikel, lid)) if is_lidnummer(lid) => (artikel, Some(lid)),
        _ => (rest, None),
    };
    Ok(Grondslag {
        regeling,
        artikel,
        lid,
    })
}

/// Of een artikeltekst een lid heeft: een regel die met `<n>.` of `<n> `
/// begint.
pub fn heeft_lid(artikel: &Article, lid: &str) -> bool {
    artikel.text.lines().any(|regel| {
        regel
            .trim_start()
            .strip_prefix(lid)
            .is_some_and(|rest| rest.starts_with('.') || rest.starts_with(' '))
    })
}

/// Het artikel achter een grondslag `<regeling>#<artikel>`, ook als de
/// grondslag een lid noemt: een lid heeft geen eigen parameters. Of het lid
/// bestaat, controleert [`heeft_lid`].
pub fn artikel<'s>(
    service: &'s LawExecutionService,
    grondslag: &str,
) -> Result<&'s Article, String> {
    let Grondslag {
        regeling,
        artikel: nummer,
        ..
    } = ontleed(grondslag)?;
    let law = service
        .resolver()
        .get_law(regeling)
        .ok_or_else(|| format!("grondslag '{grondslag}': regeling '{regeling}' is niet geladen"))?;
    law.find_article_by_number(nummer).ok_or_else(|| {
        format!("grondslag '{grondslag}': regeling '{regeling}' heeft geen artikel {nummer}")
    })
}

/// Het artikel achter een grondslag, en het lid dat ze noemt moet de
/// artikeltekst hebben (zie [`heeft_lid`]). Gedeeld door de controles op de
/// grondslag van een event, van een afleiding en van een formulierveld.
pub fn geldig<'s>(
    service: &'s LawExecutionService,
    grondslag: &str,
) -> Result<&'s Article, String> {
    let a = artikel(service, grondslag)?;
    if let Some(lid) = ontleed(grondslag)?.lid.filter(|l| !heeft_lid(a, l)) {
        return Err(format!(
            "grondslag '{grondslag}': artikel {} heeft geen lid {lid} (geen regel die met '{lid}.' of '{lid} ' begint)",
            a.number
        ));
    }
    Ok(a)
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

/// Het type van een waarde volgens de regeling: `type` en de eenheid
/// (`type_spec.unit`, zoals `eurocent`). Een frontend toont en vraagt een
/// waarde daarmee, niet naar haar naam.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Waardetype {
    #[serde(rename = "type")]
    pub soort: ParameterType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eenheid: Option<String>,
}

impl Waardetype {
    pub fn nieuw(soort: ParameterType, type_spec: Option<&TypeSpec>) -> Self {
        Self {
            soort,
            eenheid: type_spec.and_then(|t| t.unit.clone()),
        }
    }
}

/// De typen van de uitkomsten van een artikel, per naam.
pub fn uitkomsttypen(service: &LawExecutionService, artikel: &str) -> BTreeMap<String, Waardetype> {
    self::artikel(service, artikel)
        .ok()
        .and_then(|a| a.get_execution_spec())
        .and_then(|e| e.output.as_ref())
        .map(|o| {
            o.iter()
                .map(|o| {
                    (
                        o.name.clone(),
                        Waardetype::nieuw(o.output_type, o.type_spec.as_ref()),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Een parameter die de aanroeper van een artikel moet leveren, met het
/// artikel dat hem declareert.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Benodigd {
    pub naam: String,
    /// `<regeling>#<artikel>`.
    pub artikel: String,
    #[serde(flatten)]
    pub typering: Waardetype,
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
                typering: Waardetype::nieuw(p.param_type, p.type_spec.as_ref()),
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
    fn grondslag_met_een_lid() {
        assert_eq!(
            ontleed("een_wet#102 lid 1").unwrap(),
            Grondslag {
                regeling: "een_wet",
                artikel: "102",
                lid: Some("1")
            }
        );
        assert_eq!(ontleed("een_wet#4:2 lid 2a").unwrap().lid, Some("2a"));
        // Een artikelnummer met een spatie, met en zonder lid.
        assert_eq!(
            ontleed("kieswet#G 1 lid 3").unwrap(),
            Grondslag {
                regeling: "kieswet",
                artikel: "G 1",
                lid: Some("3")
            }
        );
        assert_eq!(ontleed("kieswet#G 1").unwrap().artikel, "G 1");
        // Geen lidnummer: dan hoort het bij het artikelnummer.
        assert_eq!(ontleed("een_wet#A lid B").unwrap().artikel, "A lid B");
        assert_eq!(ontleed("een_wet#1 lid 12ab").unwrap().lid, None);

        let s = laad(&fixtures()).unwrap().service;
        let a = artikel(&s, "testregeling_aanvraag#1 lid 1").unwrap();
        assert_eq!(a.number, "1");
        assert!(heeft_lid(a, "1"), "{}", a.text);
        assert!(!heeft_lid(a, "9"));
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
        assert_eq!(p["datum_mededeling"].typering.soort, ParameterType::Date);
        // De testregeling register krijgt haar parameters van artikel 1.
        assert!(!p.contains_key("is_ingeschreven_in_register"));
    }
}
