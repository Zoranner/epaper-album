use std::collections::HashSet;

use anyhow::{anyhow, Result};
use chrono::Utc;
use protocol::{Image, ImageStatus, LocalDate, Plan, PlanType};
use sqlx::SqlitePool;

#[derive(Debug, Clone)]
pub struct Store {
    pool: SqlitePool,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct PlanRow {
    date: String,
    caption: String,
    plan_type: String,
    image: String,
    tags: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct ImageRow {
    sha256: String,
    status: String,
    remark: String,
    created_at: String,
    updated_at: String,
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
            let mut plan = plan_from_row(row)?;
            match plan.plan_type {
                PlanType::Fixed => {
                    let status: Option<String> =
                        sqlx::query_scalar("SELECT status FROM images WHERE sha256 = ?")
                            .bind(&plan.image)
                            .fetch_optional(&self.pool)
                            .await?;
                    if status.as_deref() == Some("ready") {
                        plan.tags.clear();
                        plans.push(plan);
                    }
                }
                PlanType::Random => {
                    if let Some(image) = self.random_ready_image_for_tags(&plan.tags).await? {
                        plan.image = image;
                        plan.tags.clear();
                        plan.plan_type = PlanType::Fixed;
                        plans.push(plan);
                    }
                }
            }
        }

        Ok(plans)
    }

