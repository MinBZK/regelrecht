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
use crate::lexostatus_engine::CelRoute;
use crate::reductie::{self, Lexostatussen};
use crate::stroom::{self, Event, Stroom};
use crate::{controle, startstand, wet};

/// Een geladen cel die de controles bij het opstarten doorstond.
pub struct Cell {
    pub definitie: CelDefinitie,
    /// De map van de cel; paden in `cel.yaml` zijn hier relatief aan.
    pub map: PathBuf,
    pub streams: Vec<Stroom>,
    pub lexostatuses: Lexostatussen,
    /// Het corpus, gedeeld door alle cellen van de runtime.
    pub service: Arc<LawExecutionService>,
    /// De grammen voor een lege kroniek (leeg zonder `startstand`).
    pub initial_state: Vec<Gram>,
    /// De engine-route van de cel (experiment A, `CEL_REDUCTIE`); zonder
    /// reduceert de cel langs de reductie-DSL.
    pub route: Option<Arc<CelRoute>>,
}

impl Cell {
    /// Laad een cel uit haar map en controleer haar. Elke fout komt terug,
    /// niet alleen de eerste, en elke fout noemt de cel.
    pub fn laad(map: &Path, service: Arc<LawExecutionService>) -> Result<Self, Vec<String>> {
        let name = map
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let definitie = CelDefinitie::laad(map).map_err(|f| met_cel(&name, f))?;
        Self::laad_definitie(definitie, map, service)
    }

    fn laad_definitie(
        definitie: CelDefinitie,
        map: &Path,
        service: Arc<LawExecutionService>,
    ) -> Result<Self, Vec<String>> {
        let id = definitie.id.clone();
        let error = |f: Vec<String>| met_cel(&id, f);
        let mut fouten = Vec::new();
        let mut streams = Vec::new();
        for path in &definitie.streams {
            match stroom::laad(&map.join(path)) {
                Ok(s) => streams.extend(s),
                Err(f) => fouten.extend(f),
            }
        }
        // Wat de wet over de events zegt (`vestigt`), vóór alles wat de
        // events leest.
        if fouten.is_empty() {
            fouten.extend(wet::vestig(&mut streams, &service));
        }
        // De rollen van de events (besluit, volgt een besluit, wortel) volgen
        // uit hun stage en verwijzingen; elke verwijzing moet naar een event
        // van de cel kunnen wijzen.
        if fouten.is_empty() {
            stroom::leid_rollen_af(&mut streams);
            fouten.extend(stroom::controleer_verwijzingen(&streams));
        }
        let lexostatuses = reductie::laad(&map.join(&definitie.lexostatuses))
            .map_err(|f| fouten.extend(f))
            .ok();
        let Some(mut lexostatuses) = lexostatuses.filter(|_| fouten.is_empty()) else {
            return Err(error(fouten));
        };
        // De lexostatussen die de wet in deze cel leest, naast die van de cel.
        match wet::lexostatuses(&streams, &service, &lexostatuses.law) {
            Ok(from_law) => {
                for d in from_law {
                    if lexostatuses.lexostatus(&d.name).is_some() {
                        fouten.push(format!("lexostatus '{}' staat ook in de wet", d.name));
                    }
                    lexostatuses.lexostatus_definitions.push(d);
                }
            }
            Err(f) => fouten.extend(f),
        }
        fouten.extend(controle::perioden(&streams, &mut lexostatuses, &service));
        if lexostatuses.cell != definitie.id {
            fouten.push(format!(
                "{}: cel '{}' is niet de id van deze cel",
                definitie.lexostatuses, lexostatuses.cell
            ));
        }
        for s in &streams {
            if s.recording_actor != definitie.recording_actor {
                fouten.push(format!(
                    "stroom '{}' heeft recording_actor '{}', de cel '{}'",
                    s.id, s.recording_actor, definitie.recording_actor
                ));
            }
        }
        if let Err(f) = controle::controleer(&streams, &lexostatuses, &service) {
            fouten.extend(f);
        }
        let initial_state = match &definitie.initial_state {
            Some(path) => startstand::laad(&map.join(path), &streams)
                .map_err(|f| fouten.extend(f))
                .unwrap_or_default(),
            None => Vec::new(),
        };
        if !fouten.is_empty() {
            return Err(error(fouten));
        }
        Ok(Self {
            definitie,
            map: map.to_path_buf(),
            streams,
            lexostatuses,
            service,
            initial_state,
            route: None,
        })
    }

