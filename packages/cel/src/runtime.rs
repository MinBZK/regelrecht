//! The runtime: all cells under `CELLS_PATH` and all processes, in one
//! program. With `CELL_CHANNELS` the processes follow from the policy and the
//! deployment (RFC-047); otherwise they are the directories under
//! `PROCESSES_PATH`.
//!
//! Every cell gets its own chronicle (`DATA_DIR/<id>/`) and its own
//! routes (`/cells/<id>/api/...`); every process its own routes
//! (`/processes/<id>/api/...`). `GET /api/cells` and `GET /api/processes`
//! say which ones exist. A cell sees only its own grams. A process
//! sees none: it queries cells through a
//! [`Transport`](crate::transport::Transport), internally if the cell runs in
//! this runtime and over HTTP if the synthesis source has a url. It queries
//! the cell in which the process records the same way.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::{Arc, OnceLock};

use axum::routing::get;
use axum::{Json, Router};
use serde_json::Value;

use crate::api::{self, ActionState, CellState, Clock, ProcessState};
use crate::cell::{with_cell, Cell};
use crate::chronicle::Chronicle;
use crate::config::{cell_dirs, process_dirs, Config, ReductionMode, RowsDefinition};
use crate::process::{with_process, Process};
use crate::session::Sessions;
use crate::synthesis::{self, Source, TIME_LIMIT};
use crate::transport::{Http, Internal, ReadToken, RuntimeToken, Transport};
use crate::{action, initial_state, lexostatus_engine, reduction, regulations, rows};

/// A loaded runtime: the cells, the processes and the router over all of them.
pub struct Runtime {
    pub cells: Vec<CellState>,
    pub processes: Vec<ProcessState>,
    pub router: Router,
    /// The token with which the processes of this runtime record; new at
    /// every start (see [`RuntimeToken`]).
    pub runtime_token: RuntimeToken,
}

impl Runtime {
    /// Load the corpus, every cell and every process, check them, open the
    /// chronicles (with the initial state if a chronicle is empty) and build
    /// the router. If a cell or a process fails, the runtime does not start;
    /// every message names the cell or the process.
    pub fn load(config: &Config, clock: Clock) -> Result<Self, Vec<String>> {
        let mut corpus = regulations::load(&config.regulation_path)?;
        // The registers the policy queries, as a source in the corpus
        // (note on source and gram id); their chronicles are added when the
        // cells open them.
        let registers = crate::register::load(
            config.registers.as_deref(),
            &mut corpus.service,
            &crate::date::reference_date(&clock()),
        )?;
        let register_links = Arc::new(registers.links());
        let registers_file = config.registers.clone().map(Arc::new);
        let service = Arc::new(corpus.service);
        // RFC-047: what a policy executes is a startup check, also without
        // a policy-based process.
        let mut errors: Vec<String> = crate::policy::check_executes(&service);
        let loaded = Arc::new(corpus.regulations);
        let regulation_files = Arc::new(corpus.files);
        // A relative `REGULATION_PATH` of one component has an empty parent.
        let root = Arc::new(
            config
                .regulation_path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new("."))
                .to_path_buf(),
        );
        let dirs = cell_dirs(&config.cells_path).map_err(|e| vec![e])?;
        // The shape of an event follows from the law as it applies today.
        let today = clock().date_naive();
        let mut loaded_cells: Vec<Cell> = Vec::new();
        for map in &dirs {
            match Cell::load_on(map, service.clone(), Some(today)) {
                Ok(c) => loaded_cells.push(c),
                Err(f) => errors.extend(f),
            }
        }
        // The engine route (experiment A): every lexostatus of every cell has
        // a binding, or the runtime does not start.
        if let (ReductionMode::Engine { binding, compare }, true) =
            (&config.reduction, errors.is_empty())
        {
            let per_cell: Vec<(&str, &reduction::Lexostatuses)> = loaded_cells
                .iter()
                .map(|c| (c.id(), &c.lexostatuses))
                .collect();
            match lexostatus_engine::load_binding(binding, *compare, &per_cell, &service) {
                Ok(mut routes) => {
                    for c in &mut loaded_cells {
                        c.route = routes.remove(c.id()).map(Arc::new);
                    }
                }
                Err(f) => errors.extend(f),
            }
        }
        let per_chronicle: Vec<(&str, Vec<&str>)> = loaded_cells
            .iter()
            .map(|c| (c.id(), c.chronicles()))
            .collect();
        errors.extend(registers.check(&per_chronicle));
        let cells: Vec<Arc<Cell>> = loaded_cells.into_iter().map(Arc::new).collect();
        // A field the law declares and nothing reads: not the cell, nor a
        // register that queries its chronicle. A warning, not a reason not
        // to start (note "het gram uit de wet").
        for c in &cells {
            for u in crate::check::unread_law_fields(&c.streams, &c.lexostatuses) {
                let by_register = crate::register::read_by(
                    &register_links,
                    &service,
                    c.id(),
                    &u.chronicle,
                    &u.event,
                    &u.path,
                );
                if by_register.is_none() {
                    tracing::warn!(
                        cell = %c.id(), field = %u.path, event = %u.event, stream = %u.stream,
                        "field with a legal basis that no derivation or register reads"
                    );
                }
            }
        }
        let mut per_id: BTreeMap<String, Arc<Cell>> = BTreeMap::new();
        for c in &cells {
            if per_id.insert(c.id().to_string(), c.clone()).is_some() {
                errors.push(format!(
                    "cell '{}': the id occurs more than once ({})",
                    c.id(),
                    c.dir.display()
                ));
            }
        }

