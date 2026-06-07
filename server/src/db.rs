use std::collections::HashSet;

use anyhow::{anyhow, Result};
use sqlx::SqlitePool;

use crate::models::{AdminPlan, ImageRecord, PlanPayload, UserPlan};

#[derive(Debug, Clone)]
pub struct Store {
    pool: SqlitePool,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct PlanRow {
    id: i64,
    start_date: String,
    end_date: String,
    caption: String,
    images: String,
}

impl Store {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn list_admin_plans(&self, start: &str, end: &str) -> Result<Vec<AdminPlan>> {
        let rows = self.load_plan_rows(start, end).await?;
        let mut plans = Vec::with_capacity(rows.len());

        for row in rows {
            let sha256s = parse_images(&row.images)?;
            let mut images = Vec::with_capacity(sha256s.len());
            for sha256 in sha256s {
                if let Some(image) = self.get_image(&sha256).await? {
                    images.push(image);
                }
            }
            plans.push(AdminPlan {
                id: row.id,
                start: row.start_date,
                end: row.end_date,
                caption: row.caption,
                images,
            });
        }

        Ok(plans)
    }

    pub async fn list_user_plans(&self, start: &str, end: &str) -> Result<Vec<UserPlan>> {
        let rows = self.load_plan_rows(start, end).await?;
        let mut plans = Vec::with_capacity(rows.len());

        for row in rows {
            let sha256s = parse_images(&row.images)?;
            let mut images = Vec::new();
            for sha256 in sha256s {
                let status: Option<String> =
                    sqlx::query_scalar("SELECT status FROM images WHERE sha256 = ?")
                        .bind(&sha256)
                        .fetch_optional(&self.pool)
                        .await?;
                if status.as_deref() == Some("ready") {
                    images.push(sha256);
                }
            }
            plans.push(UserPlan {
                id: row.id,
                start: row.start_date,
                end: row.end_date,
                caption: row.caption,
                images,
            });
        }

        Ok(plans)
    }

    pub async fn create_plan(&self, payload: PlanPayload) -> Result<AdminPlan> {
        let images = self.validate_and_deduplicate_images(payload.images).await?;
        let images_json = serde_json::to_string(&images)?;
        let result = sqlx::query(
            "INSERT INTO plans (start_date, end_date, caption, images) VALUES (?, ?, ?, ?)",
        )
        .bind(&payload.start)
        .bind(&payload.end)
        .bind(&payload.caption)
        .bind(images_json)
        .execute(&self.pool)
        .await?;

        self.get_admin_plan(result.last_insert_rowid())
            .await?
            .ok_or_else(|| anyhow!("created plan not found"))
    }

    pub async fn update_plan(&self, id: i64, payload: PlanPayload) -> Result<Option<AdminPlan>> {
        let images = self.validate_and_deduplicate_images(payload.images).await?;
        let images_json = serde_json::to_string(&images)?;
        let result = sqlx::query(
            "UPDATE plans SET start_date = ?, end_date = ?, caption = ?, images = ? WHERE id = ?",
        )
        .bind(&payload.start)
        .bind(&payload.end)
        .bind(&payload.caption)
        .bind(images_json)
        .bind(id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Ok(None);
        }

        self.get_admin_plan(id).await
    }

    pub async fn delete_plan(&self, id: i64) -> Result<bool> {
        let result = sqlx::query("DELETE FROM plans WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn list_images(&self, keyword: Option<&str>) -> Result<Vec<ImageRecord>> {
        let images = if let Some(keyword) = keyword.filter(|value| !value.is_empty()) {
            sqlx::query_as::<_, ImageRecord>(
                "SELECT sha256, status, remark FROM images
                 WHERE remark LIKE '%' || ? || '%'
                 ORDER BY sha256 ASC",
            )
            .bind(keyword)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, ImageRecord>(
                "SELECT sha256, status, remark FROM images ORDER BY sha256 ASC",
            )
            .fetch_all(&self.pool)
            .await?
        };

        Ok(images)
    }

    pub async fn get_image(&self, sha256: &str) -> Result<Option<ImageRecord>> {
        let image = sqlx::query_as::<_, ImageRecord>(
            "SELECT sha256, status, remark FROM images WHERE sha256 = ?",
        )
        .bind(sha256)
        .fetch_optional(&self.pool)
        .await?;

        Ok(image)
    }

    pub async fn upsert_uploaded_image(
        &self,
        sha256: &str,
        remark: Option<&str>,
    ) -> Result<(ImageRecord, bool)> {
        let existing = self.get_image(sha256).await?;

        match existing {
            Some(image) => {
                let mut next_status = image.status.clone();
                let mut should_enqueue = matches!(image.status.as_str(), "pending" | "processing");
                if image.status == "failed" {
                    next_status = "pending".to_string();
                    should_enqueue = true;
                }

                if let Some(remark) = remark {
                    sqlx::query("UPDATE images SET status = ?, remark = ? WHERE sha256 = ?")
                        .bind(&next_status)
                        .bind(remark)
                        .bind(sha256)
                        .execute(&self.pool)
                        .await?;
                } else if next_status != image.status {
                    sqlx::query("UPDATE images SET status = ? WHERE sha256 = ?")
                        .bind(&next_status)
                        .bind(sha256)
                        .execute(&self.pool)
                        .await?;
                }

                let updated = self
                    .get_image(sha256)
                    .await?
                    .ok_or_else(|| anyhow!("updated image not found"))?;
                Ok((updated, should_enqueue))
            }
            None => {
                let remark = remark.unwrap_or_default();
                sqlx::query("INSERT INTO images (sha256, status, remark) VALUES (?, 'pending', ?)")
                    .bind(sha256)
                    .bind(remark)
                    .execute(&self.pool)
                    .await?;
                let image = self
                    .get_image(sha256)
                    .await?
                    .ok_or_else(|| anyhow!("inserted image not found"))?;
                Ok((image, true))
            }
        }
    }

    pub async fn update_image_remark(
        &self,
        sha256: &str,
        remark: &str,
    ) -> Result<Option<ImageRecord>> {
        let result = sqlx::query("UPDATE images SET remark = ? WHERE sha256 = ?")
            .bind(remark)
            .bind(sha256)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Ok(None);
        }
        self.get_image(sha256).await
    }

