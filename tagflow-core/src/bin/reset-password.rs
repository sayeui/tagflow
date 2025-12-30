//! 密码重置工具
//!
//! 用法：
//! ```bash
//! cargo run --bin reset-password -- --new-password YOUR_NEW_PASSWORD
//! cargo run --bin reset-password -- --username admin --new-password YOUR_NEW_PASSWORD
//! ```

use anyhow::{anyhow, Result};

// 使用 tagflow_core 库中的模块
use tagflow_core::{core::auth, infra::db};

#[tokio::main]
async fn main() -> Result<()> {
    // 解析命令行参数
    let args: Vec<String> = std::env::args().collect();

    let mut username = "admin".to_string();
    let mut new_password = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--username" | "-u" => {
                if i + 1 < args.len() {
                    username = args[i + 1].clone();
                    i += 2;
                } else {
                    return Err(anyhow!("--username 需要一个参数"));
                }
            }
            "--new-password" | "-p" => {
                if i + 1 < args.len() {
                    new_password = Some(args[i + 1].clone());
                    i += 2;
                } else {
                    return Err(anyhow!("--new-password 需要一个参数"));
                }
            }
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            _ => {
                return Err(anyhow!("未知参数: {}", args[i]));
            }
        }
    }

    // 如果没有提供新密码，提示用户输入
    let new_password = match new_password {
        Some(p) => p,
        None => {
            println!("请输入新密码:");
            let mut password = String::new();
            std::io::stdin().read_line(&mut password)?;
            password.trim().to_string()
        }
    };

    if new_password.is_empty() {
        return Err(anyhow!("密码不能为空"));
    }

    println!();
    println!("==============================================");
    println!("正在重置用户 '{}' 的密码...", username);

    // 连接数据库
    let db_url = "sqlite:tagflow.db?mode=rwc";
    let pool = db::init_db(db_url).await?;

    // 检查用户是否存在
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM users WHERE username = ?")
        .bind(&username)
        .fetch_one(&pool)
        .await?;

    if count == 0 {
        return Err(anyhow!("用户 '{}' 不存在", username));
    }

    // 哈希新密码
    let password_hash = auth::hash_password(&new_password)?;

    // 更新数据库
    let rows_affected = sqlx::query(
        "UPDATE users SET password_hash = ? WHERE username = ?"
    )
    .bind(&password_hash)
    .bind(&username)
    .execute(&pool)
    .await?
    .rows_affected();

    if rows_affected == 0 {
        return Err(anyhow!("密码更新失败"));
    }

    println!("密码重置成功！");
    println!("  用户名: {}", username);
    println!("  新密码: {}", new_password);
    println!("==============================================");
    println!();

    Ok(())
}

fn print_help() {
    println!("TagFlow 密码重置工具");
    println!();
    println!("用法:");
    println!("  cargo run --bin reset-password -- [选项]");
    println!();
    println!("选项:");
    println!("  -u, --username <用户名>     要重置密码的用户名 (默认: admin)");
    println!("  -p, --new-password <密码>   新密码 (不提供则交互式输入)");
    println!("  -h, --help                  显示此帮助信息");
    println!();
    println!("示例:");
    println!("  cargo run --bin reset-password -- -u admin -p newpass123");
    println!("  cargo run --bin reset-password -- --new-password newpass123");
}
