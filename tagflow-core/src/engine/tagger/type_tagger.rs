use crate::core::tag::TagManager;
use crate::engine::tagger::type_map::ext_to_type;

/// 宏类型标签生成器，按 [`type_map`] 把扩展名映射为 `#type:<bucket>`（category = `type`）。
///
/// 未知扩展名跳过，避免 `#type:other` 兜底桶膨胀。
pub struct TypeTagger {
    tag_manager: TagManager,
}

impl TypeTagger {
    pub fn new(tag_manager: TagManager) -> Self {
        Self { tag_manager }
    }

    pub async fn process(&self, file_id: i32, extension: Option<&str>) -> anyhow::Result<()> {
        let Some(ext) = extension else {
            return Ok(());
        };
        let Some(type_name) = ext_to_type(ext) else {
            return Ok(());
        };

        let tag_id = self.tag_manager.ensure_tag(type_name, "type", None).await?;
        self.tag_manager
            .link_file_to_tag(file_id, tag_id, "auto")
            .await?;
        Ok(())
    }
}
