//! A cell: a directory under `CELLS_PATH`, loaded and checked.
//!
//! A cell records, keeps and reduces (the position paper: "een ruimte
//! waarin chronolexogrammen worden gemaakt, bewaard en verwerkt"). Who
//! acts is not here but in a process ([`crate::process`]).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::NaiveDate;

use regelrecht_engine::LawExecutionService;

use crate::config::CellDefinition;
use crate::gram::Gram;
use crate::lexostatus_engine::CellRoute;
use crate::reduction::{self, Lexostatuses};
use crate::stream::{self, Event, Stream};
use crate::{check, initial_state, law};

/// A loaded cell that passed the checks at startup.
pub struct Cell {
    pub definition: CellDefinition,
    /// The directory of the cell; paths in `cell.yaml` are relative to it.
    pub dir: PathBuf,
    pub streams: Vec<Stream>,
    pub lexostatuses: Lexostatuses,
    /// The corpus, shared by all cells of the runtime.
    pub service: Arc<LawExecutionService>,
    /// The grams for an empty chronicle (empty without `initial_state`).
    pub initial_state: Vec<Gram>,
    /// The engine route of the cell (experiment A, `CELL_REDUCTION`); without
    /// it the cell reduces along the reduction DSL.
    pub route: Option<Arc<CellRoute>>,
    /// The day whose version of each regulation the cell was loaded with
    /// (the newest without one).
    pub date: Option<NaiveDate>,
}

impl Cell {
    /// Load a cell from its directory and check it. Every error is returned,
    /// not only the first, and every error names the cell.
    pub fn load(map: &Path, service: Arc<LawExecutionService>) -> Result<Self, Vec<String>> {
        Self::load_on(map, service, None)
    }

    /// Load a cell with the law as it applies on `date`: the shape of an
    /// event follows from the version of each regulation in force then (the
    /// newest without a date). The runtime loads on the day it starts.
    pub fn load_on(
        map: &Path,
        service: Arc<LawExecutionService>,
        date: Option<NaiveDate>,
    ) -> Result<Self, Vec<String>> {
        let name = map
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let definition = CellDefinition::load(map).map_err(|f| with_cell(&name, f))?;
        Self::load_definition(definition, map, service, date)
    }

    fn load_definition(
        definition: CellDefinition,
        map: &Path,
        service: Arc<LawExecutionService>,
        date: Option<NaiveDate>,
    ) -> Result<Self, Vec<String>> {
        let id = definition.id.clone();
        let error = |f: Vec<String>| with_cell(&id, f);
        let mut errors = Vec::new();
        let mut streams = Vec::new();
        for path in &definition.streams {
            match stream::load(&map.join(path)) {
                Ok(s) => streams.extend(s),
                Err(f) => errors.extend(f),
            }
        }
        // What the law says about the events (`establishes`), before anything
        // that reads the events.
        if errors.is_empty() {
            errors.extend(law::establish(&mut streams, &service, date));
        }
        // The roles of the events (decision, follows a decision, root) follow
        // from their stage and references; every reference must be able to
        // point at an event of the cell.
        if errors.is_empty() {
            stream::derive_roles(&mut streams);
            errors.extend(stream::check_references(&streams));
        }
        let lexostatuses = reduction::load(&map.join(&definition.lexostatuses))
            .map_err(|f| errors.extend(f))
            .ok();
        let Some(mut lexostatuses) = lexostatuses.filter(|_| errors.is_empty()) else {
            return Err(error(errors));
        };
        // The lexostatuses the law reads in this cell, next to the cell's own.
        match law::lexostatuses(&streams, &service, &lexostatuses.law) {
            Ok(from_law) => {
                for d in from_law {
                    if lexostatuses.lexostatus(&d.name).is_some() {
                        errors.push(format!("lexostatus '{}' is also in the law", d.name));
                    }
                    lexostatuses.lexostatus_definitions.push(d);
                }
            }
            Err(f) => errors.extend(f),
        }
        // The worklist the runtime offers for every cell with submissions
        // (RFC-047); the name is the runtime's.
        if lexostatuses.lexostatus(reduction::WORKLIST).is_some() {
            errors.push(format!(
                "lexostatus '{}': that name belongs to the runtime, which offers it for every cell with submissions",
                reduction::WORKLIST
            ));
        } else {
            match reduction::worklist(&streams, &service, date) {
                Ok(Some(d)) => lexostatuses.lexostatus_definitions.push(d),
                Ok(None) => {}
                Err(e) => errors.push(e),
            }
        }
        errors.extend(check::periods(&streams, &mut lexostatuses, &service));
        if lexostatuses.cell != definition.id {
            errors.push(format!(
                "{}: cell '{}' is not the id of this cell",
                definition.lexostatuses, lexostatuses.cell
            ));
        }
        for s in &streams {
            if s.recording_actor != definition.recording_actor {
                errors.push(format!(
                    "stream '{}' has recording_actor '{}', the cell '{}'",
                    s.id, s.recording_actor, definition.recording_actor
                ));
            }
        }
        if let Err(f) = check::check(&streams, &lexostatuses, &service) {
            errors.extend(f);
        }
        let initial_state = match &definition.initial_state {
            Some(path) => initial_state::load(&map.join(path), &streams)
                .map_err(|f| errors.extend(f))
                .unwrap_or_default(),
            None => Vec::new(),
        };
        if !errors.is_empty() {
            return Err(error(errors));
        }
        Ok(Self {
            definition,
            dir: map.to_path_buf(),
            streams,
            lexostatuses,
            service,
            initial_state,
            route: None,
            date,
        })
    }

