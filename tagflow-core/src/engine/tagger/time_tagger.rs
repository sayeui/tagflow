use chrono::{Local, TimeZone};

use crate::core::tag::TagManager;

/// 时间标签生成器，从 `files.mtime`（Unix 秒）产出层级标签：
/// - `#year:<YYYY>`（category = `time`，根级）
/// - `#month:<YYYY-MM>`（category = `time`，挂在对应 year 下）
///
/// 文件只关联到 month（叶子），查 year 通过递归 CTE 命中（与 PathTagger
/// 仅关联叶子路径的约定一致）。
///
/// 时区取服务器本地（部署 `TZ=Asia/Shanghai`）；未设置时 chrono 回退 UTC。
/// mtime ≤ 0 视为非法，跳过。
pub struct TimeTagger {
    tag_manager: TagManager,
}

impl TimeTagger {
    pub fn new(tag_manager: TagManager) -> Self {
        Self { tag_manager }
    }

    pub async fn process(&self, file_id: i32, mtime: i64) -> anyhow::Result<()> {
        if mtime <= 0 {
            return Ok(());
        }

        // timestamp_opt 对正常 Unix 秒恒为 Single；.single() 兜底非法边界
        let Some(dt) = Local.timestamp_opt(mtime, 0).single() else {
            return Ok(());
        };

        let year = dt.format("%Y").to_string();
        let month = dt.format("%Y-%m").to_string();

        let year_id = self.tag_manager.ensure_tag(&year, "time", None).await?;
        // month 挂在 year 下；名字用完整 YYYY-MM 保证全局可读、字典序正确
        let month_id = self
            .tag_manager
            .ensure_tag(&month, "time", Some(year_id))
            .await?;

        self.tag_manager
            .link_file_to_tag(file_id, month_id, "auto")
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 纯逻辑测试：验证 year/month 字符串格式（不触 DB）。
    #[test]
    fn formats_year_and_month_strings() {
        let ts: i64 = 1_700_000_000; // 2023-11-14 UTC，本地时区转换后年月可能不同
        let dt = Local.timestamp_opt(ts, 0).single().unwrap();
        let year = dt.format("%Y").to_string();
        let month = dt.format("%Y-%m").to_string();
        assert!(year.len() == 4, "year 应为 4 位: {}", year);
        assert!(
            month.len() == 7 && month.chars().nth(4) == Some('-'),
            "month 应为 YYYY-MM: {}",
            month
        );
    }
}
