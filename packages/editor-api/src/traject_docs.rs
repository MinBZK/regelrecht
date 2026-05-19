//! Per-traject docs handlers.
//!
//! Two endpoints:
//!   GET /api/trajects/{traject_id}/docs/tree
//!     -> JSON: { sources: [ { source_id, name, tree: [ { path } ] } ] }
//!   GET /api/trajects/{traject_id}/docs/page?source={src}&path={p}
//!     -> text/markdown; charset=utf-8
//!
//! ## Current state: stub
//!
//! This module returns **canned data** while two upstream PRs are still in
//! review:
//!
//! - PR #632 (tdjager/trajects) introduces the `trajects`, `traject_members`,
//!   `accounts`, and `traject_corpus_sources` tables. Without those we can't
//!   do the real authz check ("is this account a member of this traject?")
//!   nor list the traject's sources.
//! - PR #626 (tdjager/feat/layered-rbac) introduces the
//!   `editor-reader/writer/admin` realm-role gating; the docs routes belong
//!   on `editor-reader`.
//!
//! When both PRs land this module switches to:
//!   1. Resolving `session.person_sub -> accounts.id` (one query).
//!   2. Asserting membership in `traject_members` for `traject_id`. Not a
//!      member → 403.
//!   3. Selecting `traject_corpus_sources` rows for the traject and walking
//!      each source's on-disk clone (the same clones #632's writable-own
//!      handlers use). The `docs/` subdir of each clone is the docs tree.
//!
//! The handler signatures and response shapes here are stable across the
//! stub→real transition — the frontend (`frontend/src/composables/useDocs.js`)
//! does not need to change.

use axum::extract::{Path, Query};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct DocsTreeEntry {
    pub path: String,
}

#[derive(Serialize)]
pub struct DocsSourceTree {
    pub source_id: String,
    pub name: String,
    pub tree: Vec<DocsTreeEntry>,
}

#[derive(Serialize)]
pub struct DocsTreeResponse {
    pub sources: Vec<DocsSourceTree>,
}

#[derive(Deserialize)]
pub struct PageQuery {
    pub source: String,
    pub path: String,
}

/// GET /api/trajects/{traject_id}/docs/tree
///
/// Stub: returns a single fictive "dummy" source whose tree mirrors the
/// `regelrecht-corpus-dummy` repo. Once PR #632 lands, this becomes a DB
/// query over `traject_corpus_sources` + directory walks per clone.
pub async fn tree(Path(_traject_id): Path<String>) -> Json<DocsTreeResponse> {
    Json(DocsTreeResponse {
        sources: vec![DocsSourceTree {
            source_id: "dummy".to_string(),
            name: "Gemeente Dummy — Verordening Fietsval-verbod".to_string(),
            tree: stub_dummy_tree(),
        }],
    })
}

/// GET /api/trajects/{traject_id}/docs/page?source={src}&path={p}
///
/// Stub: returns canned markdown for known (source, path) pairs. For unknown
/// paths returns 404 (so the frontend's error path renders). Once PR #632
/// lands, this reads from `/opt/corpora/<clone-cache>/<source>/docs/<path>`
/// after asserting the requester is a member of the traject.
pub async fn page(Path(_traject_id): Path<String>, Query(q): Query<PageQuery>) -> Response {
    let Some(body) = stub_page_body(&q.source, &q.path) else {
        return (StatusCode::NOT_FOUND, "page not found").into_response();
    };
    (
        [(header::CONTENT_TYPE, "text/markdown; charset=utf-8")],
        body,
    )
        .into_response()
}

// ---------------------------------------------------------------------------
// Stub fixtures — REMOVE these once the real DB-backed implementation lands.
// ---------------------------------------------------------------------------

