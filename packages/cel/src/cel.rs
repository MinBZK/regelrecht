//! Een cel: een map onder `CELLS_PATH`, geladen en gecontroleerd.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use regelrecht_engine::LawExecutionService;

use crate::config::{CelDefinitie, Portaal};
use crate::formulier::{self, Formulier};
use crate::reductie::{self, Lexostatussen};
use crate::stroom::{self, Event, Gram, Stroom};
use crate::voorbeelden::{self, Voorbeelden};
use crate::{controle, startstand};

/// Een geladen cel die de controles bij het opstarten doorstond.
pub struct Cel {
    pub definitie: CelDefinitie,
    /// De map van de cel; paden in `cel.yaml` zijn hier relatief aan.
    pub map: PathBuf,
    pub strommen: Vec<Stroom>,
    pub lexostatussen: Lexostatussen,
    /// Het corpus, gedeeld door alle cellen van de runtime.
    pub service: Arc<LawExecutionService>,
    pub formulier: Option<Formulier>,
    /// De grammen voor een lege kroniek (leeg zonder `startstand`).
    pub startstand: Vec<Gram>,
    /// Standaardgegevens per handeling (leeg zonder `voorbeelden`).
    pub voorbeelden: Voorbeelden,
}

impl Cel {
    /// Laad een cel uit haar map en controleer haar. Elke fout komt terug,
    /// niet alleen de eerste, en elke fout noemt de cel.
    pub fn laad(map: &Path, service: Arc<LawExecutionService>) -> Result<Self, Vec<String>> {
        let naam = map
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let definitie = CelDefinitie::laad(map).map_err(|f| met_cel(&naam, f))?;
        Self::laad_definitie(definitie, map, service)
    }

    fn laad_definitie(
        mut definitie: CelDefinitie,
        map: &Path,
        service: Arc<LawExecutionService>,
    ) -> Result<Self, Vec<String>> {
        let id = definitie.id.clone();
        let fout = |f: Vec<String>| met_cel(&id, f);
        let mut fouten = Vec::new();
        let mut strommen = Vec::new();
        for pad in &definitie.stromen {
            match stroom::laad(&map.join(pad)) {
                Ok(s) => strommen.extend(s),
                Err(f) => fouten.extend(f),
            }
        }
        let lexostatussen = reductie::laad(&map.join(&definitie.lexostatussen))
            .map_err(|f| fouten.extend(f))
            .ok();
        let Some(lexostatussen) = lexostatussen.filter(|_| fouten.is_empty()) else {
            return Err(fout(fouten));
        };
        if lexostatussen.cel != definitie.id {
            fouten.push(format!(
                "{}: cel '{}' is niet de id van deze cel",
                definitie.lexostatussen, lexostatussen.cel
            ));
        }
        for s in &strommen {
            if s.recording_actor != definitie.recording_actor {
                fouten.push(format!(
                    "stroom '{}' heeft recording_actor '{}', de cel '{}'",
                    s.id, s.recording_actor, definitie.recording_actor
                ));
            }
        }
        fouten.extend(vind_besluit(&mut definitie, &service));
        if let Err(f) = controle::controleer(
            &strommen,
            &lexostatussen,
            definitie.portaal.as_ref(),
            &service,
        ) {
            fouten.extend(f);
        }
        let formulier = match definitie
            .portaal
            .as_ref()
            .and_then(|p| p.formulier.as_ref())
        {
            Some(f) => formulier::laad(&map.join(&f.pad), &f.scherm)
                .map_err(|e| fouten.push(e))
                .ok(),
            None => None,
        };
        let startstand = match &definitie.startstand {
            Some(pad) => startstand::laad(&map.join(pad), &strommen)
                .map_err(|f| fouten.extend(f))
                .unwrap_or_default(),
            None => Vec::new(),
        };
        let voorbeelden = match &definitie.voorbeelden {
            Some(v) => {
                fouten.extend(voorbeelden_zonder_handeling(&definitie, v));
                voorbeelden::laad(map, v)
                    .map_err(|f| fouten.extend(f))
                    .unwrap_or_default()
            }
            None => Voorbeelden::default(),
        };
        if !fouten.is_empty() {
            return Err(fout(fouten));
        }
        Ok(Self {
            definitie,
            map: map.to_path_buf(),
            strommen,
            lexostatussen,
            service,
            formulier,
            startstand,
            voorbeelden,
        })
    }

    pub fn id(&self) -> &str {
        &self.definitie.id
    }

    /// Het portaalblok, als de cel een portaal heeft.
    pub fn portaal(&self) -> Option<&Portaal> {
        self.definitie.portaal.as_ref()
    }

    /// De stroom en het event waarin het portaal vastlegt.
    pub fn portaal_event(&self) -> Option<(&Stroom, &Event)> {
        let p = self.portaal()?;
        let s = self.strommen.iter().find(|s| s.id == p.stroom)?;
        Some((s, s.event(&p.event)?))
    }

