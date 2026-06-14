//! 运行时配置：从环境变量读取可配置项，提供单一来源以避免硬编码散落。
//!
//! 当前收敛：
//! - `TAGFLOW_DB_PATH`：SQLite 数据库文件路径
//! - `TAGFLOW_CACHE_DIR`：缩略图缓存目录
//!
//! 读取入口（启动期 `main.rs`、读取端 `api/file.rs`）必须通过本模块获取值，
//! 不允许在调用点重复 `std::env::var` 或硬编码字符串。

/// SQLite 数据库文件路径环境变量名
pub const DB_PATH_ENV: &str = "TAGFLOW_DB_PATH";

/// 缩略图缓存目录环境变量名
pub const CACHE_DIR_ENV: &str = "TAGFLOW_CACHE_DIR";

/// 数据库文件路径缺省值（相对当前工作目录，兼容历史行为）
pub const DEFAULT_DB_PATH: &str = "tagflow.db";

/// 缩略图缓存目录缺省值（相对当前工作目录）
pub const DEFAULT_CACHE_DIR: &str = "./cache";

/// 读取数据库文件路径，缺省回退到 [`DEFAULT_DB_PATH`]
pub fn db_path() -> String {
    std::env::var(DB_PATH_ENV).unwrap_or_else(|_| DEFAULT_DB_PATH.to_string())
}

/// 读取缩略图缓存目录，缺省回退到 [`DEFAULT_CACHE_DIR`]
pub fn cache_dir() -> String {
    std::env::var(CACHE_DIR_ENV).unwrap_or_else(|_| DEFAULT_CACHE_DIR.to_string())
}

/// 构造 sqlx 连接 URL（`sqlite:<path>?mode=rwc`），首次启动自动创建文件
pub fn db_url() -> String {
    sqlite_url(&db_path())
}

/// 纯函数：将路径拼接为 SQLite 连接 URL
fn sqlite_url(path: &str) -> String {
    format!("sqlite:{}?mode=rwc", path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_url_拼接相对路径() {
        assert_eq!(sqlite_url("tagflow.db"), "sqlite:tagflow.db?mode=rwc");
    }

    #[test]
    fn sqlite_url_拼接绝对路径() {
        assert_eq!(
            sqlite_url("/var/lib/tagflow/tagflow.db"),
            "sqlite:/var/lib/tagflow/tagflow.db?mode=rwc"
        );
    }

    #[test]
    fn sqlite_url_拼接空路径() {
        assert_eq!(sqlite_url(""), "sqlite:?mode=rwc");
    }

    #[test]
    fn 默认值不随环境漂移() {
        assert_eq!(DEFAULT_DB_PATH, "tagflow.db");
        assert_eq!(DEFAULT_CACHE_DIR, "./cache");
        assert_eq!(DB_PATH_ENV, "TAGFLOW_DB_PATH");
        assert_eq!(CACHE_DIR_ENV, "TAGFLOW_CACHE_DIR");
    }
}
