//! 运行时配置：从环境变量读取可配置项，提供单一来源以避免硬编码散落。
//!
//! 当前收敛：
//! - `TAGFLOW_DB_PATH`：SQLite 数据库文件路径
//! - `TAGFLOW_CACHE_DIR`：缩略图缓存目录
//! - `TAGFLOW_SCAN_INTERVAL`：定时扫描间隔（秒），供 scheduler 读取
//!
//! 读取入口（启动期 `main.rs`、读取端 `api/file.rs`、`engine/scheduler.rs`）
//! 必须通过本模块获取值，不允许在调用点重复 `std::env::var` 或硬编码字符串。

/// SQLite 数据库文件路径环境变量名
pub const DB_PATH_ENV: &str = "TAGFLOW_DB_PATH";

/// 缩略图缓存目录环境变量名
pub const CACHE_DIR_ENV: &str = "TAGFLOW_CACHE_DIR";

/// 定时扫描间隔（秒）环境变量名
pub const SCAN_INTERVAL_ENV: &str = "TAGFLOW_SCAN_INTERVAL";

/// 数据库文件路径缺省值（相对当前工作目录，兼容历史行为）
pub const DEFAULT_DB_PATH: &str = "tagflow.db";

/// 缩略图缓存目录缺省值（相对当前工作目录）
pub const DEFAULT_CACHE_DIR: &str = "./cache";

/// 定时扫描间隔缺省值（秒）：1 小时
pub const DEFAULT_SCAN_INTERVAL_SECS: u64 = 3600;

/// 定时扫描间隔最小值（秒）：低于此值一律 clamp 回它，避免高频扫描压满 IO
pub const MIN_SCAN_INTERVAL_SECS: u64 = 60;

/// e2e 专用：绕过 [`MIN_SCAN_INTERVAL_SECS`] clamp 的开关环境变量名。
///
/// 仅当 TagFlow 自身的 Playwright 套件（`tagflow-e2e`）需要在可观测窗口内验证
/// scheduler 行为时使用——production / 开发环境**绝不应**设置。值为 `1` 时跳过 clamp，
/// 让 `TAGFLOW_SCAN_INTERVAL=2` 真正生效。详见 [`scan_interval_secs`]。
pub const E2E_FAST_SCAN_ENV: &str = "TAGFLOW_E2E_FAST_SCAN";

/// 读取数据库文件路径，缺省回退到 [`DEFAULT_DB_PATH`]
pub fn db_path() -> String {
    std::env::var(DB_PATH_ENV).unwrap_or_else(|_| DEFAULT_DB_PATH.to_string())
}

/// 读取缩略图缓存目录，缺省回退到 [`DEFAULT_CACHE_DIR`]
pub fn cache_dir() -> String {
    std::env::var(CACHE_DIR_ENV).unwrap_or_else(|_| DEFAULT_CACHE_DIR.to_string())
}

/// 读取定时扫描间隔（秒）。
///
/// 缺省回退到 [`DEFAULT_SCAN_INTERVAL_SECS`]；任何无法解析为 `u64` 的值
/// （缺失、空串、负数、非数字）回退到缺省值；解析成功但小于
/// [`MIN_SCAN_INTERVAL_SECS`] 的值 clamp 到下限，避免高频扫描压满 IO。
///
/// **e2e 例外**：当 [`E2E_FAST_SCAN_ENV`] = `1` 时跳过 clamp，让 Playwright 套件
/// 能注入亚分钟级间隔（如 2s）验证 scheduler 行为。该环境变量仅为 TagFlow 自身的
/// e2e 套件设计，production / 开发环境不应设置。
pub fn scan_interval_secs() -> u64 {
    let parsed = parse_scan_interval_secs();
    if is_e2e_fast_scan() {
        parsed
    } else {
        clamp_scan_interval_secs(parsed)
    }
}

/// 是否启用 e2e 快速扫描模式（绕过生产 clamp）。
fn is_e2e_fast_scan() -> bool {
    matches!(std::env::var(E2E_FAST_SCAN_ENV).as_deref(), Ok("1"))
}

