use regelrecht_pipeline::feature_flags;
use regelrecht_pipeline::test_utils::TestDb;

const FIX_SEED: &str = include_str!("../migrations/0036_machine_readable_flag_default.sql");

/// A fresh database runs the 0012 seed (machine_readable off) and then 0036,
/// which turns the untouched seed row on to match the registered default.
#[tokio::test]
async fn seeded_machine_readable_flag_ends_up_on() {
    let db = TestDb::new().await;
    let flag = feature_flags::get_flag(&db.pool, "panel.machine_readable")
        .await
        .unwrap()
        .expect("seed row present");
    assert!(flag.enabled);
}

/// A row someone switched off after the seed is a deliberate choice; replaying
/// the correction must leave it off.
#[tokio::test]
async fn toggled_machine_readable_flag_is_left_alone() {
    let db = TestDb::new().await;
    // Rewind to the seed state, then simulate a later toggle in its own
    // transaction so `updated_at` moves past `created_at`.
    sqlx::query(
        "UPDATE feature_flags SET enabled = false, created_at = now() - interval '1 day' \
         WHERE key = 'panel.machine_readable'",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    feature_flags::upsert_flag(&db.pool, "panel.machine_readable", false, None)
        .await
        .unwrap();

    sqlx::raw_sql(FIX_SEED).execute(&db.pool).await.unwrap();

    let flag = feature_flags::get_flag(&db.pool, "panel.machine_readable")
        .await
        .unwrap()
        .unwrap();
    assert!(!flag.enabled);
}

/// The untouched seed state (off, never updated) is what the correction flips.
#[tokio::test]
async fn untouched_seed_row_is_corrected() {
    let db = TestDb::new().await;
    // Recreate the exact seed row: both timestamps from the same `now()`.
    sqlx::query("DELETE FROM feature_flags WHERE key = 'panel.machine_readable'")
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO feature_flags (key, enabled, description) \
         VALUES ('panel.machine_readable', false, 'Machine readable weergave')",
    )
    .execute(&db.pool)
    .await
    .unwrap();

    sqlx::raw_sql(FIX_SEED).execute(&db.pool).await.unwrap();

    let flag = feature_flags::get_flag(&db.pool, "panel.machine_readable")
        .await
        .unwrap()
        .unwrap();
    assert!(flag.enabled);
}
