use crate::models::db::{FileEntry, Library};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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

/// 文件详情面板展示的单个标签信息（id + 名称 + 类别）。
#[derive(Serialize, Debug)]
pub struct FileTagInfo {
    pub id: i32,
    pub name: String,
    pub category: String,
}

/// 文件详情（`GET /api/v1/files/:id`）：完整元数据 + 该文件的全部标签。
///
/// 与列表 [`FileItem`] 分离，避免列表查询每行多一次 file_tags join。
#[derive(Serialize, Debug)]
pub struct FileDetail {
    pub id: i32,
    pub filename: String,
    pub extension: Option<String>,
    pub size: i64,
    pub mtime: i64,
    pub parent_path: String,
    pub tags: Vec<FileTagInfo>,
}

#[derive(Deserialize, Debug)]
pub struct FileQuery {
    /// 多标签 AND 过滤，逗号分隔（`tag_ids=3,7`）。
    /// 选用逗号而非重复 key：axum 的 `serde_urlencoded` 不支持重复 key 反序列化成 Vec。
    /// 空或缺失表示不过滤。兼容旧前端 `tag_id` 单值（并入此集合）。
    #[serde(default, deserialize_with = "deserialize_csv_i32")]
    pub tag_ids: Vec<i32>,
    /// 旧版单标签参数（向后兼容，并入 `tag_ids`）。
    pub tag_id: Option<i32>,
    pub recursive: Option<bool>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
}

/// 反序列化逗号分隔的 i32 列表（`"3,7,12"` → `vec![3,7,12]`，`""` → `vec![]`）。
fn deserialize_csv_i32<'de, D>(deserializer: D) -> Result<Vec<i32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let raw = Option::<String>::deserialize(deserializer)?;
    match raw {
        None => Ok(vec![]),
        Some(s) if s.trim().is_empty() => Ok(vec![]),
        Some(s) => s
            .split(',')
            .map(|x| x.trim().parse::<i32>().map_err(serde::de::Error::custom))
            .collect(),
    }
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
