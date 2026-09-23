//! De runtime: alle cellen onder `CELLS_PATH` in een proces.
//!
//! Elke cel krijgt haar eigen kroniek (`DATA_DIR/<id>/`) en haar eigen
//! routes (`/cellen/<id>/api/...`). `GET /api/cellen` zegt welke cellen er
//! zijn. Een cel ziet alleen haar eigen grammen; een andere cel bevraagt ze
//! via een [`Transport`](crate::transport::Transport), intern als de bron in
//! deze runtime draait en over HTTP als de synthese-bron een url heeft.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use axum::routing::get;
use axum::{Json, Router};
use serde_json::Value;

use crate::api::{self, AppState, Klok};
use crate::cel::{met_cel, Cel};
use crate::config::{celmappen, Config};
use crate::kroniek::Kroniek;
use crate::sessie::Sessies;
use crate::synthese::{self, Bron, TIJDSLIMIET};
use crate::transport::{Http, Intern, Transport};
use crate::{besluit, regelingen, rijen};

/// Een geladen runtime: de cellen en de router over allemaal.
pub struct Runtime {
    pub cellen: Vec<AppState>,
    pub router: Router,
}

impl Runtime {
    /// Laad het corpus en elke cel, controleer ze, open de kronieken (met de
    /// startstand als een kroniek leeg is) en bouw de router. Faalt een cel,
    /// dan start de runtime niet; elke melding noemt de cel.
    pub fn laad(config: &Config, klok: Klok) -> Result<Self, Vec<String>> {
        let corpus = regelingen::laad(&config.regulation_path)?;
        let service = Arc::new(corpus.service);
        let geladen = Arc::new(corpus.regelingen);
        let mappen = celmappen(&config.cells_path).map_err(|e| vec![e])?;
        let mut cellen = Vec::new();
        let mut fouten = Vec::new();
        for map in &mappen {
            match Cel::laad(map, service.clone()) {
                Ok(c) => cellen.push(c),
                Err(f) => fouten.extend(f),
            }
        }
        let mut ids = BTreeSet::new();
        for c in &cellen {
            if !ids.insert(c.id().to_string()) {
                fouten.push(format!(
                    "cel '{}': de id staat er meer dan een keer ({})",
                    c.id(),
                    c.map.display()
                ));
            }
            fouten.extend(met_cel(c.id(), synthese::controleer(c)));
            fouten.extend(met_cel(c.id(), besluit::controleer(c)));
        }
        if !fouten.is_empty() {
            return Err(fouten);
        }

        let slot: Arc<OnceLock<Router>> = Arc::new(OnceLock::new());
        let intern: Arc<dyn Transport> = Arc::new(Intern::new(slot.clone()));
        let mut staten = Vec::new();
        for cel in cellen {
            let kroniek =
                open_kroniek(&config.data_dir, &cel).map_err(|f| met_cel(cel.id(), vec![f]))?;
            let transport = |url: &Option<String>| -> Result<Arc<dyn Transport>, Vec<String>> {
                Ok(match url {
                    Some(url) => Arc::new(
                        Http::new(url, TIJDSLIMIET).map_err(|e| met_cel(cel.id(), vec![e]))?,
                    ),
                    None => intern.clone(),
                })
            };
            let mut bronnen = Vec::new();
            for b in &cel.definitie.synthese {
                bronnen.push(Bron {
                    definitie: b.clone(),
                    transport: transport(&b.url)?,
                });
            }
            let mut per_regel = Vec::new();
            for r in cel.rijen() {
                let mut rijbronnen = Vec::new();
                for b in &r.bronnen {
                    rijbronnen.push(rijen::Bron {
                        definitie: b.clone(),
                        transport: transport(&b.url)?,
                    });
                }
                per_regel.push(rijen::Rijen {
                    definitie: r.clone(),
                    bronnen: rijbronnen,
                });
            }
            staten.push(AppState {
                cel: Arc::new(cel),
                kroniek: Arc::new(kroniek),
                sessies: Arc::new(Sessies::default()),
                klok: klok.clone(),
                bronnen: Arc::new(bronnen),
                rijen: Arc::new(per_regel),
                regelingen: geladen.clone(),
            });
        }
        let router = bouw_router(&staten);
        // Pas nu bestaat de router, en daarmee het interne transport.
        let _ = slot.set(router.clone());
        Ok(Self {
            cellen: staten,
            router,
        })
    }

    /// De waarschuwingen over synthese-bronnen: onbereikbaar, of zonder de
    /// verwachte lexostatus of parameters. Geen reden om niet te starten.
    pub async fn waarschuwingen(&self) -> Vec<String> {
        let mut uit = Vec::new();
        for s in &self.cellen {
            for b in s.bronnen.iter() {
                // Een intern transport naar een cel die hier niet draait.
                if b.definitie.url.is_none()
                    && !self.cellen.iter().any(|c| c.cel.id() == b.definitie.cel)
                {
                    uit.push(format!(
                        "cel '{}': synthese-bron '{}' draait niet in deze runtime en heeft geen url; de toets meldt haar onbereikbaar",
                        s.cel.id(),
                        b.definitie.cel
                    ));
                }
            }
            uit.extend(synthese::waarschuwingen(s.cel.id(), &s.bronnen).await);
        }
        uit.dedup();
        uit
    }
}

/// Open de kroniek van een cel. Is elke kroniek van de cel leeg, dan komt de
/// startstand erin.
fn open_kroniek(data_dir: &Path, cel: &Cel) -> Result<Kroniek, String> {
    let kroniek = Kroniek::open(&data_dir.join(cel.id()))?;
    if cel.startstand.is_empty() {
        return Ok(kroniek);
    }
    let mut leeg = true;
    for k in cel.kronieken() {
        leeg &= kroniek.lees(k)?.is_empty();
    }
    if leeg {
        for gram in &cel.startstand {
            kroniek.voeg_toe(gram)?;
        }
        tracing::info!(cel = %cel.id(), grammen = cel.startstand.len(), "startstand in lege kroniek gezet");
    }
    Ok(kroniek)
}

/// `GET /api/cellen` en per cel haar routes onder `/cellen/<id>`.
fn bouw_router(staten: &[AppState]) -> Router {
    let lijst: Arc<Vec<Value>> = Arc::new(staten.iter().map(api::beschrijving).collect());
    let mut router = Router::new().route(
        "/api/cellen",
        get(move || {
            let lijst = lijst.clone();
            async move { Json(Value::Array(lijst.as_ref().clone())) }
        }),
    );
    for s in staten {
        router = router.nest(&format!("/cellen/{}", s.cel.id()), api::router(s.clone()));
    }
    router
}