    pub async fn create_plan(&self, payload: Plan) -> Result<Plan> {
        let payload = normalize_plan(payload);
        self.validate_plan_selection(&payload).await?;
        let result = sqlx::query(
            "INSERT INTO plans (date, caption, type, image, tags) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(payload.date.to_string())
        .bind(payload.caption.trim())
        .bind(plan_type_str(&payload.plan_type))
        .bind(&payload.image)
        .bind(serde_json::to_string(&payload.tags)?)
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
        let payload = normalize_plan(payload);
        self.validate_plan_selection(&payload).await?;
        let result = sqlx::query(
            "UPDATE plans SET date = ?, caption = ?, type = ?, image = ?, tags = ? WHERE date = ?",
        )
        .bind(payload.date.to_string())
        .bind(payload.caption.trim())
        .bind(plan_type_str(&payload.plan_type))
        .bind(&payload.image)
        .bind(serde_json::to_string(&payload.tags)?)
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

    pub async fn list_images(&self, keyword: Option<&str>, tags: &[String]) -> Result<Vec<Image>> {
        let rows = if let Some(keyword) = keyword.filter(|value| !value.is_empty()) {
            sqlx::query_as::<_, ImageRow>(
                "SELECT sha256, status, remark, created_at, updated_at FROM images
                 WHERE remark LIKE '%' || ? || '%'
                 ORDER BY sha256 ASC",
            )
            .bind(keyword)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, ImageRow>(
                "SELECT sha256, status, remark, created_at, updated_at FROM images ORDER BY sha256 ASC",
            )
            .fetch_all(&self.pool)
            .await?
        };

        let mut images = self.images_from_rows(rows).await?;
        if !tags.is_empty() {
            images.retain(|image| tags.iter().all(|tag| image.tags.contains(tag)));
        }
        Ok(images)
    }

    pub async fn get_image(&self, sha256: &str) -> Result<Option<Image>> {
        let row = sqlx::query_as::<_, ImageRow>(
            "SELECT sha256, status, remark, created_at, updated_at FROM images WHERE sha256 = ?",
        )
        .bind(sha256)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            Ok(Some(self.image_from_row(row).await?))
        } else {
            Ok(None)
        }
    }

    pub async fn upsert_uploaded_image(
        &self,
        sha256: &str,
        remark: Option<&str>,
        tags: Option<&[String]>,
    ) -> Result<(Image, bool)> {
        let existing = self.get_image(sha256).await?;

        match existing {
            Some(image) => {
                let mut next_status = image.status;
                let mut should_enqueue =
                    matches!(image.status, ImageStatus::Pending | ImageStatus::Processing);
                if image.status == ImageStatus::Failed {
                    next_status = ImageStatus::Pending;
                    should_enqueue = true;
                }

                if let Some(remark) = remark {
                    sqlx::query(
                        "UPDATE images SET status = ?, remark = ?, updated_at = ? WHERE sha256 = ?",
                    )
                    .bind(image_status_str(&next_status))
                    .bind(remark)
                    .bind(now_string())
                    .bind(sha256)
                    .execute(&self.pool)
                    .await?;
                } else if next_status != image.status {
                    sqlx::query("UPDATE images SET status = ?, updated_at = ? WHERE sha256 = ?")
                        .bind(image_status_str(&next_status))
                        .bind(now_string())
                        .bind(sha256)
                        .execute(&self.pool)
                        .await?;
                }
                if let Some(tags) = tags {
                    self.replace_image_tags(sha256, tags, true).await?;
                }

                let updated = self
                    .get_image(sha256)
                    .await?
                    .ok_or_else(|| anyhow!("updated image not found"))?;
                Ok((updated, should_enqueue))
            }
            None => {
                let remark = remark.unwrap_or_default();
                let now = now_string();
                sqlx::query(
                    "INSERT INTO images (sha256, status, remark, created_at, updated_at) VALUES (?, 'pending', ?, ?, ?)",
                )
                    .bind(sha256)
                    .bind(remark)
                    .bind(&now)
                    .bind(&now)
                    .execute(&self.pool)
                    .await?;
                if let Some(tags) = tags {
                    self.replace_image_tags(sha256, tags, false).await?;
                }
                let image = self
                    .get_image(sha256)
                    .await?
                    .ok_or_else(|| anyhow!("inserted image not found"))?;
                Ok((image, true))
            }
        }
    }

    pub async fn update_image(
        &self,
        sha256: &str,
        remark: &str,
        tags: &[String],
    ) -> Result<Option<Image>> {
        let result = sqlx::query("UPDATE images SET remark = ?, updated_at = ? WHERE sha256 = ?")
            .bind(remark)
            .bind(now_string())
            .bind(sha256)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Ok(None);
        }
        self.replace_image_tags(sha256, tags, true).await?;
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

    pub async fn requeue_image(&self, sha256: &str) -> Result<Option<Image>> {
        let result =
            sqlx::query("UPDATE images SET status = 'pending', updated_at = ? WHERE sha256 = ?")
                .bind(now_string())
                .bind(sha256)
                .execute(&self.pool)
                .await?;
        if result.rows_affected() == 0 {
            return Ok(None);
        }

        self.get_image(sha256).await
    }

    pub async fn recover_processing_images(&self) -> Result<()> {
        sqlx::query(
            "UPDATE images SET status = 'pending', updated_at = ? WHERE status = 'processing'",
        )
        .bind(now_string())
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
                "UPDATE images SET status = 'pending', updated_at = ? WHERE sha256 = ? AND status = 'ready'",
            )
            .bind(now_string())
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
            "UPDATE images SET status = 'processing', updated_at = ? WHERE sha256 = ? AND status = 'pending'",
        )
        .bind(now_string())
        .bind(sha256)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn mark_ready(&self, sha256: &str) -> Result<()> {
        sqlx::query("UPDATE images SET status = 'ready', updated_at = ? WHERE sha256 = ?")
            .bind(now_string())
            .bind(sha256)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn mark_failed(&self, sha256: &str) -> Result<()> {
        sqlx::query("UPDATE images SET status = 'failed', updated_at = ? WHERE sha256 = ?")
            .bind(now_string())
            .bind(sha256)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn images_from_rows(&self, rows: Vec<ImageRow>) -> Result<Vec<Image>> {
        let mut images = Vec::with_capacity(rows.len());
        for row in rows {
            images.push(self.image_from_row(row).await?);
        }
        Ok(images)
    }

    async fn image_from_row(&self, row: ImageRow) -> Result<Image> {
        Ok(Image {
            tags: self.image_tags(&row.sha256).await?,
            status: image_status_from_str(&row.status)?,
            sha256: row.sha256,
            remark: row.remark,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    async fn image_tags(&self, sha256: &str) -> Result<Vec<String>> {
        let tags = sqlx::query_scalar::<_, String>(
            "SELECT tag FROM image_tags WHERE image = ? ORDER BY tag ASC",
        )
        .bind(sha256)
        .fetch_all(&self.pool)
        .await?;
        Ok(tags)
    }

    async fn replace_image_tags(
        &self,
        sha256: &str,
        tags: &[String],
        touch_updated_at: bool,
    ) -> Result<()> {
        let mut transaction = self.pool.begin().await?;
        sqlx::query("DELETE FROM image_tags WHERE image = ?")
            .bind(sha256)
            .execute(&mut *transaction)
            .await?;
        for tag in normalized_tags(tags) {
            sqlx::query("INSERT INTO image_tags (image, tag) VALUES (?, ?)")
                .bind(sha256)
                .bind(tag)
                .execute(&mut *transaction)
                .await?;
        }
        if touch_updated_at {
            sqlx::query("UPDATE images SET updated_at = ? WHERE sha256 = ?")
                .bind(now_string())
                .bind(sha256)
                .execute(&mut *transaction)
                .await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    async fn random_ready_image_for_tags(&self, tags: &[String]) -> Result<Option<String>> {
        let tags = normalized_tags(tags);
        if tags.is_empty() {
            return Ok(None);
        }

        let placeholders = std::iter::repeat_n("?", tags.len())
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT i.sha256
             FROM images AS i
             JOIN image_tags AS it ON it.image = i.sha256
             WHERE i.status = 'ready' AND it.tag IN ({placeholders})
             GROUP BY i.sha256
             HAVING COUNT(DISTINCT it.tag) = ?
             ORDER BY RANDOM()
             LIMIT 1"
        );
        let mut query = sqlx::query_scalar::<_, String>(&sql);
        for tag in &tags {
            query = query.bind(tag);
        }
        query = query.bind(tags.len() as i64);
        Ok(query.fetch_optional(&self.pool).await?)
    }

    async fn load_plan_rows(&self, start: LocalDate, end: LocalDate) -> Result<Vec<PlanRow>> {
        let rows = sqlx::query_as::<_, PlanRow>(
            "SELECT date, caption, type AS plan_type, image, tags
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
        let row = sqlx::query_as::<_, PlanRow>(
            "SELECT date, caption, type AS plan_type, image, tags FROM plans WHERE date = ?",
        )
        .bind(date.to_string())
        .fetch_optional(&self.pool)
        .await?;

        row.map(plan_from_row).transpose()
    }

    async fn validate_plan_selection(&self, payload: &Plan) -> Result<()> {
        match payload.plan_type {
            PlanType::Fixed => {
                if !payload.image.is_empty() {
                    self.validate_image(&payload.image).await?;
                }
            }
            PlanType::Random => {
                if normalized_tags(&payload.tags).is_empty() {
                    return Err(anyhow!("Random plan tags are empty"));
                }
            }
        }
        Ok(())
    }

    async fn validate_image(&self, sha256: &str) -> Result<()> {
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

fn plan_from_row(row: PlanRow) -> Result<Plan> {
    Ok(Plan {
        date: LocalDate::parse(&row.date)
            .map_err(|_| anyhow!("Stored plan date is invalid: {}", row.date))?,
        caption: row.caption,
        plan_type: match row.plan_type.as_str() {
            "fixed" => PlanType::Fixed,
            "random" => PlanType::Random,
            _ => return Err(anyhow!("Stored plan type is invalid: {}", row.plan_type)),
        },
        image: row.image,
        tags: serde_json::from_str(&row.tags)
            .map_err(|_| anyhow!("Stored plan tags are invalid: {}", row.tags))?,
    })
}

fn plan_type_str(plan_type: &PlanType) -> &'static str {
    match plan_type {
        PlanType::Fixed => "fixed",
        PlanType::Random => "random",
    }
}

fn image_status_from_str(value: &str) -> Result<ImageStatus> {
    match value {
        "pending" => Ok(ImageStatus::Pending),
        "processing" => Ok(ImageStatus::Processing),
        "ready" => Ok(ImageStatus::Ready),
        "failed" => Ok(ImageStatus::Failed),
        _ => Err(anyhow!("Stored image status is invalid: {value}")),
    }
}

fn image_status_str(status: &ImageStatus) -> &'static str {
    match status {
        ImageStatus::Pending => "pending",
        ImageStatus::Processing => "processing",
        ImageStatus::Ready => "ready",
        ImageStatus::Failed => "failed",
    }
}

fn now_string() -> String {
    Utc::now().to_rfc3339()
}

fn normalize_plan(mut plan: Plan) -> Plan {
    plan.caption = plan.caption.trim().to_string();
    plan.tags = normalized_tags(&plan.tags);
    match plan.plan_type {
        PlanType::Fixed => plan.tags.clear(),
        PlanType::Random => plan.image.clear(),
    }
    plan
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

fn normalized_tags(tags: &[String]) -> Vec<String> {
    let mut normalized = Vec::new();
    for tag in tags {
        let tag = tag.trim();
        if !tag.is_empty() && !normalized.iter().any(|item| item == tag) {
            normalized.push(tag.to_string());
        }
    }
    normalized
}
