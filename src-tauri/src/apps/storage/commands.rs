use std::{
    fs,
    io,
    path::PathBuf,
};

use sqlx::{Connection, Row, SqliteConnection, sqlite::SqliteRow};
use tauri::{State, command};

use crate::{
    app_state::AppState,
    apps::registry::read_installed_app_by_id,
    error::Result,
};

use super::{
    db::{
        database_path, ensure_store_exists, get_current_version, open_connection,
        set_current_version,
    },
    types::*,
    validate::{decode_b64, encode_b64, ensure_storage_permission, validate_name},
};

async fn base_path_from_state(state: &State<'_, AppState>) -> PathBuf {
    let state = state.lock().await;
    state.path.clone()
}

async fn load_app(
    state: &State<'_, AppState>,
    app_id: &str,
) -> io::Result<crate::apps::types::InstalledSageApp> {
    let base_path = base_path_from_state(state).await;

    read_installed_app_by_id(&base_path, app_id)
        .map_err(|err| io::Error::other(format!("failed to read app {}: {err}", app_id)))
}

async fn open_app_db(
    state: &State<'_, AppState>,
    app_id: &str,
    db_name: &str,
) -> io::Result<SqliteConnection> {
    let app = load_app(state, app_id).await?;

    open_connection(&app, db_name)
        .await
        .map_err(|err| io::Error::other(format!("failed to open database: {err}")))
}

async fn open_app_db_with_store(
    state: &State<'_, AppState>,
    app_id: &str,
    db_name: &str,
    store_name: &str,
) -> io::Result<SqliteConnection> {
    let mut conn = open_app_db(state, app_id, db_name).await?;

    ensure_store_exists(&mut conn, store_name)
        .await
        .map_err(|err| io::Error::other(err.to_string()))?;

    Ok(conn)
}

fn row_to_value_record(row: SqliteRow) -> io::Result<SageStorageValueRecord> {
    let key: Vec<u8> = row
        .try_get(0)
        .map_err(|err| io::Error::other(err.to_string()))?;
    let value: Vec<u8> = row
        .try_get(1)
        .map_err(|err| io::Error::other(err.to_string()))?;

    Ok(SageStorageValueRecord {
        key_base64: encode_b64(&key),
        value_base64: encode_b64(&value),
    })
}

fn validate_db_name(db_name: &str) -> io::Result<()> {
    validate_name(db_name, "database name")
        .map_err(|err| io::Error::other(err.to_string()))
}

fn validate_store_name(store_name: &str) -> io::Result<()> {
    validate_name(store_name, "store name")
        .map_err(|err| io::Error::other(err.to_string()))
}

fn validate_index_name(index_name: &str) -> io::Result<()> {
    validate_name(index_name, "index name")
        .map_err(|err| io::Error::other(err.to_string()))
}

#[command]
#[specta::specta]
pub async fn storage_open_database(
    state: State<'_, AppState>,
    app_id: String,
    req: SageStorageOpenDatabaseRequest,
) -> Result<SageStorageDatabaseInfo> {
    validate_db_name(&req.name)?;

    let mut conn = open_app_db(&state, &app_id, &req.name).await?;

    let current = get_current_version(&mut conn)
        .await
        .map_err(|err| io::Error::other(format!("failed to read version: {err}")))?;

    if req.version < current {
        return Err(io::Error::other(format!(
            "requested version {} is older than current version {}",
            req.version, current
        ))
            .into());
    }

    if req.version > current {
        set_current_version(&mut conn, req.version)
            .await
            .map_err(|err| io::Error::other(format!("failed to set version: {err}")))?;
    }

    Ok(SageStorageDatabaseInfo {
        name: req.name,
        version: req.version.max(current),
    })
}

#[command]
#[specta::specta]
pub async fn storage_delete_database(
    state: State<'_, AppState>,
    app_id: String,
    db_name: String,
) -> Result<()> {
    validate_db_name(&db_name)?;

    let app = load_app(&state, &app_id).await?;

    ensure_storage_permission(&app)
        .map_err(|err| io::Error::other(err.to_string()))?;

    let path = database_path(&app, &db_name)
        .map_err(|err| io::Error::other(err.to_string()))?;

    let wal_path = PathBuf::from(format!("{}-wal", path.display()));
    let shm_path = PathBuf::from(format!("{}-shm", path.display()));

    for candidate in [&path, &wal_path, &shm_path] {
        if candidate.exists() {
            fs::remove_file(candidate).map_err(|err| {
                io::Error::other(format!(
                    "failed to remove {}: {err}",
                    candidate.display()
                ))
            })?;
        }
    }

    Ok(())
}