/// 纯函数：解析 `TAGFLOW_SCAN_INTERVAL` 环境变量为秒数。
///
/// 缺失或非合法值（含负数、空串、非数字）一律回退到 [`DEFAULT_SCAN_INTERVAL_SECS`]。
fn parse_scan_interval_secs() -> u64 {
    match std::env::var(SCAN_INTERVAL_ENV) {
        Ok(raw) => match raw.trim().parse::<i64>() {
            // 非负整数按 u64 取（i64 非负域 ⊂ u64 域，转换不会溢出）
            Ok(v) if v >= 0 => v as u64,
            // 负数 → 缺省
            _ => DEFAULT_SCAN_INTERVAL_SECS,
        },
        Err(_) => DEFAULT_SCAN_INTERVAL_SECS,
    }
}

/// 纯函数：把任意秒数 clamp 到 ≥ [`MIN_SCAN_INTERVAL_SECS`]
fn clamp_scan_interval_secs(secs: u64) -> u64 {
    secs.max(MIN_SCAN_INTERVAL_SECS)
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
        assert_eq!(SCAN_INTERVAL_ENV, "TAGFLOW_SCAN_INTERVAL");
        assert_eq!(DEFAULT_SCAN_INTERVAL_SECS, 3600);
        assert_eq!(MIN_SCAN_INTERVAL_SECS, 60);
    }

    #[test]
    fn clamp_scan_interval_下限() {
        // 0、小于下限的值都被 clamp 到 MIN_SCAN_INTERVAL_SECS
        assert_eq!(clamp_scan_interval_secs(0), MIN_SCAN_INTERVAL_SECS);
        assert_eq!(clamp_scan_interval_secs(30), MIN_SCAN_INTERVAL_SECS);
        assert_eq!(clamp_scan_interval_secs(59), MIN_SCAN_INTERVAL_SECS);
        // 恰好等于下限保留
        assert_eq!(
            clamp_scan_interval_secs(MIN_SCAN_INTERVAL_SECS),
            MIN_SCAN_INTERVAL_SECS
        );
        // 正常值原样返回
        assert_eq!(clamp_scan_interval_secs(120), 120);
        assert_eq!(
            clamp_scan_interval_secs(DEFAULT_SCAN_INTERVAL_SECS),
            DEFAULT_SCAN_INTERVAL_SECS
        );
        assert_eq!(clamp_scan_interval_secs(86_400), 86_400);
    }

    #[test]
    fn scan_interval_secs_生产与_e2e_模式_clamp_行为() {
        // 这两个逻辑必须合并在一个 #[test]：cargo test 默认多线程共享 process.env，
        // 分别用两个 #[test] 互相 race（一个 remove 另一个刚 set 的 E2E_FAST_SCAN_ENV）。
        // 顺序执行同一段代码就不会有竞态。

        // 环境变量名契约
        assert_eq!(E2E_FAST_SCAN_ENV, "TAGFLOW_E2E_FAST_SCAN");

        // === 开关识别（is_e2e_fast_scan）===
        // 缺省未设置 → 关闭
        unsafe { std::env::remove_var(E2E_FAST_SCAN_ENV) };
        assert!(!is_e2e_fast_scan());
        // 设为 "1" → 开启
        unsafe { std::env::set_var(E2E_FAST_SCAN_ENV, "1") };
        assert!(is_e2e_fast_scan());
        // 其它值（含 "true"、"yes"）一律视为关闭，避免误开
        unsafe { std::env::set_var(E2E_FAST_SCAN_ENV, "true") };
        assert!(!is_e2e_fast_scan());

        // === 生产模式（旁路关闭）：2s 被 clamp 回 60s ===
        unsafe {
            std::env::remove_var(E2E_FAST_SCAN_ENV);
            std::env::set_var(SCAN_INTERVAL_ENV, "2");
        }
        assert_eq!(scan_interval_secs(), MIN_SCAN_INTERVAL_SECS);

        // === e2e 模式（旁路开启）：2s 原样生效 ===
        unsafe { std::env::set_var(E2E_FAST_SCAN_ENV, "1") };
        assert_eq!(scan_interval_secs(), 2);

        // 恢复，避免污染同进程其它测试
        unsafe {
            std::env::remove_var(SCAN_INTERVAL_ENV);
            std::env::remove_var(E2E_FAST_SCAN_ENV);
        }
    }
}
