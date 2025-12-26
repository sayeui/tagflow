
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Library {
    pub id: i32,
    pub name: String,
    pub protocol: String,
    pub base_path: String,
    pub config_json: Option<String>,
    pub last_scanned_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Tag {
    pub id: i32,
    pub name: String,
    pub category: String,
    pub parent_id: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct FileEntry {
    pub id: i32,
    pub library_id: i32,
    pub parent_path: String,
    pub filename: String,
    pub extension: Option<String>,
    pub size: i64,
    pub mtime: i64,
    pub hash: Option<String>,
    pub status: i32,
    pub indexed_at: DateTime<Utc>,
}