    pub async fn recover_processing_images(&self) -> Result<()> {
        sqlx::query("UPDATE images SET status = 'pending' WHERE status = 'processing'")
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn mark_ready_missing_display_pending(
        &self,
        missing_sha256s: &[String],
    ) -> Result<()> {
        for sha256 in missing_sha256s {
            sqlx::query(
                "UPDATE images SET status = 'pending' WHERE sha256 = ? AND status = 'ready'",
            )
            .bind(sha256)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    pub async fn pending_sha256s(&self) -> Result<Vec<String>> {
        let sha256s = sqlx::query_scalar::<_, String>(
            "SELECT sha256 FROM images WHERE status = 'pending' ORDER BY sha256 ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(sha256s)
    }

    pub async fn ready_sha256s(&self) -> Result<Vec<String>> {
        let sha256s = sqlx::query_scalar::<_, String>(
            "SELECT sha256 FROM images WHERE status = 'ready' ORDER BY sha256 ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(sha256s)
    }

    pub async fn claim_pending(&self, sha256: &str) -> Result<bool> {
        let result = sqlx::query(
            "UPDATE images SET status = 'processing' WHERE sha256 = ? AND status = 'pending'",
        )
        .bind(sha256)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn mark_ready(&self, sha256: &str) -> Result<()> {
        sqlx::query("UPDATE images SET status = 'ready' WHERE sha256 = ?")
            .bind(sha256)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn mark_failed(&self, sha256: &str) -> Result<()> {
        sqlx::query("UPDATE images SET status = 'failed' WHERE sha256 = ?")
            .bind(sha256)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn load_plan_rows(&self, start: &str, end: &str) -> Result<Vec<PlanRow>> {
        let rows = sqlx::query_as::<_, PlanRow>(
            "SELECT id, start_date, end_date, caption, images
             FROM plans
             WHERE start_date <= ? AND end_date >= ?
             ORDER BY start_date ASC, id ASC",
        )
        .bind(end)
        .bind(start)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    async fn get_admin_plan(&self, id: i64) -> Result<Option<AdminPlan>> {
        let row = sqlx::query_as::<_, PlanRow>(
            "SELECT id, start_date, end_date, caption, images FROM plans WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        let Some(row) = row else {
            return Ok(None);
        };

        let sha256s = parse_images(&row.images)?;
        let mut images = Vec::with_capacity(sha256s.len());
        for sha256 in sha256s {
            if let Some(image) = self.get_image(&sha256).await? {
                images.push(image);
            }
        }

        Ok(Some(AdminPlan {
            id: row.id,
            start: row.start_date,
            end: row.end_date,
            caption: row.caption,
            images,
        }))
    }

    async fn validate_and_deduplicate_images(&self, images: Vec<String>) -> Result<Vec<String>> {
        let mut seen = HashSet::new();
        let mut deduplicated = Vec::new();
        for sha256 in images {
            if !seen.insert(sha256.clone()) {
                continue;
            }
            let exists: Option<i64> =
                sqlx::query_scalar("SELECT 1 FROM images WHERE sha256 = ? LIMIT 1")
                    .bind(&sha256)
                    .fetch_optional(&self.pool)
                    .await?;
            if exists.is_none() {
                return Err(anyhow!("Unknown image sha256: {sha256}"));
            }
            deduplicated.push(sha256);
        }
        Ok(deduplicated)
    }
}

pub async fn init_schema(pool: &SqlitePool) -> Result<()> {
    drop_incompatible_table(
        pool,
        "plans",
        &["id", "start_date", "end_date", "caption", "images"],
    )
    .await?;
    drop_incompatible_table(pool, "images", &["sha256", "status", "remark"]).await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS plans (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            start_date TEXT NOT NULL,
            end_date   TEXT NOT NULL,
            caption    TEXT NOT NULL,
            images     TEXT NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS images (
            sha256  TEXT PRIMARY KEY,
            status  TEXT NOT NULL CHECK (status IN ('pending', 'processing', 'ready', 'failed')),
            remark  TEXT NOT NULL DEFAULT ''
        )
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

fn parse_images(value: &str) -> Result<Vec<String>> {
    serde_json::from_str(value).map_err(Into::into)
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
