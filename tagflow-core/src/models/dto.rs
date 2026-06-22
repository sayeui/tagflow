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

/// 文件详情面板展示的单个标签信息（id + 名称 + 类别 + 来源）。
///
/// `source` 区分 `auto`（扫描器自动打）/ `manual`（用户手动打），
/// 前端据此决定是否在 chip 上显示「×」移除按钮（仅 manual 可移除）。
#[derive(Serialize, Debug)]
pub struct FileTagInfo {
    pub id: i32,
    pub name: String,
    pub category: String,
    pub source: String,
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

/// 添加手动标签请求体（`POST /api/v1/files/:id/tags`）。
///
/// `path` 用 `/` 分隔层级（如 `"项目/TagFlow"`），后端按段逐层建/复用
/// `category='user'` 节点，叶子挂到文件（`source='manual'`）。
#[derive(Deserialize, Debug)]
pub struct AddTagRequest {
    pub path: String,
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
    /// 文件名模糊匹配（`filename LIKE '%kw%'`，不区分 ASCII 大小写）。
    /// 与 tag_ids AND 组合；None/空字符串表示不过滤。
    #[serde(default)]
    pub keyword: Option<String>,
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
///
/// `scan_interval_secs` 来自全局配置（[`crate::infra::config::scan_interval_secs`]），
/// 非数据库字段——所有库共享同一间隔（参见 PRD「全局单定时器 + 全局 env 间隔」决策）。
/// 前端据此推算「预计下次扫描」= `last_scanned_at + scan_interval_secs`。
#[derive(Serialize, Debug)]
pub struct LibraryResponse {
    pub id: i32,
    pub name: String,
    pub protocol: String,
    pub base_path: String,
    pub last_scanned_at: Option<DateTime<Utc>>,
    pub scan_interval_secs: i64,
}

impl LibraryResponse {
    /// 从 DB 模型构造响应，附带全局扫描间隔。
    ///
    /// 不用 `From<Library>`：`scan_interval_secs` 不属于 `Library`，需显式传入，
    /// 避免在 `From` 实现里偷偷读环境变量造成隐式依赖。
    pub fn from_library(lib: Library, scan_interval_secs: i64) -> Self {
        LibraryResponse {
            id: lib.id,
            name: lib.name,
            protocol: lib.protocol,
            base_path: lib.base_path,
            last_scanned_at: lib.last_scanned_at,
            scan_interval_secs,
        }
    }
}

/// 连接测试结果
#[derive(Serialize)]
pub struct TestConnectionResponse {
    pub reachable: bool,
    pub message: String,
}
