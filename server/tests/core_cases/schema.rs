use epaper_album_server::db::{self, Store};
use sqlx::sqlite::SqlitePoolOptions;

use super::common::*;

#[tokio::test]
async fn init_schema_replaces_incompatible_legacy_tables() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect in-memory sqlite");

    sqlx::query("CREATE TABLE images (sha256 TEXT PRIMARY KEY, content_type TEXT NOT NULL, bytes BLOB NOT NULL)")
        .execute(&pool)
        .await
        .expect("create legacy images");
    sqlx::query("CREATE TABLE plans (caption TEXT NOT NULL)")
        .execute(&pool)
        .await
        .expect("create legacy plans");

    db::init_schema(&pool).await.expect("init schema");

    sqlx::query("INSERT INTO images (sha256, status, remark) VALUES (?, 'pending', '')")
        .bind(valid_sha(14))
        .execute(&pool)
        .await
        .expect("insert current image");
    sqlx::query("INSERT INTO plans (date, caption, image) VALUES (?, ?, ?)")
        .bind("2026-06-06")
        .bind("current")
        .bind(valid_sha(14))
        .execute(&pool)
        .await
        .expect("insert current plan");
}

#[tokio::test]
async fn init_schema_replaces_legacy_plan_foreign_key_table() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect in-memory sqlite");

    sqlx::query(
        r#"
        CREATE TABLE images (
            sha256 TEXT PRIMARY KEY,
            status TEXT NOT NULL,
            remark TEXT NOT NULL DEFAULT ''
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("create current images");
    sqlx::query(
        r#"
        CREATE TABLE plans (
            date TEXT PRIMARY KEY,
            caption TEXT NOT NULL,
            image TEXT NOT NULL,
            FOREIGN KEY (image) REFERENCES images(sha256)
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("create legacy plans");

    db::init_schema(&pool).await.expect("init schema");

    sqlx::query("INSERT INTO plans (date, caption, image) VALUES (?, ?, '')")
        .bind("2026-06-06")
        .bind("empty image")
        .execute(&pool)
        .await
        .expect("insert empty image plan");
}

#[tokio::test]
async fn plan_rows_with_invalid_dates_return_errors_without_panicking() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect in-memory sqlite");
    sqlx::query(
        r#"
        CREATE TABLE plans (
            date TEXT PRIMARY KEY,
            caption TEXT NOT NULL,
            type TEXT NOT NULL DEFAULT 'fixed',
            image TEXT NOT NULL,
            tags TEXT NOT NULL DEFAULT '[]'
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("create legacy plans");
    sqlx::query(
        r#"
        CREATE TABLE images (
            sha256 TEXT PRIMARY KEY,
            status TEXT NOT NULL CHECK (status IN ('pending', 'processing', 'ready', 'failed')),
            remark TEXT NOT NULL DEFAULT ''
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("create images");
    let sha = valid_sha(61);
    seed_image(&pool, &sha, "ready", "").await;
    sqlx::query("INSERT INTO plans (date, caption, image) VALUES (?, ?, ?)")
        .bind("2026-06-99")
        .bind("bad date")
        .bind(&sha)
        .execute(&pool)
        .await
        .expect("insert dirty plan");

    let store = Store::new(pool);
    let result = store
        .list_admin_plans(
            protocol::LocalDate::parse("2026-01-01").expect("start"),
            protocol::LocalDate::parse("2026-12-31").expect("end"),
        )
        .await;

    assert!(result.is_err());
    assert!(result
        .expect_err("dirty date should return an error")
        .to_string()
        .contains("Stored plan date is invalid"));
}
