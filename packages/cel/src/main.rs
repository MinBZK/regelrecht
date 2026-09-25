//! De cel-runtime: laadt elke cel onder `CELLS_PATH` en elk proces onder
//! `PROCESSES_PATH`, en biedt ze aan onder `/cellen/<id>/api/` en
//! `/processen/<id>/api/`. Zie README.md voor de env-variabelen.

use regelrecht_cel::api::systeemklok;
use regelrecht_cel::config::Config;
use regelrecht_cel::runtime::Runtime;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    regelrecht_shared::telemetry::init_subscriber("info");

    let config = Config::from_env()?;
    // De controles bij het opstarten: faalt er een cel of een proces, dan
    // start de runtime niet, en elke fout wordt genoemd met de cel of het
    // proces erbij.
    // Laden leest het corpus en de kronieken van schijf: buiten de
    // async-draden.
    let c = config.clone();
    let geladen = tokio::task::spawn_blocking(move || Runtime::laad(&c, systeemklok())).await?;
    let runtime = match geladen {
        Ok(r) => r,
        Err(fouten) => {
            for f in &fouten {
                tracing::error!("{f}");
            }
            return Err(format!(
                "de runtime start niet: {} fout(en) bij de controle",
                fouten.len()
            )
            .into());
        }
    };
    for s in &runtime.cellen {
        tracing::info!(
            cel = %s.cel.id(),
            strommen = s.cel.strommen.len(),
            lexostatussen = s.cel.lexostatussen.lexostatus_definitions.len(),
            "cel gecontroleerd",
        );
    }
    for s in &runtime.processen {
        tracing::info!(
            proces = %s.proces.id(),
            cel = %s.proces.cel.id(),
            portaal = s.proces.portaal().is_some(),
            behandeling = s.proces.definitie.behandeling.is_some(),
            synthese = s.proces.definitie.synthese.len(),
            "proces gecontroleerd",
        );
    }
    tracing::info!(
        cellen = runtime.cellen.len(),
        processen = runtime.processen.len(),
        regelingen = runtime.cellen.first().map_or(0, |s| s.cel.service.law_count()),
        data_dir = %config.data_dir.display(),
        "runtime gecontroleerd",
    );

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", config.port)).await?;
    tracing::info!("luistert op 0.0.0.0:{}", config.port);
    let router = runtime.router.clone();
    let server = tokio::spawn(async move { axum::serve(listener, router).await });
    // Na het starten, zodat een bron in dezelfde runtime al antwoordt.
    for w in runtime.waarschuwingen().await {
        tracing::warn!("{w}");
    }
    server.await??;
    Ok(())
}
