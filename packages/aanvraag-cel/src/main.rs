//! De aanvraag-cel. Zie README.md voor de vier lagen en de env-variabelen.

use std::sync::Arc;

use regelrecht_aanvraag_cel::api::{router, systeemklok, AppState};
use regelrecht_aanvraag_cel::config::{Cel, Config};
use regelrecht_aanvraag_cel::eherkenning::Sessies;
use regelrecht_aanvraag_cel::kroniek::Kroniek;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    regelrecht_shared::telemetry::init_subscriber("info");

    let config = Config::from_env()?;
    // De controles bij het opstarten: bij een fout start de cel niet, en elke
    // fout wordt genoemd.
    let cel = match Cel::laad(&config) {
        Ok(cel) => cel,
        Err(fouten) => {
            for f in &fouten {
                tracing::error!("{f}");
            }
            return Err(format!(
                "de cel start niet: {} fout(en) bij de controle",
                fouten.len()
            )
            .into());
        }
    };
    let kroniek = Kroniek::open(&config.data_dir)?;
    tracing::info!(
        strommen = cel.strommen.len(),
        lexostatussen = cel.config.lexostatus_definitions.len(),
        regelingen = cel.service.law_count(),
        data_dir = %config.data_dir.display(),
        "aanvraag-cel gecontroleerd",
    );

    let app = router(AppState {
        cel: Arc::new(cel),
        kroniek: Arc::new(kroniek),
        sessies: Arc::new(Sessies::default()),
        klok: systeemklok(),
    });
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", config.port)).await?;
    tracing::info!("luistert op 0.0.0.0:{}", config.port);
    axum::serve(listener, app).await?;
    Ok(())
}
