//! Test helpers exposed via the `test-utils` Cargo feature.
//!
//! These are used by this crate's integration tests and by downstream
//! crates (e.g. admin) that need a real Postgres container with the
//! pipeline schema applied.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Weak};

use sqlx::PgPool;
use testcontainers::runners::AsyncRunner;
use testcontainers::ContainerAsync;
use testcontainers_modules::postgres::Postgres;
use tokio::sync::Mutex;

use crate::config::PipelineConfig;
use crate::db;

/// One Postgres container per test binary, shared by every `TestDb` in it,
/// together with the connection details resolved once.
///
/// A container per test is what this used to do, and at 185 call sites that is
/// 185 container starts per run. Resolving the published port is a second
/// daemon round-trip; doing that per test had thirty tests inspecting the
/// daemon at once, which fails with EAGAIN and reads as a test failure.
///
/// Held as a `Weak` so the container still stops when the last `TestDb` in the
/// binary drops; a `static` is never dropped, so an owning handle here would
/// leave the container running after the process exits.
struct SharedPostgres {
    base_url: String,
    _container: ContainerAsync<Postgres>,
}

static SHARED_CONTAINER: LazyLock<Mutex<Weak<SharedPostgres>>> =
    LazyLock::new(|| Mutex::new(Weak::new()));

/// Names the per-test database. Only unique within one test binary, which is
/// enough: each binary has its own container.
static DB_SEQ: AtomicU64 = AtomicU64::new(0);

#[allow(clippy::unwrap_used)]
async fn shared_postgres() -> Arc<SharedPostgres> {
    let mut shared = SHARED_CONTAINER.lock().await;
    if let Some(running) = shared.upgrade() {
        return running;
    }

    let container = Postgres::default().start().await.unwrap();
    let host_port = container.get_host_port_ipv4(5432).await.unwrap();
    // testcontainers reports the published port on the docker host. From a
    // native Linux dev environment `127.0.0.1` is correct; from a dev
    // container talking to Docker Desktop on a different host (WSL2, remote
    // docker) the docker host is reachable as `host.docker.internal` instead.
    // `TESTCONTAINERS_HOST_OVERRIDE` lets those setups point at the right host
    // without forking this helper.
    let host = std::env::var("TESTCONTAINERS_HOST_OVERRIDE")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "127.0.0.1".to_string());

    let started = Arc::new(SharedPostgres {
        base_url: format!("postgres://postgres:postgres@{host}:{host_port}"),
        _container: container,
    });
    *shared = Arc::downgrade(&started);
    started
}

/// Postgres database with the pipeline schema applied and seed rows truncated,
/// ready for integration tests. Every `TestDb` gets its own database inside the
/// shared container, so tests stay isolated from each other.
pub struct TestDb {
    pub pool: PgPool,
    _container: Arc<SharedPostgres>,
}

impl TestDb {
    #[allow(clippy::unwrap_used)]
    pub async fn new() -> Self {
        let shared = shared_postgres().await;

        let db_name = format!("regelrecht_test_{}", DB_SEQ.fetch_add(1, Ordering::Relaxed));
        let admin = PgPool::connect(&format!("{}/postgres", shared.base_url))
            .await
            .unwrap();
        sqlx::query(&format!("CREATE DATABASE \"{db_name}\""))
            .execute(&admin)
            .await
            .unwrap();
        admin.close().await;

        let config = PipelineConfig::new(format!("{}/{db_name}", shared.base_url));
        let pool = db::create_pool(&config).await.unwrap();
        // ensure_schema takes a pg_advisory_lock, which is scoped to the
        // database, so parallel tests do not queue behind each other.
        db::ensure_schema(&pool).await.unwrap();

        // Clear seed data from migrations so tests start with empty tables.
        sqlx::query("TRUNCATE jobs, law_entries CASCADE")
            .execute(&pool)
            .await
            .unwrap();

        Self {
            pool,
            _container: shared,
        }
    }
}
