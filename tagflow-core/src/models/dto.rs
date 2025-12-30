use serde::{Deserialize, Serialize};
use crate::models::db::{FileEntry, Library};
use chrono::{DateTime, Utc};

#[derive(Serialize, Debug)]
pub struct TagNode {
    pub id: i32,
    pub name: String,
    pub category: String,
    pub children: Vec<TagNode>,
}

#[derive(Serialize, Debug)]
pub struct FileResponse {
    pub items: Vec<FileItem>,
    pub total: i64,
}

#[derive(Serialize, Debug)]
pub struct FileItem {
    pub id: i32,
    pub filename: String,
    pub extension: Option<String>,
    pub size: i64,
    pub mtime: i64,
    pub parent_path: String,
}

#[derive(Deserialize, Debug)]
pub struct FileQuery {
    pub tag_id: Option<i32>,
    pub recursive: Option<bool>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
}

impl From<FileEntry> for FileItem {
    fn from(entry: FileEntry) -> Self {
        FileItem {
            id: entry.id,
            filename: entry.filename,
            extension: entry.extension,
            size: entry.size,
            mtime: entry.mtime,
            parent_path: entry.parent_path,
        }
    }
}

// ========== Library 相关 DTO ==========

/// 创建资源库请求
#[derive(Deserialize, Debug)]
pub struct CreateLibraryRequest {
    pub name: String,
    pub protocol: String,
    pub base_path: String,
    pub config_json: Option<String>,
}

/// 资源库响应
#[derive(Serialize, Debug)]
pub struct LibraryResponse {
    pub id: i32,
    pub name: String,
    pub protocol: String,
    pub base_path: String,
    pub last_scanned_at: Option<DateTime<Utc>>,
}

impl From<Library> for LibraryResponse {
    fn from(lib: Library) -> Self {
        LibraryResponse {
            id: lib.id,
            name: lib.name,
            protocol: lib.protocol,
            base_path: lib.base_path,
            last_scanned_at: lib.last_scanned_at,
        }
    }
}

/// 连接测试结果
#[derive(Serialize)]
pub struct TestConnectionResponse {
    pub reachable: bool,
    pub message: String,
}