fn stub_dummy_tree() -> Vec<DocsTreeEntry> {
    [
        "analysis/fietsval-verbod-context.md",
        "analysis/handhavingsbevoegdheid.md",
        "analysis/samenloop-rijksregels.md",
        "diagrams/cross-law-gemeente-dummy.md",
        "diagrams/fietsval-flow-2025-04-01.md",
        "issues/issue-onduidelijkheid-definitie-fietsval.md",
        "plans/2025-03-15-implementatie-traject.md",
        "reviews/review-eerste-uitvoering.md",
    ]
    .iter()
    .map(|p| DocsTreeEntry {
        path: (*p).to_string(),
    })
    .collect()
}

fn stub_page_body(source: &str, path: &str) -> Option<String> {
    if source != "dummy" {
        return None;
    }
    let body = match path {
        "analysis/fietsval-verbod-context.md" => STUB_FIETSVAL_CONTEXT,
        "analysis/handhavingsbevoegdheid.md" => STUB_HANDHAVING,
        "diagrams/fietsval-flow-2025-04-01.md" => STUB_FLOW_DIAGRAM,
        "diagrams/cross-law-gemeente-dummy.md" => STUB_CROSS_LAW_DIAGRAM,
        _ => STUB_PLACEHOLDER,
    };
    Some(body.to_string())
}

const STUB_FIETSVAL_CONTEXT: &str = r#"# Context: waarom een fietsval-verbod?

> Stuk is **volledig fictief** en dient als demo-tekst voor de
> docs-platform-ingest. Geen verwijzingen naar bestaande casuïstiek.

De Gemeente Dummy worstelt sinds enkele jaren met een toename van
fietsvalmeldingen in het centrum. Uit de (verzonnen) bestuurlijke
inventarisatie volgt dat:

1. Het aantal meldingen tussen 2022 en 2024 met circa 40% is gestegen.
2. De directe oorzaak in meerderheid van de gevallen niet aan
   wegonderhoud is te wijten, maar aan gedragsfactoren.
3. Bestaande rijksregels (zoals de Wegenverkeerswet) onvoldoende
   handvatten bieden om gemeentelijke prioriteiten te stellen.

Tegen deze achtergrond is gekozen voor een lokale verordening die het
verschijnsel als zelfstandige overtreding benoemt en handhavingsbeleid
mogelijk maakt zonder afhankelijkheid van het Openbaar Ministerie.
"#;

const STUB_HANDHAVING: &str = r#"# Handhavingsbevoegdheid en rol BOA's

De Verordening Handhaving Fietsval-verbod Gemeente Dummy 2025 wijst
handhaving expliciet toe aan buitengewoon opsporingsambtenaren (BOA's)
in dienst van — of aangewezen door — het college.

Twee aspecten verdienen nadere aandacht: de **bevoegdheidsketen** en de
**registratieplicht**. Beide worden hieronder besproken.
"#;

const STUB_FLOW_DIAGRAM: &str = r#"# Flow: constatering → registratie → sanctie

```mermaid
flowchart TD
    A[Constatering fietsval door BOA] --> B{Valt onder uitzondering?}
    B -- ja --> X[Geen sanctie<br/>geen registratie]
    B -- nee --> C[Registratie in handhavingsregister<br/>binnen 24 uur]
    C --> D{Recidive binnen 12 mnd?}
    D -- nee --> F[Boete ≤ € 30]
    D -- ja --> H[Boete ≤ € 90]
```

Bovenstaand diagram laat zien hoe een constatering door de BOA via
registratie tot een sanctie kan leiden.
"#;

const STUB_CROSS_LAW_DIAGRAM: &str = r#"# Cross-law relaties binnen Gemeente Dummy

```mermaid
graph LR
    V1[Verordening Fietsval-verbod] --> V2[Verordening Handhaving]
    V2 --> B1[Beleidsregel Uitvoering]
    B1 --> E1[Wegenverkeerswet]
```

Pijl-richting = juridische delegatie of verwijzing.
"#;

const STUB_PLACEHOLDER: &str = r#"# Stub

Deze pagina is nog niet uitgewerkt. Dit is een platzhalter terwijl
PR #632 (trajects) nog in review zit.
"#;
