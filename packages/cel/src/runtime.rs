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

use crate::api::{self, CelState, HandelingState, Klok, ProcesState};
use crate::cel::{met_cel, Cell};
use crate::config::{celmappen, procesmappen, Config, Reductiemodus, RijenDefinitie};
use crate::kroniek::Kroniek;
use crate::proces::{met_proces, Proces};
use crate::sessie::Sessies;
use crate::synthese::{self, Bron, TIJDSLIMIET};
use crate::transport::{Http, Intern, LeesToken, RuntimeToken, Transport};
use crate::{handeling, lexostatus_engine, reductie, regelingen, rijen, startstand};

/// Een geladen runtime: de cellen, de processen en de router over allemaal.
pub struct Runtime {
    pub cells: Vec<CelState>,
    pub processen: Vec<ProcesState>,
    pub router: Router,
    /// Het token waarmee de processen van deze runtime vastleggen; bij elke
    /// start nieuw (zie [`RuntimeToken`]).
    pub runtime_token: RuntimeToken,
}

impl Runtime {
    /// Laad het corpus, elke cel en elk proces, controleer ze, open de
    /// kronieken (met de startstand als een kroniek leeg is) en bouw de
    /// router. Faalt een cel of een proces, dan start de runtime niet; elke
    /// melding noemt de cel of het proces.
    pub fn laad(config: &Config, klok: Klok) -> Result<Self, Vec<String>> {
        let mut corpus = regelingen::laad(&config.regulation_path)?;
        // De registers die het beleid bevraagt, als bron in het corpus
        // (notitie bron en gram-id); hun kronieken komen erbij als de cellen
        // ze openen.
        let registers = crate::register::laad(
            config.registers.as_deref(),
            &mut corpus.service,
            &crate::datum::reference_date(&klok()),
        )?;
        let service = Arc::new(corpus.service);
        let geladen = Arc::new(corpus.regulations);
        let mappen = celmappen(&config.cells_path).map_err(|e| vec![e])?;
        let mut geladen_cellen: Vec<Cell> = Vec::new();
        let mut fouten = Vec::new();
        for map in &mappen {
            match Cell::laad(map, service.clone()) {
                Ok(c) => geladen_cellen.push(c),
                Err(f) => fouten.extend(f),
            }
        }
        // De engine-route (experiment A): elke lexostatus van elke cel heeft
        // een koppeling, of de runtime start niet.
        if let (
            Reductiemodus::Engine {
                koppeling,
                vergelijk,
            },
            true,
        ) = (&config.reduction, fouten.is_empty())
        {
            let per_cel: Vec<(&str, &reductie::Lexostatussen)> = geladen_cellen
                .iter()
                .map(|c| (c.id(), &c.lexostatuses))
                .collect();
            match lexostatus_engine::laad_koppeling(koppeling, *vergelijk, &per_cel, &service) {
                Ok(mut routes) => {
                    for c in &mut geladen_cellen {
                        c.route = routes.remove(c.id()).map(Arc::new);
                    }
                }
                Err(f) => fouten.extend(f),
            }
        }
        let per_kroniek: Vec<(&str, Vec<&str>)> = geladen_cellen
            .iter()
            .map(|c| (c.id(), c.chronicles()))
            .collect();
        fouten.extend(registers.controleer(&per_kroniek));
        let cells: Vec<Arc<Cell>> = geladen_cellen.into_iter().map(Arc::new).collect();
        let mut per_id: BTreeMap<String, Arc<Cell>> = BTreeMap::new();
        for c in &cells {
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
        for p in &mut processen {
            if !ids.insert(p.id().to_string()) {
                fouten.push(format!(
                    "proces '{}': de id staat er meer dan een keer ({})",
                    p.id(),
                    p.map.display()
                ));
            }
            // Eerst de herkomst: daaruit volgt het formulier van elke
            // handeling. Haar fouten tellen pas als synthese en handelingen
            // kloppen.
            let provenance = p.controleer_herkomst(&per_id);
            let mut eigen = synthese::controleer(p);
            eigen.extend(handeling::controleer(p));
            if eigen.is_empty() {
                eigen = provenance;
            }
            fouten.extend(met_proces(p.id(), eigen));
        }
        if !fouten.is_empty() {
            return Err(fouten);
        }

        let runtime_token = RuntimeToken::nieuw();
        let lees_token = config.lees_token.as_deref().map(LeesToken::uit);
        let mut celstaten = Vec::new();
        for cell in cells {
            let chronicle = Arc::new(
                open_kroniek(&config.data_dir, &cell, &klok)
                    .map_err(|f| met_cel(cell.id(), vec![f]))?,
            );
            registers.open(cell.id(), &chronicle);
            celstaten.push(CelState {
                cell,
                chronicle,
                klok: klok.clone(),
                runtime_token: runtime_token.clone(),
                lees_token: lees_token.clone(),
            });
        }

        let slot: Arc<OnceLock<Router>> = Arc::new(OnceLock::new());
        let intern: Arc<dyn Transport> = Arc::new(Intern::new(slot.clone(), runtime_token.clone()));
        let mut processtaten = Vec::new();
        for proces in processen {
            let id = proces.id().to_string();
            let transport = |url: &Option<String>| -> Result<Arc<dyn Transport>, Vec<String>> {
                Ok(match url {
                    Some(url) => Arc::new(
                        Http::new(url, TIJDSLIMIET)
                            .map_err(|e| met_proces(&id, vec![e]))?
                            .met_lees_token(lees_token.clone().filter(|_| {
                                // Alleen een runtime die het token deelt,
                                // krijgt het: het geeft lezen in deze runtime.
                                config
                                    .lees_token_bronnen
                                    .iter()
                                    .any(|b| b == url.trim_end_matches('/'))
                            })),
                    ),
                    None => intern.clone(),
                })
            };
            let mut sources = Vec::new();
            for b in proces.definitie.andere_bronnen() {
                sources.push(Bron {
                    definitie: b.clone(),
                    // Het eigen beleid van de afnemer rekent de engine uit
                    // (notitie bron en gram-id); een cel vraagt het proces.
                    transport: match &b.regulation {
                        Some(_) => Arc::new(synthese::Beleidsbron::nieuw(service.clone(), b)),
                        None => transport(&b.url)?,
                    },
                });
            }
            let per_regel = |defs: &[RijenDefinitie]| -> Result<Vec<rijen::Rijen>, Vec<String>> {
                let mut uit = Vec::new();
                for r in defs {
                    let mut rijbronnen = Vec::new();
                    for b in &r.sources {
                        rijbronnen.push(rijen::Bron {
                            definitie: b.clone(),
                            transport: transport(&b.url)?,
                        });
                    }
                    uit.push(rijen::Rijen {
                        definitie: r.clone(),
                        sources: rijbronnen,
                    });
                }
                Ok(uit)
            };
            // Per handeling de bronnen die haar artikel vraagt en haar
            // synthese per regel.
            let mut actions = Vec::new();
            for h in proces.actions() {
                let pick = handeling::bronnen_voor(&proces, h);
                actions.push(HandelingState {
                    sources: pick.iter().map(|i| sources[*i].clone()).collect(),
                    rows: per_regel(&h.rows)?,
                });
            }
            let toets_rijen = per_regel(proces.toets_rijen())?;
            processtaten.push(ProcesState {
                proces: Arc::new(proces),
                // De cel waarin het proces vastlegt, draait in deze runtime.
                cell: intern.clone(),
                sessies: Arc::new(Sessies::default()),
                klok: klok.clone(),
                sources: Arc::new(sources),
                actions: Arc::new(actions),
                toets_rijen: Arc::new(toets_rijen),
                regulations: geladen.clone(),
            });
        }
        let router = bouw_router(&celstaten, &processtaten);
        // Pas nu bestaat de router, en daarmee het interne transport.
        let _ = slot.set(router.clone());
        Ok(Self {
            cells: celstaten,
            processen: processtaten,
            router,
            runtime_token,
        })
    }

    /// De waarschuwingen over synthese-bronnen: onbereikbaar, of zonder de
    /// verwachte lexostatus of parameters. Geen reden om niet te starten.
    pub async fn warnings(&self) -> Vec<String> {
        let mut uit = Vec::new();
        for s in &self.processen {
            uit.extend(met_proces(s.proces.id(), s.proces.warnings.clone()));
            for b in s.sources.iter() {
                // Een intern transport naar een cel die hier niet draait.
                if b.definitie.url.is_none()
                    && b.definitie.regulation.is_none()
                    && !self.cells.iter().any(|c| c.cell.id() == b.definitie.cell)
                {
                    uit.push(format!(
                        "proces '{}': synthese-bron '{}' draait niet in deze runtime en heeft geen url; de toets meldt haar onbereikbaar",
                        s.proces.id(),
                        b.definitie.cell
                    ));
                }
            }
            uit.extend(synthese::warnings(s.proces.id(), &s.sources).await);
        }
        uit.dedup();
        uit
    }
}

/// Open de kroniek van een cel. Is elke kroniek van de cel leeg, dan komt de
/// startstand erin, met de laadtijd als `vastgelegd_op`.
fn open_kroniek(data_dir: &Path, cell: &Cell, klok: &Klok) -> Result<Kroniek, String> {
    let chronicle = Kroniek::open(&data_dir.join(cell.id()), &cell.chronicles())?;
    if !cell.initial_state.is_empty()
        && chronicle.zet_startstand(
            &cell.chronicles(),
            &startstand::geplaatst(&cell.initial_state, &klok())?,
        )?
    {
        tracing::info!(cell = %cell.id(), grams = cell.initial_state.len(), "startstand in lege kroniek gezet");
    }
    Ok(chronicle)
}

/// Een lijst als route.
fn lijst_route(list: Vec<Value>) -> axum::routing::MethodRouter {
    let list = Arc::new(Value::Array(list));
    get(move || {
        let list = list.clone();
        async move { Json(list.as_ref().clone()) }
    })
}

/// `GET /api/cellen`, `GET /api/processen`, per cel haar routes onder
/// `/cellen/<id>` en per proces zijn routes onder `/processen/<id>`.
fn bouw_router(cells: &[CelState], processen: &[ProcesState]) -> Router {
    let mut router = Router::new()
        .route(
            "/api/cells",
            lijst_route(cells.iter().map(api::cel_beschrijving).collect()),
        )
        .route(
            "/api/processes",
            lijst_route(processen.iter().map(api::proces_beschrijving).collect()),
        );
    for s in cells {
        router = router.nest(
            &format!("/cells/{}", s.cell.id()),
            api::cel_router(s.clone()),
        );
    }
    for s in processen {
        router = router.nest(
            &format!("/processes/{}", s.proces.id()),
            api::proces_router(s.clone()),
        );
    }
    router
}
