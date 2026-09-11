//! Het proces: configuratie lezen, wereld ophalen, luisteren.
//!
//! De vorm van `packages/admin` — telemetry, OIDC-discovery, binden op
//! `0.0.0.0`, netjes afsluiten op SIGTERM — zonder iets van zijn database. Wat
//! er hier bij komt is het ophalen van de wereld, en dat gebeurt vóór de
//! listener opengaat: mislukt het, dan is er niets om te bedienen en stopt het
//! proces met de reden.

use std::net::SocketAddr;
use std::sync::Arc;

use regelrecht_chrono_poc_web::config::{AppConfig, SESSION_TTL, SWEEP_INTERVAL};
use regelrecht_chrono_poc_web::state::AppState;
use regelrecht_chrono_poc_web::worlds::WorldRegistry;
use regelrecht_chrono_poc_web::{router, sources};

#[tokio::main]
async fn main() {
    regelrecht_shared::telemetry::init_subscriber("info");
    regelrecht_auth::install_crypto_provider();

    let config = AppConfig::from_env();

    // Vóór de OIDC-discovery: dit is de stap die het vaakst misgaat (een token
    // dat er niet is, een ref die niet bestaat), en dan hoort de melding daarover
    // te gaan en niet over een IdP.
    let resolved = match sources::resolve(
        &config.world,
        config.corpus.as_ref(),
        config.auth_ref.as_deref(),
    )
    .await
    {
        Ok(resolved) => resolved,
        Err(e) => {
            tracing::error!("kon de wereld niet klaarzetten: {e}");
            std::process::exit(1);
        }
    };
    let sources::Resolved {
        definition,
        regulation_root,
        // Blijft leven tot `main` eindigt: dit houdt een opgehaald corpus op
        // schijf. Zie de doc-comment op het veld.
        fetched: _fetched,
    } = resolved;
    tracing::info!(
        cells = definition.cells.len(),
        regulation_root = %regulation_root.display(),
        "wereldbestand gelezen"
    );

    let (oidc_client, end_session_url) = match config.oidc.as_ref() {
        Some(oidc) => match regelrecht_auth::discover_client(oidc).await {
            Ok(result) => (Some(Arc::new(result.client)), result.end_session_url),
            Err(e) => {
                tracing::error!(error = %e, "OIDC-discovery mislukt");
                std::process::exit(1);
            }
        },
        None => (None, None),
    };

    let http_client = match reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(client) => client,
        Err(e) => {
            tracing::error!(error = %e, "kon geen HTTP-client bouwen");
            std::process::exit(1);
        }
    };

    let port = config.port;
    let worlds = Arc::new(WorldRegistry::new(definition, regulation_root, SESSION_TTL));
    let state = AppState {
        config: Arc::new(config),
        worlds: Arc::clone(&worlds),
        oidc_client,
        end_session_url,
        http_client,
    };

    // De ruimer: zonder hem houdt elke bezoeker die ooit langskwam een thread en
    // een wereld bezet tot het proces stopt.
    let sweeper = Arc::clone(&worlds);
    tokio::spawn(async move {
        let mut ticks = tokio::time::interval(SWEEP_INTERVAL);
        loop {
            ticks.tick().await;
            let removed = sweeper.sweep();
            if removed > 0 {
                tracing::info!(
                    removed,
                    active = sweeper.len(),
                    "verlopen werelden opgeruimd"
                );
            }
        }
    });

    let app = router(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => listener,
        Err(e) => {
            tracing::error!(error = %e, "kon niet binden op {addr}");
            std::process::exit(1);
        }
    };
    tracing::info!("luistert op {addr}");

    let shutdown = async {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm = match signal(SignalKind::terminate()) {
            Ok(sigterm) => sigterm,
            Err(e) => {
                tracing::error!(error = %e, "kon geen SIGTERM-handler installeren");
                std::process::exit(1);
            }
        };
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = sigterm.recv() => {}
        }
        tracing::info!("afsluitsignaal ontvangen, verbindingen aflaten");
    };

    if let Err(e) = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await
    {
        tracing::error!(error = %e, "serverfout");
        std::process::exit(1);
    }
}
