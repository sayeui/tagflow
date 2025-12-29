use axum::{extract::State, Json};
use sqlx::SqlitePool;
use crate::models::dto::TagNode;
use crate::models::db::Tag;

pub async fn get_tag_tree(State(pool): State<SqlitePool>) -> Json<Vec<TagNode>> {
    // 1. 获取所有标签
    let tags: Vec<Tag> = sqlx::query_as("SELECT * FROM tags")
        .fetch_all(&pool)
        .await
        .unwrap_or_default();

    // 2. 在内存中构建树 (简单递归)
    let tree = build_tree(&tags, None);
    Json(tree)
}

fn build_tree(tags: &[Tag], parent_id: Option<i32>) -> Vec<TagNode> {
    tags.iter()
        .filter(|t| t.parent_id == parent_id)
        .map(|t| TagNode {
            id: t.id,
            name: t.name.clone(),
            category: t.category.clone(),
            children: build_tree(tags, Some(t.id)),
        })
        .collect()
}