    pub fn id(&self) -> &str {
        &self.definition.id
    }

    /// An event from a stream of the cell.
    pub fn event(&self, stream: &str, event: &str) -> Option<(&Stream, &Event)> {
        let s = self.streams.iter().find(|s| s.id == stream)?;
        Some((s, s.event(event)?))
    }

    /// The chronicles of the cell, sorted and without duplicates.
    pub fn chronicles(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self.streams.iter().map(|s| s.chronicle.as_str()).collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    /// Whether an event of the cell refers to another gram (or is a root
    /// another refers to). Such a cell offers the lexostatus
    /// [`crate::reduction::CASE_STATE`]: the state of the group around a
    /// root.
    pub fn has_cases(&self) -> bool {
        self.streams
            .iter()
            .flat_map(|s| s.events.iter())
            .any(|e| e.case.has_attribute())
    }
}

/// Put the cell in front of every message.
pub fn with_cell(cell: &str, errors: Vec<String>) -> Vec<String> {
    errors
        .into_iter()
        .map(|f| format!("cell '{cell}': {f}"))
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
            crate::regulations::load(&fixtures().join("regulation"))
                .unwrap()
                .service,
        )
    }

    #[test]
    fn fixture_cells_load() {
        let s = service();
        let agency = Cell::load(&fixtures().join("cells/instantie"), s.clone()).unwrap();
        assert_eq!(
            agency
                .event("test_aanvragen", "aanvraag_ontvangen")
                .unwrap()
                .1
                .name,
            "aanvraag_ontvangen"
        );
        assert!(agency.initial_state.is_empty());

        let register = Cell::load(&fixtures().join("cells/register"), s.clone()).unwrap();
        assert_eq!(register.initial_state.len(), 4);
        assert_eq!(register.chronicles(), ["test_register"]);

        let consumer = Cell::load(&fixtures().join("cells/afnemer"), s).unwrap();
        assert_eq!(
            consumer.chronicles(),
            ["test_afnemer"],
            "two streams, one chronicle"
        );
    }

    /// Copy a fixture cell and the streams to a temporary directory, so that
    /// a test can change a file in it.
    fn copy(cell: &str) -> tempfile::TempDir {
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
    fn every_message_names_the_cell() {
        let dir = copy("register");
        let map = dir.path().join("cells/register");
        let lexo = std::fs::read_to_string(map.join("lexostatuses.yaml")).unwrap();
        std::fs::write(
            map.join("lexostatuses.yaml"),
            lexo.replace("cell: test_register", "cell: ander_register"),
        )
        .unwrap();
        let errors = Cell::load(&map, service()).err().unwrap();
        assert!(!errors.is_empty());
        assert!(
            errors
                .iter()
                .all(|f| f.starts_with("cell 'test_register': ")),
            "{errors:?}"
        );
        assert!(errors
            .iter()
            .any(|f| f.contains("'ander_register' is not the id")));
    }

    /// The worklist is the runtime's (RFC-047): a cell that defines one
    /// itself does not start, also without submissions.
    #[test]
    fn the_worklist_name_belongs_to_the_runtime() {
        let dir = copy("afnemer");
        let map = dir.path().join("cells/afnemer");
        let lexo = std::fs::read_to_string(map.join("lexostatuses.yaml")).unwrap();
        std::fs::write(
            map.join("lexostatuses.yaml"),
            lexo.replace("  - name: werkvoorraad", "  - name: worklist"),
        )
        .unwrap();
        let errors = Cell::load(&map, service()).err().unwrap();
        assert!(
            errors
                .iter()
                .any(|f| f.contains("'worklist'") && f.contains("belongs to the runtime")),
            "{errors:?}"
        );
    }

    #[test]
    fn missing_files_are_all_reported() {
        let dir = copy("instantie");
        let map = dir.path().join("cells/instantie");
        std::fs::remove_file(map.join("lexostatuses.yaml")).unwrap();
        std::fs::remove_file(dir.path().join("chronicles/test_aanvragen.yaml")).unwrap();
        let errors = Cell::load(&map, service()).err().unwrap();
        assert_eq!(errors.len(), 2, "{errors:?}");
    }

    #[test]
    fn cell_without_cell_yaml_names_the_directory() {
        let dir = tempfile::tempdir().unwrap();
        let map = dir.path().join("leeg");
        std::fs::create_dir_all(&map).unwrap();
        let errors = Cell::load(&map, service()).err().unwrap();
        assert!(errors[0].starts_with("cell 'leeg': "), "{errors:?}");
    }

    #[test]
    fn stream_of_another_actor() {
        let dir = copy("register");
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
        let errors = Cell::load(&map, service()).err().unwrap();
        assert!(
            errors
                .iter()
                .any(|f| f.contains("recording_actor 'test_register'")),
            "{errors:?}"
        );
    }
}