    /// De rijen-definities van het besluit (synthese per regel).
    pub fn rijen(&self) -> &[crate::config::RijenDefinitie] {
        self.definitie
            .behandeling
            .as_ref()
            .map(|b| b.besluit.rijen.as_slice())
            .unwrap_or_default()
    }

    /// De kronieken van de cel, gesorteerd en zonder dubbelen.
    pub fn kronieken(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self.strommen.iter().map(|s| s.chronicle.as_str()).collect();
        v.sort_unstable();
        v.dedup();
        v
    }
}

/// Een voorbeeld voor een handeling die de cel niet heeft, is een fout:
/// logins of een aanvraag zonder portaal, een besluit zonder behandeling.
/// (Dat een portaal de rol aanvrager heeft, controleert
/// [`crate::besluit::controleer`].)
fn voorbeelden_zonder_handeling(
    definitie: &CelDefinitie,
    v: &crate::config::VoorbeeldenDefinitie,
) -> Vec<String> {
    let mut fouten = Vec::new();
    if !v.inloggen.is_empty() && definitie.portaal.is_none() {
        fouten.push("voorbeelden.inloggen: de cel heeft geen portaal".to_string());
    }
    if v.aanvraag.is_some() && definitie.portaal.is_none() {
        fouten.push("voorbeelden.aanvraag: de cel heeft geen portaal".to_string());
    }
    if v.besluit.is_some() && definitie.behandeling.is_none() {
        fouten.push("voorbeelden.besluit: de cel heeft geen behandeling".to_string());
    }
    fouten
}

/// Vul de regeling van het besluit in uit de wet, als `cel.yaml` haar niet
/// noemt: de beschikking waarvoor het bevoegd gezag de `recording_actor` van
/// de cel is. Precies een zo'n beschikking, en de uitkomsten van het besluit
/// komen uit dat artikel; anders een fout die de kandidaten noemt.
fn vind_besluit(definitie: &mut CelDefinitie, service: &LawExecutionService) -> Vec<String> {
    let actor = definitie.recording_actor.clone();
    let Some(b) = definitie.behandeling.as_mut().map(|b| &mut b.besluit) else {
        return Vec::new();
    };
    if !b.regeling.is_empty() {
        return Vec::new();
    }
    let kandidaten = crate::besluit::beschikkingen_van(service, &actor);
    let [(regeling, artikel)] = kandidaten.as_slice() else {
        let lijst: Vec<String> = kandidaten.iter().map(|(r, a)| format!("{r}#{a}")).collect();
        return vec![if lijst.is_empty() {
            format!(
                "besluit: geen regeling noemt '{actor}' als bevoegd gezag bij een BESCHIKKING; noem de regeling in behandeling.besluit.regeling"
            )
        } else {
            format!(
                "besluit: '{actor}' is bevoegd voor meer dan een beschikking ({}); kies er een met behandeling.besluit.regeling",
                lijst.join(", ")
            )
        }];
    };
    let mut fouten = Vec::new();
    for u in &b.uitkomsten {
        let van = service
            .resolver()
            .get_article_by_output(regeling, u, None)
            .map(|a| a.number.clone());
        if van.as_deref() != Some(artikel.as_str()) {
            fouten.push(format!(
                "besluit: uitkomst '{u}' komt niet uit {regeling}#{artikel}, de beschikking waarvoor '{actor}' bevoegd is"
            ));
        }
    }
    b.regeling = regeling.clone();
    fouten
}

