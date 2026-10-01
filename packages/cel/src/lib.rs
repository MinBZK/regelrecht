//! The cell runtime: cells that record facts as chronolexograms and reduce
//! them to a lexostatus, and processes that combine lexostatuses of cells,
//! run the engine and have a cell record.
//!
//! Cell and process are configuration, not code: a directory under
//! `CELLS_PATH` with a `cell.yaml`, and a directory under `PROCESSES_PATH` with
//! a `process.yaml` (see README.md). The lexogram (1) belongs to no one; a cell
//! has layers 2 through 4, each with its own file:
//!
//! 1. the lexogram: the regulations from `REGULATION_PATH`, unchanged and
//!    shared by all cells;
//! 2. the stream definition: which facts the cell records ([`stream`]);
//! 3. the reduction to lexostatus: how the chronicle feeds the parameters of
//!    an article ([`reduction`]);
//! 4. the gram itself, append-only in the chronicle ([`chronicle`]).
//!
//! The process ([`process`]) sits above them: it informs (the synthesis at
//! the consumer, [`synthesis`], and the assessment), concludes (the actions in
//! a case, such as the decision, [`action`]) and asks a cell to record, along
//! the same routes as any consumer ([`transport`]). The runtime
//! ([`runtime`]) runs both. Nothing here names a specific case.

pub mod action;
pub mod api;
pub mod assessment;
pub mod authority;
pub mod cell;
pub mod cell_client;
pub mod channel;
pub mod check;
pub mod chronicle;
pub mod config;
pub mod date;
pub mod engine_regulation;
pub mod examples;
pub mod form;
pub mod fragment;
pub mod gram;
pub mod initial_state;
pub mod law;
pub mod lexostatus_engine;
pub mod load;
pub mod origin;
pub mod possibility;
pub mod process;
pub mod reduction;
pub mod register;
pub mod regulations;
pub mod rows;
pub mod runtime;
pub mod schema;
pub mod session;
pub mod stream;
pub mod synthesis;
pub mod transport;
