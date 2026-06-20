use anyhow::{anyhow, Result};
use protocol::{Image, ImageStatus};

use super::{image_status_from_str, image_status_str, normalized_tags, now_string, Store};

#[derive(Debug, Clone, sqlx::FromRow)]
struct ImageRow {
    sha256: String,
    status: String,
    remark: String,
    created_at: String,
    updated_at: String,
}

impl Store {
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

    pub(super) async fn random_ready_image_for_tags(
        &self,
        tags: &[String],
    ) -> Result<Option<String>> {
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
}
