use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SageStorageDatabaseInfo {
    pub name: String,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SageStorageObjectStoreInfo {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SageStorageIndexInfo {
    pub store: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SageStorageDatabaseDescription {
    pub name: String,
    pub version: i64,
    pub stores: Vec<SageStorageObjectStoreInfo>,
    pub indexes: Vec<SageStorageIndexInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SageStorageOpenDatabaseRequest {
    pub name: String,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SageStorageCreateObjectStoreRequest {
    #[serde(rename = "dbName", alias = "db_name")]
    pub db_name: String,
    #[serde(rename = "storeName", alias = "store_name")]
    pub store_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SageStorageCreateIndexRequest {
    #[serde(rename = "dbName", alias = "db_name")]
    pub db_name: String,
    #[serde(rename = "storeName", alias = "store_name")]
    pub store_name: String,
    #[serde(rename = "indexName", alias = "index_name")]
    pub index_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SageStorageGetRequest {
    #[serde(rename = "dbName", alias = "db_name")]
    pub db_name: String,
    #[serde(rename = "storeName", alias = "store_name")]
    pub store_name: String,
    #[serde(rename = "keyBase64", alias = "key_base64")]
    pub key_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SageStoragePutRequest {
    #[serde(rename = "dbName", alias = "db_name")]
    pub db_name: String,
    #[serde(rename = "storeName", alias = "store_name")]
    pub store_name: String,
    #[serde(rename = "keyBase64", alias = "key_base64")]
    pub key_base64: String,
    #[serde(rename = "valueBase64", alias = "value_base64")]
    pub value_base64: String,
    #[serde(rename = "indexValues", alias = "index_values", default)]
    pub index_values: Vec<SageStorageIndexValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SageStorageIndexValue {
    #[serde(rename = "indexName", alias = "index_name")]
    pub index_name: String,
    #[serde(rename = "keyBase64", alias = "key_base64")]
    pub key_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SageStorageDeleteRequest {
    #[serde(rename = "dbName", alias = "db_name")]
    pub db_name: String,
    #[serde(rename = "storeName", alias = "store_name")]
    pub store_name: String,
    #[serde(rename = "keyBase64", alias = "key_base64")]
    pub key_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SageStorageClearRequest {
    #[serde(rename = "dbName", alias = "db_name")]
    pub db_name: String,
    #[serde(rename = "storeName", alias = "store_name")]
    pub store_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SageStorageCountRequest {
    #[serde(rename = "dbName", alias = "db_name")]
    pub db_name: String,
    #[serde(rename = "storeName", alias = "store_name")]
    pub store_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SageStorageGetAllRequest {
    #[serde(rename = "dbName", alias = "db_name")]
    pub db_name: String,
    #[serde(rename = "storeName", alias = "store_name")]
    pub store_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SageStorageGetAllFromIndexRequest {
    #[serde(rename = "dbName", alias = "db_name")]
    pub db_name: String,
    #[serde(rename = "storeName", alias = "store_name")]
    pub store_name: String,
    #[serde(rename = "indexName", alias = "index_name")]
    pub index_name: String,
    #[serde(rename = "keyBase64", alias = "key_base64")]
    pub key_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SageStorageValueRecord {
    #[serde(rename = "keyBase64", alias = "key_base64")]
    pub key_base64: String,
    #[serde(rename = "valueBase64", alias = "value_base64")]
    pub value_base64: String,
}
