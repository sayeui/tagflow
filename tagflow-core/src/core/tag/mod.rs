use sqlx::SqlitePool;

pub struct TagManager {
    db: SqlitePool,
}

impl TagManager {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }

    /// 确保一个层级标签路径存在。例如输入 ["Work", "Design", "2025"]
    /// 返回最后一个标签 ("2025") 的 ID
    pub async fn ensure_path_tags(&self, parts: Vec<String>) -> anyhow::Result<i32> {
        let mut last_parent_id: Option<i32> = None;

        for part in parts {
            if part.is_empty() { continue; }

            // 尝试查找或插入当前层级的标签
            let row: Option<(i32,)> = sqlx::query_as(
                "SELECT id FROM tags WHERE name = ? AND (parent_id = ? OR (parent_id IS NULL AND ? IS NULL))"
            )
            .bind(&part)
            .bind(last_parent_id)
            .bind(last_parent_id)
            .fetch_optional(&self.db)
            .await?;

            let id = if let Some((existing_id,)) = row {
                existing_id
            } else {
                let res = sqlx::query(
                    "INSERT INTO tags (name, category, parent_id) VALUES (?, 'path', ?)"
                )
                .bind(&part)
                .bind(last_parent_id)
                .execute(&self.db)
                .await?;
                res.last_insert_rowid() as i32
            };

            last_parent_id = Some(id);
        }

        last_parent_id.ok_or_else(|| anyhow::anyhow!("路径为空，无法生成标签"))
    }

    /// 建立文件与标签的关联
    pub async fn link_file_to_tag(&self, file_id: i32, tag_id: i32, source: &str) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT OR IGNORE INTO file_tags (file_id, tag_id, source) VALUES (?, ?, ?)"
        )
        .bind(file_id)
        .bind(tag_id)
        .bind(source)
        .execute(&self.db)
        .await?;
        Ok(())
    }
}