#[command]
#[specta::specta]
pub async fn storage_describe_database(
    state: State<'_, AppState>,
    app_id: String,
    db_name: String,
) -> Result<SageStorageDatabaseDescription> {
    validate_db_name(&db_name)?;

    let mut conn = open_app_db(&state, &app_id, &db_name).await?;

    let version = get_current_version(&mut conn)
        .await
        .map_err(|err| io::Error::other(format!("failed to read version: {err}")))?;

    let store_rows = sqlx::query(
        "SELECT store_name FROM sage_object_stores ORDER BY store_name ASC",
    )
        .fetch_all(&mut conn)
        .await
        .map_err(|err| io::Error::other(format!("failed to query stores: {err}")))?;

    let stores = store_rows
        .into_iter()
        .map(|row| -> io::Result<SageStorageObjectStoreInfo> {
            let name: String = row
                .try_get(0)
                .map_err(|err| io::Error::other(err.to_string()))?;
            Ok(SageStorageObjectStoreInfo { name })
        })
        .collect::<io::Result<Vec<_>>>()?;

    let index_rows = sqlx::query(
        "SELECT store_name, index_name FROM sage_indexes ORDER BY store_name ASC, index_name ASC",
    )
        .fetch_all(&mut conn)
        .await
        .map_err(|err| io::Error::other(format!("failed to query indexes: {err}")))?;

    let indexes = index_rows
        .into_iter()
        .map(|row| -> io::Result<SageStorageIndexInfo> {
            let store: String = row
                .try_get(0)
                .map_err(|err| io::Error::other(err.to_string()))?;
            let name: String = row
                .try_get(1)
                .map_err(|err| io::Error::other(err.to_string()))?;

            Ok(SageStorageIndexInfo { store, name })
        })
        .collect::<io::Result<Vec<_>>>()?;

    Ok(SageStorageDatabaseDescription {
        name: db_name,
        version,
        stores,
        indexes,
    })
}

#[command]
#[specta::specta]
pub async fn storage_create_object_store(
    state: State<'_, AppState>,
    app_id: String,
    req: SageStorageCreateObjectStoreRequest,
) -> Result<()> {
    validate_db_name(&req.db_name)?;
    validate_store_name(&req.store_name)?;

    let mut conn = open_app_db(&state, &app_id, &req.db_name).await?;

    sqlx::query("INSERT OR IGNORE INTO sage_object_stores (store_name) VALUES (?1)")
        .bind(req.store_name)
        .execute(&mut conn)
        .await
        .map_err(|err| io::Error::other(format!("failed to create store: {err}")))?;

    Ok(())
}

#[command]
#[specta::specta]
pub async fn storage_create_index(
    state: State<'_, AppState>,
    app_id: String,
    req: SageStorageCreateIndexRequest,
) -> Result<()> {
    validate_db_name(&req.db_name)?;
    validate_store_name(&req.store_name)?;
    validate_index_name(&req.index_name)?;

    let mut conn = open_app_db_with_store(&state, &app_id, &req.db_name, &req.store_name).await?;

    sqlx::query("INSERT OR IGNORE INTO sage_indexes (store_name, index_name) VALUES (?1, ?2)")
        .bind(req.store_name)
        .bind(req.index_name)
        .execute(&mut conn)
        .await
        .map_err(|err| io::Error::other(format!("failed to create index: {err}")))?;

    Ok(())
}

#[command]
#[specta::specta]
pub async fn storage_get(
    state: State<'_, AppState>,
    app_id: String,
    req: SageStorageGetRequest,
) -> Result<Option<String>> {
    let mut conn = open_app_db_with_store(&state, &app_id, &req.db_name, &req.store_name).await?;

    let key = decode_b64(&req.key_base64, "key")
        .map_err(|err| io::Error::other(err.to_string()))?;

    let row = sqlx::query(
        "SELECT value_blob FROM sage_records WHERE store_name = ?1 AND primary_key = ?2",
    )
        .bind(req.store_name)
        .bind(key)
        .fetch_optional(&mut conn)
        .await
        .map_err(|err| io::Error::other(format!("failed to read value: {err}")))?;

    let value = row
        .map(|row| row.try_get::<Vec<u8>, _>(0))
        .transpose()
        .map_err(|err| io::Error::other(format!("failed to decode value: {err}")))?;

    Ok(value.map(|bytes| encode_b64(&bytes)))
}

