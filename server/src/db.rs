use std::collections::HashSet;

use anyhow::{anyhow, Result};
use protocol::{LocalDate, Plan};
use sqlx::SqlitePool;

use crate::models::ImageRecord;

#[derive(Debug, Clone)]
pub struct Store {
    pool: SqlitePool,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct PlanRow {
    date: String,
    caption: String,
    image: String,
}

impl Store {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn list_admin_plans(&self, start: LocalDate, end: LocalDate) -> Result<Vec<Plan>> {
        let rows = self.load_plan_rows(start, end).await?;
        rows.into_iter().map(plan_from_row).collect()
    }

    pub async fn list_user_plans(&self, start: LocalDate, end: LocalDate) -> Result<Vec<Plan>> {
        let rows = self.load_plan_rows(start, end).await?;
        let mut plans = Vec::with_capacity(rows.len());

        for row in rows {
            let status: Option<String> =
                sqlx::query_scalar("SELECT status FROM images WHERE sha256 = ?")
                    .bind(&row.image)
                    .fetch_optional(&self.pool)
                    .await?;
            if status.as_deref() == Some("ready") {
                plans.push(plan_from_row(row)?);
            }
        }

        Ok(plans)
    }

    pub async fn create_plan(&self, payload: Plan) -> Result<Plan> {
        self.validate_image(&payload.image).await?;
        let result = sqlx::query("INSERT INTO plans (date, caption, image) VALUES (?, ?, ?)")
            .bind(payload.date.to_string())
            .bind(payload.caption.trim())
            .bind(&payload.image)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(anyhow!("Plan date already exists: {}", payload.date));
        }

        self.get_admin_plan(payload.date)
            .await?
            .ok_or_else(|| anyhow!("created plan not found"))
    }

    pub async fn update_plan(&self, date: LocalDate, payload: Plan) -> Result<Option<Plan>> {
        self.validate_image(&payload.image).await?;
        let result =
            sqlx::query("UPDATE plans SET date = ?, caption = ?, image = ? WHERE date = ?")
                .bind(payload.date.to_string())
                .bind(payload.caption.trim())
                .bind(&payload.image)
                .bind(date.to_string())
                .execute(&self.pool)
                .await?;

        if result.rows_affected() == 0 {
            return Ok(None);
        }

        self.get_admin_plan(payload.date).await
    }

    pub async fn delete_plan(&self, date: LocalDate) -> Result<bool> {
        let result = sqlx::query("DELETE FROM plans WHERE date = ?")
            .bind(date.to_string())
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

    pub async fn delete_image(&self, sha256: &str) -> Result<bool> {
        let mut transaction = self.pool.begin().await?;
        sqlx::query("UPDATE plans SET image = '' WHERE image = ?")
            .bind(sha256)
            .execute(&mut *transaction)
            .await?;
        let result = sqlx::query("DELETE FROM images WHERE sha256 = ?")
            .bind(sha256)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn requeue_image(&self, sha256: &str) -> Result<Option<ImageRecord>> {
        let result = sqlx::query("UPDATE images SET status = 'pending' WHERE sha256 = ?")
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

    async fn load_plan_rows(&self, start: LocalDate, end: LocalDate) -> Result<Vec<PlanRow>> {
        let rows = sqlx::query_as::<_, PlanRow>(
            "SELECT date, caption, image
             FROM plans
             WHERE (date >= ? AND date <= ?)
                OR date = (SELECT MAX(date) FROM plans WHERE date < ?)
             ORDER BY date ASC",
        )
        .bind(start.to_string())
        .bind(end.to_string())
        .bind(start.to_string())
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    async fn get_admin_plan(&self, date: LocalDate) -> Result<Option<Plan>> {
        let row =
            sqlx::query_as::<_, PlanRow>("SELECT date, caption, image FROM plans WHERE date = ?")
                .bind(date.to_string())
                .fetch_optional(&self.pool)
                .await?;

        row.map(plan_from_row).transpose()
    }

    async fn validate_image(&self, sha256: &str) -> Result<()> {
        if sha256.is_empty() {
            return Ok(());
        }

        let exists: Option<i64> =
            sqlx::query_scalar("SELECT 1 FROM images WHERE sha256 = ? LIMIT 1")
                .bind(sha256)
                .fetch_optional(&self.pool)
                .await?;
        if exists.is_none() {
            return Err(anyhow!("Unknown image sha256: {sha256}"));
        }
        Ok(())
    }
}

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
            image         TEXT NOT NULL DEFAULT ''
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

fn plan_from_row(row: PlanRow) -> Result<Plan> {
    Ok(Plan {
        date: LocalDate::parse(&row.date)
            .map_err(|_| anyhow!("Stored plan date is invalid: {}", row.date))?,
        caption: row.caption,
        image: row.image,
    })
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