        let mut processes: Vec<Process> = Vec::new();
        let deployment = match crate::deployment::load(config) {
            Ok(d) => d,
            Err(f) => {
                errors.extend(f);
                None
            }
        };
        if let Some(d) = &deployment {
            // RFC-047: the processes follow from the policy.
            match crate::policy::read(&service, Some(today))
                .and_then(|p| crate::derive::processes(&p, d, &per_id, &service, &root))
            {
                Ok(derived) => {
                    for x in derived {
                        match Process::from_derived(x, &root, &per_id, service.clone()) {
                            Ok(p) => processes.push(p),
                            Err(f) => errors.extend(f),
                        }
                    }
                }
                Err(f) => errors.extend(f),
            }
        } else {
            let process_dirs = match &config.processes_path {
                Some(p) => process_dirs(p).map_err(|e| vec![e])?,
                None => Vec::new(),
            };
            for map in &process_dirs {
                match Process::load(map, &per_id, service.clone()) {
                    Ok(p) => processes.push(p),
                    Err(f) => errors.extend(f),
                }
            }
        }
        let mut ids = BTreeSet::new();
        for p in &mut processes {
            if !ids.insert(p.id().to_string()) {
                errors.push(format!(
                    "process '{}': the id occurs more than once ({})",
                    p.id(),
                    p.dir.display()
                ));
            }
            // Provenance first: the form of every action follows from
            // it. Its errors only count once synthesis and actions are
            // correct.
            let provenance = p.check_provenance(&per_id);
            let mut own = synthesis::check(p);
            own.extend(action::check(p));
            if own.is_empty() {
                own = provenance;
            }
            errors.extend(with_process(p.id(), own));
        }
        if !errors.is_empty() {
            return Err(errors);
        }

        let runtime_token = RuntimeToken::generate();
        let read_token = config.read_token.as_deref().map(ReadToken::out);
        let mut cell_states = Vec::new();
        for cell in cells {
            let chronicle = Arc::new(
                open_chronicle(&config.data_dir, &cell, &clock)
                    .map_err(|f| with_cell(cell.id(), vec![f]))?,
            );
            registers.open(cell.id(), &chronicle);
            cell_states.push(CellState {
                cell,
                chronicle,
                clock: clock.clone(),
                runtime_token: runtime_token.clone(),
                read_token: read_token.clone(),
            });
        }

