use crate::models::db::Tag;
use crate::models::dto::TagNode;
use axum::{Json, extract::State};
use sqlx::SqlitePool;
use tracing::error;

pub async fn get_tag_tree(State(pool): State<SqlitePool>) -> Json<Vec<TagNode>> {
    // 1. 获取所有标签
    let tags: Vec<Tag> = sqlx::query_as("SELECT * FROM tags")
        .fetch_all(&pool)
        .await
        .unwrap_or_default();

    // 2. 查「有在线（status=1）文件关联的 tag_id 集合」——标签树只显示在线文件可达的标签。
    //    这同时过滤两类无效节点：删库/删关联后的真孤儿（无 file_tags）与扫描 mark_as_lost
    //    软删后关联离线文件（status=0）的节点。软删语义保留：文件恢复（status→1）时标签自动回归。
    let online_tag_ids: Vec<i32> = match sqlx::query_scalar::<_, i32>(
        "SELECT DISTINCT ft.tag_id FROM file_tags ft \
             JOIN files f ON ft.file_id = f.id \
             WHERE f.status = 1",
    )
    .fetch_all(&pool)
    .await
    {
        Ok(ids) => ids,
        Err(e) => {
            // 查询失败按「无在线关联」处理：返回空树，避免把孤儿当作在线泄漏给前端。
            error!("查询在线文件关联标签失败: {}", e);
            return Json(Vec::new());
        }
    };
    let online_set: std::collections::HashSet<i32> = online_tag_ids.into_iter().collect();

    // 3. 在内存中构建树并按子树剪枝：节点若自身不在 online_set 且所有子节点被剪，则剪。
    //    这保证父标签在子标签有在线关联时仍显示（路径父节点自身往往无直接文件关联）。
    Json(build_tree(&tags, None, &online_set))
}