    pub fn id(&self) -> &str {
        &self.definitie.id
    }

    /// Een event uit een stroom van de cel.
    pub fn event(&self, stream: &str, event: &str) -> Option<(&Stroom, &Event)> {
        let s = self.streams.iter().find(|s| s.id == stream)?;
        Some((s, s.event(event)?))
    }

    /// De kronieken van de cel, gesorteerd en zonder dubbelen.
    pub fn chronicles(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self.streams.iter().map(|s| s.chronicle.as_str()).collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    /// Of een event van de cel naar een ander gram verwijst (of een wortel is
    /// waar een ander naar verwijst). Zo'n cel biedt de lexostatus
    /// [`crate::reductie::ZAAKSTAND`] aan: de stand van de groep rond een
    /// wortel.
    pub fn heeft_zaken(&self) -> bool {
        self.streams
            .iter()
            .flat_map(|s| s.events.iter())
            .any(|e| e.case.heeft_kenmerk())
    }
}

/// Zet de cel voor elke melding.
pub fn met_cel(cell: &str, fouten: Vec<String>) -> Vec<String> {
    fouten
        .into_iter()
        .map(|f| format!("cel '{cell}': {f}"))
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
        let instantie = Cell::laad(&fixtures().join("cells/instantie"), s.clone()).unwrap();
        assert_eq!(
            instantie
                .event("test_aanvragen", "aanvraag_ontvangen")
                .unwrap()
                .1
                .name,
            "aanvraag_ontvangen"
        );
        assert!(instantie.initial_state.is_empty());

        let register = Cell::laad(&fixtures().join("cells/register"), s.clone()).unwrap();
        assert_eq!(register.initial_state.len(), 4);
        assert_eq!(register.chronicles(), ["test_register"]);

        let afnemer = Cell::laad(&fixtures().join("cells/afnemer"), s).unwrap();
        assert_eq!(
            afnemer.chronicles(),
            ["test_afnemer"],
            "twee stromen, een kroniek"
        );
    }

    /// Kopieer een fixture-cel en de stromen naar een tijdelijke map, zodat
    /// een test er een bestand in kan veranderen.
    fn kopie(cell: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let f = fixtures();
        std::fs::create_dir_all(dir.path().join("cells").join(cell)).unwrap();
        std::fs::create_dir_all(dir.path().join("chronicles")).unwrap();
        for e in std::fs::read_dir(f.join("cells").join(cell)).unwrap() {
            let p = e.unwrap().path();
            std::fs::copy(
                &p,
                dir.path()
                    .join("cells")
                    .join(cell)
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
        let map = dir.path().join("cells/register");
        let lexo = std::fs::read_to_string(map.join("lexostatuses.yaml")).unwrap();
        std::fs::write(
            map.join("lexostatuses.yaml"),
            lexo.replace("cell: test_register", "cell: ander_register"),
        )
        .unwrap();
        let fouten = Cell::laad(&map, service()).err().unwrap();
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
        let map = dir.path().join("cells/instantie");
        std::fs::remove_file(map.join("lexostatuses.yaml")).unwrap();
        std::fs::remove_file(dir.path().join("chronicles/test_aanvragen.yaml")).unwrap();
        let fouten = Cell::laad(&map, service()).err().unwrap();
        assert_eq!(fouten.len(), 2, "{fouten:?}");
    }

    #[test]
    fn cel_zonder_cel_yaml_noemt_de_map() {
        let dir = tempfile::tempdir().unwrap();
        let map = dir.path().join("leeg");
        std::fs::create_dir_all(&map).unwrap();
        let fouten = Cell::laad(&map, service()).err().unwrap();
        assert!(fouten[0].starts_with("cel 'leeg': "), "{fouten:?}");
    }

    #[test]
    fn stroom_van_een_andere_actor() {
        let dir = kopie("register");
        let map = dir.path().join("cells/register");
        let cell = std::fs::read_to_string(map.join("cell.yaml")).unwrap();
        std::fs::write(
            map.join("cell.yaml"),
            cell.replace(
                "recording_actor: test_register",
                "recording_actor: iemand_anders",
            ),
        )
        .unwrap();
        let fouten = Cell::laad(&map, service()).err().unwrap();
        assert!(
            fouten
                .iter()
                .any(|f| f.contains("recording_actor 'test_register'")),
            "{fouten:?}"
        );
    }
}
