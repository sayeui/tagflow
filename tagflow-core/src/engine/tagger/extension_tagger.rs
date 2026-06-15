use crate::core::tag::TagManager;

/// 扩展名标签生成器，产出 `#ext:<ext>`（category = `ext`）。
///
/// 无扩展名或空扩展名时跳过。输入扩展名约定已小写。
pub struct ExtensionTagger {
    tag_manager: TagManager,
}

impl ExtensionTagger {
    pub fn new(tag_manager: TagManager) -> Self {
        Self { tag_manager }
    }

    pub async fn process(&self, file_id: i32, extension: Option<&str>) -> anyhow::Result<()> {
        let Some(ext) = extension else {
            return Ok(());
        };
        if ext.is_empty() {
            return Ok(());
        }

        let tag_id = self.tag_manager.ensure_tag(ext, "ext", None).await?;
        self.tag_manager
            .link_file_to_tag(file_id, tag_id, "auto")
            .await?;
        Ok(())
    }
}
