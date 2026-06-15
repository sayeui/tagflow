use crate::core::tag::TagManager;

/// 路径分词标签生成器。
///
/// 将文件的父目录路径按 `/` 拆分为层级标签，
/// 关联到最深一层（叶子）标签。查询时通过递归 CTE 可命中任意中间层。
pub struct PathTagger {
    tag_manager: TagManager,
}

impl PathTagger {
    pub fn new(tag_manager: TagManager) -> Self {
        Self { tag_manager }
    }

    /// 处理文件的路径标签。`parent_path` 形如 `Projects/2024/Design/`。
    pub async fn process(&self, file_id: i32, parent_path: &str) -> anyhow::Result<()> {
        let parts: Vec<String> = parent_path
            .split('/')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();

        if parts.is_empty() {
            return Ok(());
        }

        let leaf_tag_id = self.tag_manager.ensure_path_tags(parts).await?;
        self.tag_manager
            .link_file_to_tag(file_id, leaf_tag_id, "auto")
            .await?;
        Ok(())
    }
}