fn build_tree(
    tags: &[Tag],
    parent_id: Option<i32>,
    online_set: &std::collections::HashSet<i32>,
) -> Vec<TagNode> {
    tags.iter()
        .filter(|t| t.parent_id == parent_id)
        .filter_map(|t| {
            // 先递归构建子树；子树为空且自身无在线关联 → 剪掉。
            let children = build_tree(tags, Some(t.id), online_set);
            if children.is_empty() && !online_set.contains(&t.id) {
                None
            } else {
                Some(TagNode {
                    id: t.id,
                    name: t.name.clone(),
                    category: t.category.clone(),
                    children,
                })
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造单连接内存库（schema 与 init.sql 对齐）。
    async fn setup_db() -> SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("connect in-memory");
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .unwrap();
        for stmt in [
            "CREATE TABLE tags (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, category TEXT NOT NULL, parent_id INTEGER REFERENCES tags(id) ON DELETE CASCADE, UNIQUE(name, parent_id))",
            "CREATE TABLE files (id INTEGER PRIMARY KEY AUTOINCREMENT, library_id INTEGER NOT NULL, parent_path TEXT NOT NULL, filename TEXT NOT NULL, extension TEXT, size INTEGER NOT NULL, mtime INTEGER NOT NULL, hash TEXT, status INTEGER DEFAULT 1, indexed_at DATETIME DEFAULT CURRENT_TIMESTAMP)",
            "CREATE TABLE file_tags (file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE, tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE, source TEXT DEFAULT 'auto', PRIMARY KEY(file_id, tag_id))",
        ] {
            sqlx::query(stmt).execute(&pool).await.unwrap();
        }
        pool
    }

    async fn insert_tag(pool: &SqlitePool, name: &str, category: &str, parent: Option<i32>) -> i32 {
        sqlx::query("INSERT INTO tags (name, category, parent_id) VALUES (?, ?, ?)")
            .bind(name)
            .bind(category)
            .bind(parent)
            .execute(pool)
            .await
            .unwrap()
            .last_insert_rowid() as i32
    }

    async fn insert_file(pool: &SqlitePool, filename: &str, status: i32) -> i32 {
        sqlx::query(
            "INSERT INTO files (library_id, parent_path, filename, size, mtime, status) \
             VALUES (1, '', ?, 1, 0, ?)",
        )
        .bind(filename)
        .bind(status)
        .execute(pool)
        .await
        .unwrap()
        .last_insert_rowid() as i32
    }

    async fn link(pool: &SqlitePool, file_id: i32, tag_id: i32) {
        sqlx::query(
            "INSERT OR IGNORE INTO file_tags (file_id, tag_id, source) VALUES (?, ?, 'auto')",
        )
        .bind(file_id)
        .bind(tag_id)
        .execute(pool)
        .await
        .unwrap();
    }

    /// 收集 TagNode 树中所有节点 id（扁平化，用于断言）。
    fn collect_ids(nodes: &[TagNode]) -> Vec<i32> {
        let mut out = Vec::new();
        for n in nodes {
            out.push(n.id);
            out.extend(collect_ids(&n.children));
        }
        out
    }

    #[tokio::test]
    async fn orphan_tag_without_file_tags_is_hidden() {
        let pool = setup_db().await;
        // 孤儿标签：无任何 file_tags 关联
        let orphan = insert_tag(&pool, "Ghost", "path", None).await;
        // 有在线文件关联的对照标签
        let alive = insert_tag(&pool, "Alive", "ext", None).await;
        let f = insert_file(&pool, "a.txt", 1).await;
        link(&pool, f, alive).await;

        let tree = get_tag_tree(State(pool.clone())).await.0;
        let ids = collect_ids(&tree);
        assert!(!ids.contains(&orphan), "孤儿标签不应显示");
        assert!(ids.contains(&alive), "有在线关联的标签应显示");
    }

    #[tokio::test]
    async fn tag_only_linked_to_offline_file_is_hidden() {
        let pool = setup_db().await;
        // 标签仅关联离线文件（status=0，扫描 mark_as_lost 软删场景）
        let offline_tag = insert_tag(&pool, "Offline", "path", None).await;
        let f = insert_file(&pool, "gone.txt", 0).await;
        link(&pool, f, offline_tag).await;

        let tree = get_tag_tree(State(pool.clone())).await.0;
        let ids = collect_ids(&tree);
        assert!(!ids.contains(&offline_tag), "仅关联离线文件的标签不应显示");
    }

    #[tokio::test]
    async fn tag_shared_by_online_and_offline_files_is_shown() {
        let pool = setup_db().await;
        let tag = insert_tag(&pool, "png", "ext", None).await;
        let online_f = insert_file(&pool, "online.png", 1).await;
        let offline_f = insert_file(&pool, "offline.png", 0).await;
        link(&pool, online_f, tag).await;
        link(&pool, offline_f, tag).await;

        let tree = get_tag_tree(State(pool.clone())).await.0;
        let ids = collect_ids(&tree);
        // 只要有一个 status=1 关联就显示（跨库 / 跨文件共享保留语义）
        assert!(ids.contains(&tag), "至少有一个在线文件关联的标签应显示");
    }

    #[tokio::test]
    async fn parent_tag_shown_when_child_has_online_file() {
        let pool = setup_db().await;
        // 路径层级：Projects(parent=NULL, 无直接关联) → 2024(child, 有在线文件)
        let parent = insert_tag(&pool, "Projects", "path", None).await;
        let child = insert_tag(&pool, "2024", "path", Some(parent)).await;
        let f = insert_file(&pool, "x.png", 1).await;
        link(&pool, f, child).await;

        let tree = get_tag_tree(State(pool.clone())).await.0;
        let ids = collect_ids(&tree);
        // 父标签自身无在线关联，但子标签有 → 父按子树判定保留
        assert!(ids.contains(&parent), "父标签在子标签有在线关联时应保留");
        assert!(ids.contains(&child), "有在线关联的子标签应显示");
    }

    #[tokio::test]
    async fn entire_chain_hidden_when_no_online_file() {
        let pool = setup_db().await;
        // 路径层级：Projects → 2024，仅关联离线文件
        let parent = insert_tag(&pool, "Projects", "path", None).await;
        let child = insert_tag(&pool, "2024", "path", Some(parent)).await;
        let f = insert_file(&pool, "gone.png", 0).await;
        link(&pool, f, child).await;

        let tree = get_tag_tree(State(pool.clone())).await.0;
        let ids = collect_ids(&tree);
        // 整条链都不显示（子被剪 → 父也无在线关联 → 父也被剪）
        assert!(ids.is_empty(), "无在线关联的整条链都应被剪");
    }
}
