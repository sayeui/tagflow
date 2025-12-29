use crate::core::tag::TagManager;

pub struct PathTagger {
    tag_manager: TagManager,
}

impl PathTagger {
    pub fn new(tag_manager: TagManager) -> Self {
        Self { tag_manager }
    }

    /// 处理文件的路径标签
    pub async fn process_path(&self, file_id: i32, parent_path: &str) -> anyhow::Result<()> {
        // 将 "Projects/2024/Design/" 拆分为 ["Projects", "2024", "Design"]
        let parts: Vec<String> = parent_path
            .split('/')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();

        if !parts.is_empty() {
            // 确保层级标签存在并获取叶子 ID
            let leaf_tag_id = self.tag_manager.ensure_path_tags(parts).await?;
            // 关联文件到叶子标签
            self.tag_manager.link_file_to_tag(file_id, leaf_tag_id, "auto").await?;
        }

        Ok(())
    }
}
