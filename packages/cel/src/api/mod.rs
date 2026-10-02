//! The routes of a cell and of a process. The runtime serves them under
//! `/cells/<id>` and `/processes/<id>` (see [`crate::runtime`]).
//!
//! A cell records, stores and reduces ([`cell`]). Every cell:
//!
//! | Route | Does |
//! |---|---|
//! | `GET /api/chronicle` | with the runtime or read token: the grams, each with YAML |
//! | `GET /api/cases/{root}` | with the runtime or read token: the grams of a case, each with YAML; 404 if the cell does not know the case |
//! | `GET /api/lexostatus/{name}?<input>=...` | with the runtime or read token: a reduction, with the inputs as query; the runtime offers `case_state` for every cell with a case |
//! | `POST /api/lexostatus/{name}/trial` | only with the runtime token: `{draft, inputs}`: builds the gram in memory and reduces the chronicle with that gram; records nothing |
//! | `POST /api/grams` | only with the runtime token: `{actor, stream, event, intake, external, refers_to?, decision?, root_grams?}`: builds the gram, gives it an id, validates it, checks the actor and the references and records it |
//! | `GET /api/stream` | the stream definitions of the cell, with their hash and, per event, the `explanation` (why the event looks this way) |
//!
//! A process acts: it informs, concludes and has a cell record
//! ([`process`]). Every process:
//!
//! | Route | Does |
//! |---|---|
//! | `GET /api/examples` | default data per action (`examples` in `process.yaml`), also without login |
//! | `GET /api/map` | the map: configuration, events, lexostatuses and the articles they touch, also without login |
//! | `GET /api/law/{regulation}/{article}` | the YAML block of the article in the version that applies today (the version the cell loaded), with file and lines; also without login |
//! | `GET /api/law/{regulation}` | the whole file of the regulation in that same version; also without login |
//! | `GET /api/config/{*config}?anchor=` | the YAML block of a loaded `process`, `form`, `stream/<id>`, `cell`, `lexostatuses`, `registers` or (process from policy) `channels`, `synthesis`, `examples`; also without login |
//!
//! A process with roles has the routes of its channels ([`session`]); every
//! channel is simulated and is listed under `channels` in `process.yaml`:
//!
//! | Route | Does |
//! |---|---|
//! | `POST /api/channels/{channel}/login` | the fields of the channel (and `role` if more than one role logs in through the channel) to a session |
//! | `GET /api/channels/{channel}/session` | who is logged in through this channel |
//! | `POST /api/channels/{channel}/logout` | end the session |
//! | `GET /api/session` | who is logged in, through any channel |
//!
//! Every other route belongs to a route group; a role names the groups it
//! may use (`roles.<role>.routes`). A process with a portal, for a role with
//! routes `portal` ([`portal`]):
//!
//! | Route | Does |
//! |---|---|
//! | `GET /api/form` | the fields of the application form, from the stream of the cell, with `why` per field and for the form (why it is there, why this value, what is not in it) and, when logged in, `supplied.trace_text` for a register value |
//! | `POST /api/application/assessment` | trial reduction in the cell, synthesis, synthesis per row, engine |
//! | `POST /api/application` | the cell records the gram |
//! | `GET /api/possibilities` | what the portal offers according to the policy, per window, with trace |
//!
//! With a role with routes `counter` ([`counter`]):
//!
//! | Route | Does |
//! |---|---|
//! | `POST /api/counter/application` | `{applicant, received_at, external}`: an application that came in another way, with the day of receipt |
//!
//! And with a `handling`, for a role with routes `handling`
//! ([`handling`]):
//!
//! | Route | Does |
//! |---|---|
//! | `GET /api/worklist` | the list lexostatus of the worklist, from the cell |
//! | `GET /api/inspection/{cell}/chronicle` | the chronicle of a cell the process reads ([`inspection`]) |
//! | `GET /api/inspection/{cell}/lexostatus/{name}?...` | a lexostatus of such a cell |
//! | `GET /api/cases/{root}` | the grams of the case, the procedure, the legal protection, and per action its form, whether it can be taken, and a trial without form |
//! | `POST /api/cases/{root}/actions/{name}/trial` | `{form}` to an action on trial; nothing is recorded |
//! | `POST /api/cases/{root}/actions/{name}` | `{form, happened?}`: take the action, or report a fact that happened; the cell records it |
//!
//! There is no security context between process and cell. Only a process of
//! this runtime may record and reduce on trial: the internal transport
//! sends the runtime token along ([`crate::transport::RuntimeToken`]); without
//! a token 401, with a different one 403. Reading (chronicle, case, lexostatus)
//! asks for that same token or the read token shared by runtimes that may
//! read each other (`CELL_READ_TOKEN`), because a gram carries the identity
//! and the intake of whoever submitted. Only the stream definitions are open.

use std::sync::Arc;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::{DateTime, FixedOffset};
use serde_json::json;

use crate::transport::TransportError;

pub mod cell;
pub mod counter;
pub mod fragment;
pub mod handling;
pub mod inspection;
pub mod portal;
pub mod process;
pub mod session;

pub use cell::{as_yaml, cell_description, cell_router, CellState};
pub use process::{process_description, process_router, ActionState, ProcessState};

/// Supplies the moment at which something is made a fact.
pub type Clock = Arc<dyn Fn() -> DateTime<FixedOffset> + Send + Sync>;

/// The clock of the runtime: now, in Dutch time.
pub fn system_clock() -> Clock {
    Arc::new(|| {
        chrono::Utc::now()
            .with_timezone(&chrono_tz::Europe::Amsterdam)
            .fixed_offset()
    })
}

/// An error as `{"error": "..."}` with a status.
pub struct Error(StatusCode, String);

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"error": self.1}))).into_response()
    }
}

fn error(status: StatusCode, text: impl Into<String>) -> Error {
    Error(status, text.into())
}

/// An error of the runtime itself (500).
fn internal(text: impl Into<String>) -> Error {
    error(StatusCode::INTERNAL_SERVER_ERROR, text)
}

/// An error of the cell as the process's response: the same status and
/// the same text. A cell that does not respond, or responds unreadably, is an
/// error of the runtime.
fn of_cell(f: TransportError) -> Error {
    match f {
        TransportError::Response {
            status,
            error: text,
        } => Error(
            StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            text,
        ),
        TransportError::Unreachable(r) => internal(format!("the cell is unreachable: {r}")),
        TransportError::Json(r) => internal(format!("the cell responded unreadably: {r}")),
    }
}
