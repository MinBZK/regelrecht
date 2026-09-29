//! The cell runtime: loads every cell under `CELLS_PATH` and every process
//! under `PROCESSES_PATH`, and serves them under `/cells/<id>/api/` and
//! `/processes/<id>/api/`. See README.md for the env variables.

use regelrecht_cel::api::system_clock;
use regelrecht_cel::config::Config;
use regelrecht_cel::runtime::Runtime;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    regelrecht_shared::telemetry::init_subscriber("info");

    let config = Config::from_env()?;
    // The checks at startup: if a cell or a process fails, the runtime
    // does not start, and every error is named with its cell or process.
    // Loading reads the corpus and the chronicles from disk: outside the
    // async threads.
    let c = config.clone();
    let loaded = tokio::task::spawn_blocking(move || Runtime::load(&c, system_clock())).await?;
    let runtime = match loaded {
        Ok(r) => r,
        Err(errors) => {
            for f in &errors {
                tracing::error!("{f}");
            }
            return Err(format!(
                "the runtime does not start: {} error(s) in the check",
                errors.len()
            )
            .into());
        }
    };
    for s in &runtime.cells {
        tracing::info!(
            cell = %s.cell.id(),
            streams = s.cell.streams.len(),
            lexostatuses = s.cell.lexostatuses.lexostatus_definitions.len(),
            "cell checked",
        );
    }
    for s in &runtime.processes {
        tracing::info!(
            process = %s.process.id(),
            cell = %s.process.cell.id(),
            portal = s.process.portal().is_some(),
            handling = s.process.definition.handling.is_some(),
            synthesis = s.process.definition.synthesis.len(),
            "process checked",
        );
    }
    tracing::info!(
        cells = runtime.cells.len(),
        processes = runtime.processes.len(),
        regulations = runtime.cells.first().map_or(0, |s| s.cell.service.law_count()),
        data_dir = %config.data_dir.display(),
        "runtime checked",
    );

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", config.port)).await?;
    tracing::info!("listening on 0.0.0.0:{}", config.port);
    let router = runtime.router.clone();
    let server = tokio::spawn(async move { axum::serve(listener, router).await });
    // After starting, so that a source in the same runtime already responds.
    for w in runtime.warnings().await {
        tracing::warn!("{w}");
    }
    server.await??;
    Ok(())
}
