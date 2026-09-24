//! De runtime: alle cellen onder `CELLS_PATH` en alle processen onder
//! `PROCESSES_PATH`, in een programma.
//!
//! Elke cel krijgt haar eigen kroniek (`DATA_DIR/<id>/`) en haar eigen
//! routes (`/cellen/<id>/api/...`); elk proces zijn eigen routes
//! (`/processen/<id>/api/...`). `GET /api/cellen` en `GET /api/processen`
//! zeggen welke er zijn. Een cel ziet alleen haar eigen grammen. Een proces
//! ziet er geen: het bevraagt cellen via een
//! [`Transport`](crate::transport::Transport), intern als de cel in deze
//! runtime draait en over HTTP als de synthese-bron een url heeft. Ook de cel
//! waarin het proces vastlegt, bevraagt het zo.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::{Arc, OnceLock};

use axum::routing::get;
use axum::{Json, Router};
use serde_json::Value;

use crate::api::{self, CelState, Klok, ProcesState};
use crate::cel::{met_cel, Cel};
use crate::config::{celmappen, procesmappen, Config};
use crate::kroniek::Kroniek;
use crate::proces::{met_proces, Proces};
use crate::sessie::Sessies;
use crate::synthese::{self, Bron, TIJDSLIMIET};
use crate::transport::{Http, Intern, Transport};
use crate::{besluit, regelingen, rijen};

/// Een geladen runtime: de cellen, de processen en de router over allemaal.
pub struct Runtime {
    pub cellen: Vec<CelState>,
    pub processen: Vec<ProcesState>,
    pub router: Router,
}

impl Runtime {
    /// Laad het corpus, elke cel en elk proces, controleer ze, open de
    /// kronieken (met de startstand als een kroniek leeg is) en bouw de
    /// router. Faalt een cel of een proces, dan start de runtime niet; elke
    /// melding noemt de cel of het proces.
    pub fn laad(config: &Config, klok: Klok) -> Result<Self, Vec<String>> {
        let corpus = regelingen::laad(&config.regulation_path)?;
        let service = Arc::new(corpus.service);
        let geladen = Arc::new(corpus.regelingen);
        let mappen = celmappen(&config.cells_path).map_err(|e| vec![e])?;
        let mut cellen: Vec<Arc<Cel>> = Vec::new();
        let mut fouten = Vec::new();
        for map in &mappen {
            match Cel::laad(map, service.clone()) {
                Ok(c) => cellen.push(Arc::new(c)),
                Err(f) => fouten.extend(f),
            }
        }
        let mut per_id: BTreeMap<String, Arc<Cel>> = BTreeMap::new();
        for c in &cellen {
            if per_id.insert(c.id().to_string(), c.clone()).is_some() {
                fouten.push(format!(
                    "cel '{}': de id staat er meer dan een keer ({})",
                    c.id(),
                    c.map.display()
                ));
            }
        }

        let procesmappen = match &config.processes_path {
            Some(p) => procesmappen(p).map_err(|e| vec![e])?,
            None => Vec::new(),
        };
        let mut processen: Vec<Proces> = Vec::new();
        for map in &procesmappen {
            match Proces::laad(map, &per_id, service.clone()) {
                Ok(p) => processen.push(p),
                Err(f) => fouten.extend(f),
            }
        }
        let mut ids = BTreeSet::new();
        for p in &processen {
            if !ids.insert(p.id().to_string()) {
                fouten.push(format!(
                    "proces '{}': de id staat er meer dan een keer ({})",
                    p.id(),
                    p.map.display()
                ));
            }
            fouten.extend(met_proces(p.id(), synthese::controleer(p)));
            fouten.extend(met_proces(p.id(), besluit::controleer(p)));
        }
        if !fouten.is_empty() {
            return Err(fouten);
        }

        let mut celstaten = Vec::new();
        for cel in cellen {
            let kroniek =
                open_kroniek(&config.data_dir, &cel).map_err(|f| met_cel(cel.id(), vec![f]))?;
            celstaten.push(CelState {
                cel,
                kroniek: Arc::new(kroniek),
                klok: klok.clone(),
            });
        }

        let slot: Arc<OnceLock<Router>> = Arc::new(OnceLock::new());
        let intern: Arc<dyn Transport> = Arc::new(Intern::new(slot.clone()));
        let mut processtaten = Vec::new();
        for proces in processen {
            let id = proces.id().to_string();
            let transport = |url: &Option<String>| -> Result<Arc<dyn Transport>, Vec<String>> {
                Ok(match url {
                    Some(url) => {
                        Arc::new(Http::new(url, TIJDSLIMIET).map_err(|e| met_proces(&id, vec![e]))?)
                    }
                    None => intern.clone(),
                })
            };
            let mut bronnen = Vec::new();
            for b in proces.definitie.andere_bronnen() {
                bronnen.push(Bron {
                    definitie: b.clone(),
                    transport: transport(&b.url)?,
                });
            }
            let mut per_regel = Vec::new();
            for r in proces.rijen() {
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
            processtaten.push(ProcesState {
                proces: Arc::new(proces),
                // De cel waarin het proces vastlegt, draait in deze runtime.
                cel: intern.clone(),
                sessies: Arc::new(Sessies::default()),
                klok: klok.clone(),
                bronnen: Arc::new(bronnen),
                rijen: Arc::new(per_regel),
                regelingen: geladen.clone(),
            });
        }
        let router = bouw_router(&celstaten, &processtaten);
        // Pas nu bestaat de router, en daarmee het interne transport.
        let _ = slot.set(router.clone());
        Ok(Self {
            cellen: celstaten,
            processen: processtaten,
            router,
        })
    }

    /// De waarschuwingen over synthese-bronnen: onbereikbaar, of zonder de
    /// verwachte lexostatus of parameters. Geen reden om niet te starten.
    pub async fn waarschuwingen(&self) -> Vec<String> {
        let mut uit = Vec::new();
        for s in &self.processen {
            for b in s.bronnen.iter() {
                // Een intern transport naar een cel die hier niet draait.
                if b.definitie.url.is_none()
                    && !self.cellen.iter().any(|c| c.cel.id() == b.definitie.cel)
                {
                    uit.push(format!(
                        "proces '{}': synthese-bron '{}' draait niet in deze runtime en heeft geen url; de toets meldt haar onbereikbaar",
                        s.proces.id(),
                        b.definitie.cel
                    ));
                }
            }
            uit.extend(synthese::waarschuwingen(s.proces.id(), &s.bronnen).await);
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

/// Een lijst als route.
fn lijst_route(lijst: Vec<Value>) -> axum::routing::MethodRouter {
    let lijst = Arc::new(Value::Array(lijst));
    get(move || {
        let lijst = lijst.clone();
        async move { Json(lijst.as_ref().clone()) }
    })
}

/// `GET /api/cellen`, `GET /api/processen`, per cel haar routes onder
/// `/cellen/<id>` en per proces zijn routes onder `/processen/<id>`.
fn bouw_router(cellen: &[CelState], processen: &[ProcesState]) -> Router {
    let mut router = Router::new()
        .route(
            "/api/cellen",
            lijst_route(cellen.iter().map(api::cel_beschrijving).collect()),
        )
        .route(
            "/api/processen",
            lijst_route(processen.iter().map(api::proces_beschrijving).collect()),
        );
    for s in cellen {
        router = router.nest(
            &format!("/cellen/{}", s.cel.id()),
            api::cel_router(s.clone()),
        );
    }
    for s in processen {
        router = router.nest(
            &format!("/processen/{}", s.proces.id()),
            api::proces_router(s.clone()),
        );
    }
    router
}
