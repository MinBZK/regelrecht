use sqlx::PgPool;
use testcontainers::runners::AsyncRunner;
use testcontainers::ContainerAsync;
use testcontainers_modules::postgres::Postgres;

use regelrecht_pipeline::config::PipelineConfig;
use regelrecht_pipeline::db;

pub struct TestDb {
    pub pool: PgPool,
    _container: ContainerAsync<Postgres>,
}

impl TestDb {
    pub async fn new() -> Self {
        let container = Postgres::default().start().await.unwrap();

        let host_port = container.get_host_port_ipv4(5432).await.unwrap();
        let database_url = format!(
            "postgres://postgres:postgres@127.0.0.1:{}/postgres",
            host_port
        );

        let config = PipelineConfig::new(&database_url);
        let pool = db::create_pool(&config).await.unwrap();
        db::ensure_schema(&pool).await.unwrap();

        // Clear seed data from migrations so tests start with empty tables
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
