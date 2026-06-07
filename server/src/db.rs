use anyhow::Result;
use sqlx::SqlitePool;

use crate::models::{ImageRecord, ImageSummary, PlanEntry, PlanResponse};

#[derive(Debug, Clone)]
pub struct Store {
    pool: SqlitePool,
}

#[derive(Debug, Clone)]
pub struct NewPlanEntry {
    pub start: String,
    pub end: String,
    pub caption: String,
    pub images: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct NewImage<'a> {
    pub sha256: &'a str,
    pub content_type: &'a str,
    pub bytes: &'a [u8],
}

#[derive(Debug, sqlx::FromRow)]
struct PlanEntryRow {
    start_date: String,
    end_date: String,
    caption: String,
    images_json: String,
}

impl Store {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn replace_plan_entries(
        &self,
        version: &str,
        entries: &[NewPlanEntry],
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        sqlx::query("DELETE FROM plan_entries")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM plan_versions")
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO plan_versions (id, version) VALUES (1, ?)")
            .bind(version)
            .execute(&mut *tx)
            .await?;

        for (position, entry) in entries.iter().enumerate() {
            let images_json = serde_json::to_string(&entry.images)?;
            sqlx::query(
                "INSERT INTO plan_entries (start_date, end_date, caption, images_json, position)
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(&entry.start)
            .bind(&entry.end)
            .bind(&entry.caption)
            .bind(images_json)
            .bind(position as i64)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    pub async fn load_plan_response(&self) -> Result<Option<PlanResponse>> {
        let version =
            sqlx::query_scalar::<_, String>("SELECT version FROM plan_versions WHERE id = 1")
                .fetch_optional(&self.pool)
                .await?;
        let Some(version) = version else {
            return Ok(None);
        };

        let rows = sqlx::query_as::<_, PlanEntryRow>(
            "SELECT start_date, end_date, caption, images_json
             FROM plan_entries
             ORDER BY start_date ASC, position ASC, id ASC",
        )
        .fetch_all(&self.pool)
        .await?;

        let plans = rows
            .into_iter()
            .map(|row| {
                let images = serde_json::from_str(&row.images_json)?;
                Ok(PlanEntry {
                    start: row.start_date,
                    end: row.end_date,
                    caption: row.caption,
                    images,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(Some(PlanResponse { version, plans }))
    }

    pub async fn upsert_image(&self, image: NewImage<'_>) -> Result<()> {
        sqlx::query(
            "INSERT INTO images (sha256, content_type, bytes)
             VALUES (?, ?, ?)
             ON CONFLICT(sha256) DO UPDATE SET
                 content_type = excluded.content_type,
                 bytes = excluded.bytes",
        )
        .bind(image.sha256)
        .bind(image.content_type)
        .bind(image.bytes)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_image(&self, sha256: &str) -> Result<Option<ImageRecord>> {
        let image = sqlx::query_as::<_, ImageRecord>(
            "SELECT sha256, content_type, bytes FROM images WHERE sha256 = ?",
        )
        .bind(sha256)
        .fetch_optional(&self.pool)
        .await?;

        Ok(image)
    }

    pub async fn list_images(&self) -> Result<Vec<ImageSummary>> {
        let images = sqlx::query_as::<_, ImageSummary>(
            "SELECT sha256, content_type, length(bytes) AS size
             FROM images
             ORDER BY sha256 ASC",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(images)
    }

    pub async fn delete_image(&self, sha256: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM images WHERE sha256 = ?")
            .bind(sha256)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }
}

pub async fn init_schema(pool: &SqlitePool) -> Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS plan_versions (
            id      INTEGER PRIMARY KEY CHECK (id = 1),
            version TEXT NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS plan_entries (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            start_date  TEXT NOT NULL,
            end_date    TEXT NOT NULL,
            caption     TEXT NOT NULL,
            images_json TEXT NOT NULL,
            position    INTEGER NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS images (
            sha256       TEXT PRIMARY KEY,
            content_type TEXT NOT NULL,
            bytes        BLOB NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}