/// Zet de cel voor elke melding.
pub fn met_cel(cel: &str, fouten: Vec<String>) -> Vec<String> {
    fouten
        .into_iter()
        .map(|f| format!("cel '{cel}': {f}"))
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    pub(crate) fn fixtures() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
    }

    fn service() -> Arc<LawExecutionService> {
        Arc::new(
            crate::regelingen::laad(&fixtures().join("regulation"))
                .unwrap()
                .service,
        )
    }

    #[test]
    fn fixture_cellen_laden() {
        let s = service();
        let instantie = Cel::laad(&fixtures().join("cellen/instantie"), s.clone()).unwrap();
        assert_eq!(
            instantie.portaal_event().unwrap().1.name,
            "aanvraag_ontvangen"
        );
        assert!(instantie.formulier.is_some());
        assert!(instantie.startstand.is_empty());

        let register = Cel::laad(&fixtures().join("cellen/register"), s.clone()).unwrap();
        assert!(register.portaal().is_none());
        assert_eq!(register.startstand.len(), 4);
        assert_eq!(register.kronieken(), ["test_register"]);

        let afnemer = Cel::laad(&fixtures().join("cellen/afnemer"), s).unwrap();
        assert_eq!(afnemer.definitie.synthese.len(), 1);
    }

    /// Kopieer een fixture-cel en de stromen naar een tijdelijke map, zodat
    /// een test er een bestand in kan veranderen.
    fn kopie(cel: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let f = fixtures();
        std::fs::create_dir_all(dir.path().join("cellen").join(cel)).unwrap();
        std::fs::create_dir_all(dir.path().join("chronicles")).unwrap();
        for e in std::fs::read_dir(f.join("cellen").join(cel)).unwrap() {
            let p = e.unwrap().path();
            std::fs::copy(
                &p,
                dir.path()
                    .join("cellen")
                    .join(cel)
                    .join(p.file_name().unwrap()),
            )
            .unwrap();
        }
        for e in std::fs::read_dir(f.join("chronicles")).unwrap() {
            let p = e.unwrap().path();
            std::fs::copy(
                &p,
                dir.path().join("chronicles").join(p.file_name().unwrap()),
            )
            .unwrap();
        }
        dir
    }

    #[test]
    fn elke_melding_noemt_de_cel() {
        let dir = kopie("register");
        let map = dir.path().join("cellen/register");
        let lexo = std::fs::read_to_string(map.join("lexostatussen.yaml")).unwrap();
        std::fs::write(
            map.join("lexostatussen.yaml"),
            lexo.replace("cel: test_register", "cel: ander_register"),
        )
        .unwrap();
        let fouten = Cel::laad(&map, service()).err().unwrap();
        assert!(!fouten.is_empty());
        assert!(
            fouten
                .iter()
                .all(|f| f.starts_with("cel 'test_register': ")),
            "{fouten:?}"
        );
        assert!(fouten
            .iter()
            .any(|f| f.contains("'ander_register' is niet de id")));
    }

    #[test]
    fn ontbrekende_bestanden_worden_allemaal_gemeld() {
        let dir = kopie("instantie");
        let map = dir.path().join("cellen/instantie");
        std::fs::remove_file(map.join("lexostatussen.yaml")).unwrap();
        std::fs::remove_file(dir.path().join("chronicles/test_aanvragen.yaml")).unwrap();
        let fouten = Cel::laad(&map, service()).err().unwrap();
        assert_eq!(fouten.len(), 2, "{fouten:?}");
    }

    #[test]
    fn cel_zonder_cel_yaml_noemt_de_map() {
        let dir = tempfile::tempdir().unwrap();
        let map = dir.path().join("leeg");
        std::fs::create_dir_all(&map).unwrap();
        let fouten = Cel::laad(&map, service()).err().unwrap();
        assert!(fouten[0].starts_with("cel 'leeg': "), "{fouten:?}");
    }

    #[test]
    fn stroom_van_een_andere_actor() {
        let dir = kopie("register");
        let map = dir.path().join("cellen/register");
        let cel = std::fs::read_to_string(map.join("cel.yaml")).unwrap();
        std::fs::write(
            map.join("cel.yaml"),
            cel.replace(
                "recording_actor: test_register",
                "recording_actor: iemand_anders",
            ),
        )
        .unwrap();
        let fouten = Cel::laad(&map, service()).err().unwrap();
        assert!(
            fouten
                .iter()
                .any(|f| f.contains("recording_actor 'test_register'")),
            "{fouten:?}"
        );
    }

    #[test]
    fn voorbeelden_van_de_fixture() {
        let afnemer = Cel::laad(&fixtures().join("cellen/afnemer"), service()).unwrap();
        let labels: Vec<&str> = afnemer
            .voorbeelden
            .inloggen
            .iter()
            .map(|v| v.label.as_str())
            .collect();
        assert_eq!(labels, ["voorbeeld-login", "voorbeeld-login-ander"]);
        assert!(afnemer.voorbeelden.aanvraag.is_some());
        assert!(afnemer.voorbeelden.besluit.is_some());
        let register = Cel::laad(&fixtures().join("cellen/register"), service()).unwrap();
        assert!(register.voorbeelden.inloggen.is_empty());
    }

    #[test]
    fn voorbeeld_voor_een_handeling_die_de_cel_niet_heeft() {
        let dir = kopie("register");
        let map = dir.path().join("cellen/register");
        let cel = std::fs::read_to_string(map.join("cel.yaml")).unwrap();
        std::fs::write(map.join("x.json"), r#"{"external": {}}"#).unwrap();
        std::fs::write(
            map.join("cel.yaml"),
            format!("{cel}\nvoorbeelden:\n  aanvraag: x.json\n  besluit: weg.json\n"),
        )
        .unwrap();
        let fouten = Cel::laad(&map, service()).err().unwrap();
        assert_eq!(fouten.len(), 3, "{fouten:?}");
        assert!(fouten[0].contains("voorbeelden.aanvraag: de cel heeft geen portaal"));
        assert!(fouten[1].contains("voorbeelden.besluit: de cel heeft geen behandeling"));
        assert!(fouten[2].contains("weg.json"), "{fouten:?}");
    }
}
