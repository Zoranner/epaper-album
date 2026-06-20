use std::collections::HashSet;

use anyhow::Result;
use sqlx::SqlitePool;

use super::now_string;

pub async fn init_schema(pool: &SqlitePool) -> Result<()> {
    drop_incompatible_table(pool, "plans", &["date", "caption", "image"]).await?;
    drop_table_with_foreign_keys(pool, "plans").await?;
    drop_incompatible_table(pool, "images", &["sha256", "status", "remark"]).await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS plans (
            date          TEXT PRIMARY KEY CHECK (
                date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'
            ),
            caption       TEXT NOT NULL,
            type          TEXT NOT NULL DEFAULT 'fixed' CHECK (type IN ('fixed', 'random')),
            image         TEXT NOT NULL DEFAULT '',
            tags          TEXT NOT NULL DEFAULT '[]'
        )
        "#,
    )
    .execute(pool)
    .await?;

    ensure_column(pool, "plans", "type", "TEXT NOT NULL DEFAULT 'fixed'").await?;
    ensure_column(pool, "plans", "tags", "TEXT NOT NULL DEFAULT '[]'").await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS images (
            sha256     TEXT PRIMARY KEY,
            status     TEXT NOT NULL CHECK (status IN ('pending', 'processing', 'ready', 'failed')),
            remark     TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL DEFAULT '',
            updated_at TEXT NOT NULL DEFAULT ''
        )
        "#,
    )
    .execute(pool)
    .await?;

    ensure_column(pool, "images", "created_at", "TEXT NOT NULL DEFAULT ''").await?;
    ensure_column(pool, "images", "updated_at", "TEXT NOT NULL DEFAULT ''").await?;
    backfill_image_timestamps(pool).await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS image_tags (
            image TEXT NOT NULL,
            tag   TEXT NOT NULL CHECK (length(trim(tag)) > 0),
            PRIMARY KEY (image, tag),
            FOREIGN KEY (image) REFERENCES images(sha256) ON DELETE CASCADE
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_image_tags_tag ON image_tags(tag)")
        .execute(pool)
        .await?;

    Ok(())
}

async fn drop_table_with_foreign_keys(pool: &SqlitePool, table: &str) -> Result<()> {
    let rows = sqlx::query(&format!("PRAGMA foreign_key_list({table})"))
        .fetch_all(pool)
        .await?;
    if rows.is_empty() {
        return Ok(());
    }

    sqlx::query(&format!("DROP TABLE {table}"))
        .execute(pool)
        .await?;
    Ok(())
}

async fn drop_incompatible_table(
    pool: &SqlitePool,
    table: &str,
    required_columns: &[&str],
) -> Result<()> {
    let rows = sqlx::query_as::<_, TableColumn>(&format!("PRAGMA table_info({table})"))
        .fetch_all(pool)
        .await?;
    if rows.is_empty() {
        return Ok(());
    }

    let columns = rows.into_iter().map(|row| row.name).collect::<HashSet<_>>();
    if required_columns
        .iter()
        .all(|column| columns.contains(*column))
    {
        return Ok(());
    }

    sqlx::query(&format!("DROP TABLE {table}"))
        .execute(pool)
        .await?;
    Ok(())
}

#[derive(Debug, sqlx::FromRow)]
struct TableColumn {
    name: String,
}

async fn ensure_column(
    pool: &SqlitePool,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<()> {
    let rows = sqlx::query_as::<_, TableColumn>(&format!("PRAGMA table_info({table})"))
        .fetch_all(pool)
        .await?;
    if rows.into_iter().any(|row| row.name == column) {
        return Ok(());
    }

    sqlx::query(&format!(
        "ALTER TABLE {table} ADD COLUMN {column} {definition}"
    ))
    .execute(pool)
    .await?;
    Ok(())
}

async fn backfill_image_timestamps(pool: &SqlitePool) -> Result<()> {
    let now = now_string();
    sqlx::query("UPDATE images SET created_at = ? WHERE created_at = ''")
        .bind(&now)
        .execute(pool)
        .await?;
    sqlx::query("UPDATE images SET updated_at = created_at WHERE updated_at = ''")
        .execute(pool)
        .await?;
    Ok(())
}
