//! Een cel: een map onder `CELLS_PATH`, geladen en gecontroleerd.
//!
//! Een cel legt vast, bewaart en reduceert (de positionpaper: "een ruimte
//! waarin chronolexogrammen worden gemaakt, bewaard en verwerkt"). Wie er
//! handelt, staat niet hier maar in een proces ([`crate::proces`]).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use regelrecht_engine::LawExecutionService;

use crate::config::CelDefinitie;
use crate::gram::Gram;
use crate::reductie::{self, Lexostatussen};
use crate::stroom::{self, Event, Stroom};
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
    /// De grammen voor een lege kroniek (leeg zonder `startstand`).
    pub startstand: Vec<Gram>,
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
        definitie: CelDefinitie,
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
        let Some(mut lexostatussen) = lexostatussen.filter(|_| fouten.is_empty()) else {
            return Err(fout(fouten));
        };
        fouten.extend(controle::perioden(&strommen, &mut lexostatussen, &service));
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
        if let Err(f) = controle::controleer(&strommen, &lexostatussen, &service) {
            fouten.extend(f);
        }
        let startstand = match &definitie.startstand {
            Some(pad) => startstand::laad(&map.join(pad), &strommen)
                .map_err(|f| fouten.extend(f))
                .unwrap_or_default(),
            None => Vec::new(),
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
            startstand,
        })
    }

    pub fn id(&self) -> &str {
        &self.definitie.id
    }

    /// Een event uit een stroom van de cel.
    pub fn event(&self, stroom: &str, event: &str) -> Option<(&Stroom, &Event)> {
        let s = self.strommen.iter().find(|s| s.id == stroom)?;
        Some((s, s.event(event)?))
    }

    /// De kronieken van de cel, gesorteerd en zonder dubbelen.
    pub fn kronieken(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self.strommen.iter().map(|s| s.chronicle.as_str()).collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    /// Of een event van de cel een zaak opent of volgt. Zo'n cel biedt de
    /// lexostatus [`crate::reductie::ZAAKSTAND`] aan.
    pub fn heeft_zaken(&self) -> bool {
        self.strommen
            .iter()
            .flat_map(|s| s.events.iter())
            .any(|e| e.zaak.heeft_kenmerk())
    }
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
            instantie
                .event("test_aanvragen", "aanvraag_ontvangen")
                .unwrap()
                .1
                .name,
            "aanvraag_ontvangen"
        );
        assert!(instantie.startstand.is_empty());

        let register = Cel::laad(&fixtures().join("cellen/register"), s.clone()).unwrap();
        assert_eq!(register.startstand.len(), 4);
        assert_eq!(register.kronieken(), ["test_register"]);

        let afnemer = Cel::laad(&fixtures().join("cellen/afnemer"), s).unwrap();
        assert_eq!(
            afnemer.kronieken(),
            ["test_afnemer"],
            "twee stromen, een kroniek"
        );
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
}
