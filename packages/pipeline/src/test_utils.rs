//! Test helpers exposed via the `test-utils` Cargo feature.
//!
//! These are used by this crate's integration tests and by downstream
//! crates (e.g. admin) that need a real Postgres container with the
//! pipeline schema applied.

use sqlx::PgPool;
use testcontainers::runners::AsyncRunner;
use testcontainers::ContainerAsync;
use testcontainers_modules::postgres::Postgres;

use crate::config::PipelineConfig;
use crate::db;

/// Postgres testcontainer with the pipeline schema applied and seed rows
/// truncated, ready for integration tests.
pub struct TestDb {
    pub pool: PgPool,
    _container: ContainerAsync<Postgres>,
}

impl TestDb {
    #[allow(clippy::unwrap_used)]
    pub async fn new() -> Self {
        let container = Postgres::default().start().await.unwrap();

        let host_port = container.get_host_port_ipv4(5432).await.unwrap();
        // testcontainers reports the published port on the docker host. From a
        // native Linux dev environment `127.0.0.1` is correct; from a dev
        // container talking to Docker Desktop on a different host (WSL2,
        // remote docker) the docker host is reachable as
        // `host.docker.internal` instead. `TESTCONTAINERS_HOST_OVERRIDE` lets
        // those setups point at the right host without forking this helper.
        let host = std::env::var("TESTCONTAINERS_HOST_OVERRIDE")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "127.0.0.1".to_string());
        let database_url = format!("postgres://postgres:postgres@{host}:{host_port}/postgres");

        let config = PipelineConfig::new(&database_url);
        let pool = db::create_pool(&config).await.unwrap();
        db::ensure_schema(&pool).await.unwrap();

        // Clear seed data from migrations so tests start with empty tables.
        sqlx::query("TRUNCATE jobs, law_entries CASCADE")
            .execute(&pool)
            .await
            .unwrap();

        Self {
            pool,
            _container: container,
        }
    }
}
