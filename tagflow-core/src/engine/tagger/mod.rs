//! 自动标签生成流水线。每个 Tagger 负责一种维度的标签：
//!
//! - [`path_tagger::PathTagger`]：路径分词（category = `path`）
//! - [`extension_tagger::ExtensionTagger`]：扩展名（category = `ext`）
//! - [`type_tagger::TypeTagger`]：宏类型分桶（category = `type`）
//! - [`time_tagger::TimeTagger`]：年/月层级（category = `time`）
//!
//! Tagger 之间无依赖，scanner 在文件入库时按固定顺序依次调用。
pub mod extension_tagger;
pub mod path_tagger;
pub mod time_tagger;
pub mod type_map;
pub mod type_tagger;

pub use extension_tagger::ExtensionTagger;
pub use path_tagger::PathTagger;
pub use time_tagger::TimeTagger;
pub use type_tagger::TypeTagger;

use crate::core::tag::TagManager;

/// 当前 tagger 流水线版本。
///
/// - `1`：仅 PathTagger（M1–M9 历史数据）
/// - `2`：新增 ExtensionTagger / TypeTagger / TimeTagger
///
/// 启动时若 `app_meta.tagger_version` 低于此值，触发 [`crate::engine::backfill`]
/// 对存量文件补齐新维度标签。
pub const CURRENT_TAGGER_VERSION: i64 = 2;

/// 对单个文件运行全部自动 tagger（path/ext/type/time）。
///
/// 幂等：`link_file_to_tag` 用 `INSERT OR IGNORE`，重复运行不产生重复关联。
/// scanner 入库时与 backfill 回填时共用此入口，保证两路逻辑一致。
pub async fn run_all(
    tag_manager: &TagManager,
    file_id: i32,
    parent_path: &str,
    extension: Option<&str>,
    mtime: i64,
) -> anyhow::Result<()> {
    PathTagger::new(tag_manager.clone())
        .process(file_id, parent_path)
        .await?;
    ExtensionTagger::new(tag_manager.clone())
        .process(file_id, extension)
        .await?;
    TypeTagger::new(tag_manager.clone())
        .process(file_id, extension)
        .await?;
    TimeTagger::new(tag_manager.clone())
        .process(file_id, mtime)
        .await?;
    Ok(())
}
