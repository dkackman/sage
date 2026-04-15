use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Result as AnyResult};
use sqlx::{
    Connection, Executor, Row, SqliteConnection,
    sqlite::{SqliteConnectOptions, SqliteJournalMode},
};

use crate::apps::types::InstalledSageApp;

use super::validate::{ensure_storage_permission, validate_name};

pub const STORAGE_DIR_NAME: &str = "storage";

pub fn app_storage_root(install_dir: &Path) -> PathBuf {
    install_dir.join(STORAGE_DIR_NAME)
}

pub fn database_path(app: &InstalledSageApp, db_name: &str) -> AnyResult<PathBuf> {
    validate_name(db_name, "database name")?;
    let install_dir = PathBuf::from(&app.install_dir);
    let root = app_storage_root(&install_dir);
    Ok(root.join(format!("{db_name}.sqlite3")))
}

pub async fn open_connection(
    app: &InstalledSageApp,
    db_name: &str,
) -> AnyResult<SqliteConnection> {
    ensure_storage_permission(app)?;

    let db_path = database_path(app, db_name)?;
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let options = SqliteConnectOptions::new()
        .filename(&db_path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true);

    let mut conn = SqliteConnection::connect_with(&options).await?;
    initialize_schema(&mut conn).await?;
    Ok(conn)
}

pub async fn initialize_schema(conn: &mut SqliteConnection) -> AnyResult<()> {
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS sage_meta (
          key TEXT PRIMARY KEY NOT NULL,
          value_text TEXT
        )
        "#,
    )
        .await?;

    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS sage_object_stores (
          store_name TEXT PRIMARY KEY NOT NULL
        )
        "#,
    )
        .await?;

    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS sage_indexes (
          store_name TEXT NOT NULL,
          index_name TEXT NOT NULL,
          PRIMARY KEY (store_name, index_name),
          FOREIGN KEY (store_name) REFERENCES sage_object_stores(store_name) ON DELETE CASCADE
        )
        "#,
    )
        .await?;

    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS sage_records (
          store_name TEXT NOT NULL,
          primary_key BLOB NOT NULL,
          value_blob BLOB NOT NULL,
          PRIMARY KEY (store_name, primary_key),
          FOREIGN KEY (store_name) REFERENCES sage_object_stores(store_name) ON DELETE CASCADE
        )
        "#,
    )
        .await?;

    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS sage_index_entries (
          store_name TEXT NOT NULL,
          index_name TEXT NOT NULL,
          index_key BLOB NOT NULL,
          primary_key BLOB NOT NULL,
          PRIMARY KEY (store_name, index_name, index_key, primary_key),
          FOREIGN KEY (store_name, index_name) REFERENCES sage_indexes(store_name, index_name) ON DELETE CASCADE,
          FOREIGN KEY (store_name, primary_key) REFERENCES sage_records(store_name, primary_key) ON DELETE CASCADE
        )
        "#,
    )
        .await?;

    Ok(())
}

pub async fn get_current_version(conn: &mut SqliteConnection) -> AnyResult<i64> {
    let row = sqlx::query("SELECT value_text FROM sage_meta WHERE key = 'db_version'")
        .fetch_optional(&mut *conn)
        .await?;

    let version = row
        .and_then(|row| row.try_get::<String, _>(0).ok())
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);

    Ok(version)
}

pub async fn set_current_version(
    conn: &mut SqliteConnection,
    version: i64,
) -> AnyResult<()> {
    sqlx::query(
        r#"
        INSERT INTO sage_meta (key, value_text)
        VALUES ('db_version', ?1)
        ON CONFLICT(key) DO UPDATE SET value_text = excluded.value_text
        "#,
    )
        .bind(version.to_string())
        .execute(&mut *conn)
        .await?;
    Ok(())
}

pub async fn ensure_store_exists(
    conn: &mut SqliteConnection,
    store_name: &str,
) -> AnyResult<()> {
    let exists = sqlx::query("SELECT 1 FROM sage_object_stores WHERE store_name = ?1")
        .bind(store_name)
        .fetch_optional(&mut *conn)
        .await?
        .is_some();

    if !exists {
        return Err(anyhow!("object store does not exist: {}", store_name));
    }

    Ok(())
}
