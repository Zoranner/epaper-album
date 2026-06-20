use anyhow::{anyhow, Result};
use protocol::{LocalDate, Plan, PlanType};

use super::{normalize_plan, normalized_tags, plan_type_str, Store};

#[derive(Debug, Clone, sqlx::FromRow)]
struct PlanRow {
    date: String,
    caption: String,
    plan_type: String,
    image: String,
    tags: String,
}

impl Store {
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
