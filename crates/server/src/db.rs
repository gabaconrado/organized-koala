//! Database pool construction and the embedded-migration runner (ADR-0004).

use sqlx::PgPool;
use sqlx::migrate::Migrator;
use sqlx::postgres::PgPoolOptions;

/// The `migrations/` tree, embedded into the binary so the shipped artifact carries its own
/// schema and needs no `sqlx` CLI or checkout at runtime (ADR-0004).
static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// Connect to Postgres and return a connection pool.
pub async fn connect(database_url: &str) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await?;
    Ok(pool)
}

/// Apply all pending migrations. Idempotent: a no-op when the schema is already current.
pub async fn migrate(pool: &PgPool) -> anyhow::Result<()> {
    MIGRATOR.run(pool).await?;
    Ok(())
}

/// Revert `steps` applied migrations, most recent first. `steps == 0` is a no-op.
///
/// sqlx's `undo` reverts down to a target version; we resolve the target by walking back
/// `steps` applied migrations so the bound is explicit and never "rollback the world".
pub async fn rollback(pool: &PgPool, steps: u32) -> anyhow::Result<()> {
    if steps == 0 {
        return Ok(());
    }

    // Versions of applied migrations that carry a `down`, newest-first.
    let mut applied: Vec<i64> = sqlx::query_scalar!(
        "SELECT version FROM _sqlx_migrations WHERE success ORDER BY version DESC"
    )
    .fetch_all(pool)
    .await?;

    let reversible: Vec<i64> = MIGRATOR
        .iter()
        .filter(|m| m.migration_type.is_down_migration())
        .map(|m| m.version)
        .collect();
    applied.retain(|v| reversible.contains(v));

    // Target = the version just below the `steps`-th most recent applied migration.
    // Reverting "to" that target undoes exactly `steps` migrations (or all of them).
    let steps = usize::try_from(steps).unwrap_or(usize::MAX);
    let target = applied.get(steps).copied().unwrap_or(-1);
    MIGRATOR.undo(pool, target).await?;
    Ok(())
}
