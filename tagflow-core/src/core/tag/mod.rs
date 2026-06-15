use sqlx::SqlitePool;

/// 标签管理领域服务。SqlitePool 内部为 Arc，clone 廉价，
/// 允许多个 tagger 各自持有一份。
#[derive(Clone)]
pub struct TagManager {
    db: SqlitePool,
}

impl TagManager {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }

    /// 确保单个标签存在，返回其 id。
    ///
    /// category 取值：`path` / `type` / `ext` / `time` / `user`。
    /// 用 SELECT-then-INSERT 而非依赖 UNIQUE(name, parent_id)：
    /// SQLite 对 NULL parent_id 的 UNIQUE 视为 distinct，约束无法拦住
    /// 多个根级同名标签，必须在应用层先查后插。
    pub async fn ensure_tag(
        &self,
        name: &str,
        category: &str,
        parent_id: Option<i32>,
    ) -> anyhow::Result<i32> {
        let row: Option<(i32,)> = sqlx::query_as(
            "SELECT id FROM tags \
             WHERE name = ? AND category = ? \
             AND (parent_id = ? OR (parent_id IS NULL AND ? IS NULL))",
        )
        .bind(name)
        .bind(category)
        .bind(parent_id)
        .bind(parent_id)
        .fetch_optional(&self.db)
        .await?;

        if let Some((id,)) = row {
            return Ok(id);
        }

        let res = sqlx::query("INSERT INTO tags (name, category, parent_id) VALUES (?, ?, ?)")
            .bind(name)
            .bind(category)
            .bind(parent_id)
            .execute(&self.db)
            .await?;
        Ok(res.last_insert_rowid() as i32)
    }

    /// 确保一个层级标签路径存在。例如输入 ["Work", "Design", "2025"]
    /// 返回最后一个标签 ("2025") 的 ID。
    pub async fn ensure_path_tags(&self, parts: Vec<String>) -> anyhow::Result<i32> {
        let mut last_parent_id: Option<i32> = None;

        for part in parts {
            if part.is_empty() {
                continue;
            }
            last_parent_id = Some(self.ensure_tag(&part, "path", last_parent_id).await?);
        }

        last_parent_id.ok_or_else(|| anyhow::anyhow!("路径为空，无法生成标签"))
    }

    /// 建立文件与标签的关联
    pub async fn link_file_to_tag(
        &self,
        file_id: i32,
        tag_id: i32,
        source: &str,
    ) -> anyhow::Result<()> {
        sqlx::query("INSERT OR IGNORE INTO file_tags (file_id, tag_id, source) VALUES (?, ?, ?)")
            .bind(file_id)
            .bind(tag_id)
            .bind(source)
            .execute(&self.db)
            .await?;
        Ok(())
    }
}