        let lock: Arc<OnceLock<Router>> = Arc::new(OnceLock::new());
        let internal: Arc<dyn Transport> =
            Arc::new(Internal::new(lock.clone(), runtime_token.clone()));
        let mut process_states = Vec::new();
        for process in processes {
            let id = process.id().to_string();
            let transport = |url: &Option<String>| -> Result<Arc<dyn Transport>, Vec<String>> {
                Ok(match url {
                    Some(url) => Arc::new(
                        Http::new(url, TIME_LIMIT)
                            .map_err(|e| with_process(&id, vec![e]))?
                            .with_read_token(read_token.clone().filter(|_| {
                                // Only a runtime that shares the token gets
                                // it: it grants reading in this runtime.
                                config
                                    .read_token_sources
                                    .iter()
                                    .any(|b| b == url.trim_end_matches('/'))
                            })),
                    ),
                    None => internal.clone(),
                })
            };
            let mut sources = Vec::new();
            for b in process.definition.other_sources() {
                sources.push(Source {
                    definition: b.clone(),
                    // The engine computes the consumer's own policy (note on
                    // source and gram id); the process queries a cell.
                    transport: match &b.regulation {
                        Some(_) => Arc::new(synthesis::PolicySource::new(service.clone(), b)),
                        None => transport(&b.url)?,
                    },
                });
            }
            let per_row = |defs: &[RowsDefinition]| -> Result<Vec<rows::Rows>, Vec<String>> {
                let mut out = Vec::new();
                for r in defs {
                    let mut row_sources = Vec::new();
                    for b in &r.sources {
                        row_sources.push(rows::Source {
                            definition: b.clone(),
                            transport: transport(&b.url)?,
                        });
                    }
                    out.push(rows::Rows {
                        definition: r.clone(),
                        sources: row_sources,
                    });
                }
                Ok(out)
            };
            // Per action the sources its article asks for and its
            // synthesis per row.
            let mut actions = Vec::new();
            for h in process.actions() {
                let pick = action::sources_for(&process, h);
                actions.push(ActionState {
                    sources: pick.iter().map(|i| sources[*i].clone()).collect(),
                    rows: per_row(&h.rows)?,
                });
            }
            let assessment_rows = per_row(process.assessment_rows())?;
            process_states.push(ProcessState {
                process: Arc::new(process),
                // The cell in which the process records runs in this runtime.
                cell: internal.clone(),
                sessions: Arc::new(Sessions::default()),
                clock: clock.clone(),
                sources: Arc::new(sources),
                actions: Arc::new(actions),
                assessment_rows: Arc::new(assessment_rows),
                regulations: loaded.clone(),
                root: root.clone(),
                regulation_files: regulation_files.clone(),
                register_links: register_links.clone(),
                registers_file: registers_file.clone(),
                channels_file: deployment
                    .as_ref()
                    .map(|d| Arc::new(d.channels_file.clone())),
                synthesis_file: deployment
                    .as_ref()
                    .and_then(|d| d.synthesis_file.clone())
                    .map(Arc::new),
                examples_file: deployment
                    .as_ref()
                    .and_then(|d| d.examples_file.clone())
                    .map(Arc::new),
            });
        }
        let router = build_router(&cell_states, &process_states);
        // Only now does the router exist, and with it the internal transport.
        let _ = lock.set(router.clone());
        Ok(Self {
            cells: cell_states,
            processes: process_states,
            router,
            runtime_token,
        })
    }

    /// The warnings about synthesis sources: unreachable, or without the
    /// expected lexostatus or parameters. No reason not to start.
    pub async fn warnings(&self) -> Vec<String> {
        let mut out = Vec::new();
        for s in &self.processes {
            out.extend(with_process(s.process.id(), s.process.warnings.clone()));
            for b in s.sources.iter() {
                // An internal transport to a cell that does not run here.
                if b.definition.url.is_none()
                    && b.definition.regulation.is_none()
                    && !self.cells.iter().any(|c| c.cell.id() == b.definition.cell)
                {
                    out.push(format!(
                        "process '{}': synthesis source '{}' does not run in this runtime and has no url; the assessment reports it unreachable",
                        s.process.id(),
                        b.definition.cell
                    ));
                }
            }
            out.extend(synthesis::warnings(s.process.id(), &s.sources).await);
        }
        out.dedup();
        out
    }
}

/// Open the chronicle of a cell. If every chronicle of the cell is empty, the
/// initial state goes in, with the load time as `recorded_at`.
fn open_chronicle(data_dir: &Path, cell: &Cell, clock: &Clock) -> Result<Chronicle, String> {
    let chronicle = Chronicle::open(&data_dir.join(cell.id()), &cell.chronicles())?;
    if !cell.initial_state.is_empty()
        && chronicle.set_initial_state(
            &cell.chronicles(),
            &initial_state::placed(&cell.initial_state, &clock())?,
        )?
    {
        tracing::info!(cell = %cell.id(), grams = cell.initial_state.len(), "initial state placed in empty chronicle");
    }
    Ok(chronicle)
}

/// A list as a route.
fn list_route(list: Vec<Value>) -> axum::routing::MethodRouter {
    let list = Arc::new(Value::Array(list));
    get(move || {
        let list = list.clone();
        async move { Json(list.as_ref().clone()) }
    })
}

/// `GET /api/cells`, `GET /api/processes`, per cell its routes under
/// `/cells/<id>` and per process its routes under `/processes/<id>`.
fn build_router(cells: &[CellState], processes: &[ProcessState]) -> Router {
    let mut router = Router::new()
        .route(
            "/api/cells",
            list_route(cells.iter().map(api::cell_description).collect()),
        )
        .route(
            "/api/processes",
            list_route(processes.iter().map(api::process_description).collect()),
        );
    for s in cells {
        router = router.nest(
            &format!("/cells/{}", s.cell.id()),
            api::cell_router(s.clone()),
        );
    }
    for s in processes {
        router = router.nest(
            &format!("/processes/{}", s.process.id()),
            api::process_router(s.clone()),
        );
    }
    router
}