#[command]
#[specta::specta]
pub async fn storage_put(
    state: State<'_, AppState>,
    app_id: String,
    req: SageStoragePutRequest,
) -> Result<()> {
    let mut conn = open_app_db_with_store(&state, &app_id, &req.db_name, &req.store_name).await?;

    let key = decode_b64(&req.key_base64, "key")
        .map_err(|err| io::Error::other(err.to_string()))?;
    let value = decode_b64(&req.value_base64, "value")
        .map_err(|err| io::Error::other(err.to_string()))?;

    let mut tx = conn
        .begin()
        .await
        .map_err(|err| io::Error::other(format!("failed to start tx: {err}")))?;

    sqlx::query(
        r#"
        INSERT INTO sage_records (store_name, primary_key, value_blob)
        VALUES (?1, ?2, ?3)
        ON CONFLICT(store_name, primary_key) DO UPDATE SET value_blob = excluded.value_blob
        "#,
    )
        .bind(&req.store_name)
        .bind(&key)
        .bind(&value)
        .execute(&mut *tx)
        .await
        .map_err(|err| io::Error::other(format!("failed to write record: {err}")))?;

    sqlx::query("DELETE FROM sage_index_entries WHERE store_name = ?1 AND primary_key = ?2")
        .bind(&req.store_name)
        .bind(&key)
        .execute(&mut *tx)
        .await
        .map_err(|err| io::Error::other(format!("failed to clear old index entries: {err}")))?;

    for index_value in &req.index_values {
        validate_index_name(&index_value.index_name)?;

        let index_key = decode_b64(&index_value.key_base64, "index key")
            .map_err(|err| io::Error::other(err.to_string()))?;

        sqlx::query(
            r#"
            INSERT INTO sage_index_entries (store_name, index_name, index_key, primary_key)
            VALUES (?1, ?2, ?3, ?4)
            "#,
        )
            .bind(&req.store_name)
            .bind(&index_value.index_name)
            .bind(index_key)
            .bind(&key)
            .execute(&mut *tx)
            .await
            .map_err(|err| io::Error::other(format!("failed to write index entry: {err}")))?;
    }

    tx.commit()
        .await
        .map_err(|err| io::Error::other(format!("failed to commit tx: {err}")))?;

    Ok(())
}

#[command]
#[specta::specta]
pub async fn storage_delete(
    state: State<'_, AppState>,
    app_id: String,
    req: SageStorageDeleteRequest,
) -> Result<()> {
    let mut conn = open_app_db_with_store(&state, &app_id, &req.db_name, &req.store_name).await?;

    let key = decode_b64(&req.key_base64, "key")
        .map_err(|err| io::Error::other(err.to_string()))?;

    sqlx::query("DELETE FROM sage_records WHERE store_name = ?1 AND primary_key = ?2")
        .bind(req.store_name)
        .bind(key)
        .execute(&mut conn)
        .await
        .map_err(|err| io::Error::other(format!("failed to delete record: {err}")))?;

    Ok(())
}

#[command]
#[specta::specta]
pub async fn storage_clear(
    state: State<'_, AppState>,
    app_id: String,
    req: SageStorageClearRequest,
) -> Result<()> {
    let mut conn = open_app_db_with_store(&state, &app_id, &req.db_name, &req.store_name).await?;

    sqlx::query("DELETE FROM sage_records WHERE store_name = ?1")
        .bind(req.store_name)
        .execute(&mut conn)
        .await
        .map_err(|err| io::Error::other(format!("failed to clear store: {err}")))?;

    Ok(())
}

#[command]
#[specta::specta]
pub async fn storage_count(
    state: State<'_, AppState>,
    app_id: String,
    req: SageStorageCountRequest,
) -> Result<i64> {
    let mut conn = open_app_db_with_store(&state, &app_id, &req.db_name, &req.store_name).await?;

    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM sage_records WHERE store_name = ?1",
    )
        .bind(req.store_name)
        .fetch_one(&mut conn)
        .await
        .map_err(|err| io::Error::other(format!("failed to count records: {err}")))?;

    Ok(count)
}

#[command]
#[specta::specta]
pub async fn storage_get_all(
    state: State<'_, AppState>,
    app_id: String,
    req: SageStorageGetAllRequest,
) -> Result<Vec<SageStorageValueRecord>> {
    let mut conn = open_app_db_with_store(&state, &app_id, &req.db_name, &req.store_name).await?;

    let rows = sqlx::query(
        "SELECT primary_key, value_blob FROM sage_records WHERE store_name = ?1 ORDER BY primary_key ASC",
    )
        .bind(req.store_name)
        .fetch_all(&mut conn)
        .await
        .map_err(|err| io::Error::other(format!("failed to query records: {err}")))?;

    let out = rows
        .into_iter()
        .map(row_to_value_record)
        .collect::<io::Result<Vec<_>>>()?;

    Ok(out)
}

#[command]
#[specta::specta]
pub async fn storage_get_all_from_index(
    state: State<'_, AppState>,
    app_id: String,
    req: SageStorageGetAllFromIndexRequest,
) -> Result<Vec<SageStorageValueRecord>> {
    let mut conn = open_app_db_with_store(&state, &app_id, &req.db_name, &req.store_name).await?;

    let index_key = decode_b64(&req.key_base64, "index key")
        .map_err(|err| io::Error::other(err.to_string()))?;

    let rows = sqlx::query(
        r#"
        SELECT r.primary_key, r.value_blob
        FROM sage_index_entries i
        JOIN sage_records r
          ON r.store_name = i.store_name
         AND r.primary_key = i.primary_key
        WHERE i.store_name = ?1
          AND i.index_name = ?2
          AND i.index_key = ?3
        ORDER BY r.primary_key ASC
        "#,
    )
        .bind(req.store_name)
        .bind(req.index_name)
        .bind(index_key)
        .fetch_all(&mut conn)
        .await
        .map_err(|err| io::Error::other(format!("failed to query index: {err}")))?;

    let out = rows
        .into_iter()
        .map(row_to_value_record)
        .collect::<io::Result<Vec<_>>>()?;

    Ok(out)
}
