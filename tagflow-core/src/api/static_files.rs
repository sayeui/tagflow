//! 前端静态资源服务：通过 `rust-embed` 将 `tagflow-ui/dist` 嵌入二进制，
//! 并以 SPA fallback 处理前端路由刷新。
//!
//! 路由优先级（在 `main.rs` 中通过 `.fallback` 挂载于所有 `/api/*` 之后）：
//! 1. `/api/*` 由 API 路由优先匹配
//! 2. 其他路径先尝试匹配嵌入的静态资源（JS/CSS/图片等）
//! 3. 未命中静态资源则返回 `index.html`，交由前端 router 处理（SPA 模式）

use axum::{
    body::Body,
    extract::Request,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use mime_guess::MimeGuess;
use rust_embed::RustEmbed;
use std::borrow::Cow;

/// 嵌入的前端构建产物（编译期从 `tagflow-ui/dist` 读取）
///
/// `folder` 路径相对于本 crate 的 `Cargo.toml` 所在目录（`tagflow-core/`）。
#[derive(RustEmbed)]
#[folder = "../tagflow-ui/dist/"]
struct Asset;

/// SPA 静态资源 fallback handler
///
/// - 命中嵌入资源：返回资源内容并附带推断的 MIME
/// - 未命中：返回 `index.html`，让前端 router 处理（如 `/login`、`/settings/security`）
/// - 极端情况（`index.html` 也缺失）：返回 500，说明构建产物不完整
pub async fn static_handler(req: Request) -> Response {
    let asset_path = resolve_asset_path(req.uri().path());

    if let Some(content) = Asset::get(asset_path) {
        return build_response(asset_path, content.data);
    }

    // SPA fallback：交由前端 router
    match Asset::get("index.html") {
        Some(content) => build_response("index.html", content.data),
        None => {
            tracing::error!("前端构建产物缺失 index.html，请先执行 npm run build");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// 去除前导 `/`，空路径回退到 `index.html`
fn resolve_asset_path(raw: &str) -> &str {
    let trimmed = raw.trim_start_matches('/');
    if trimmed.is_empty() {
        "index.html"
    } else {
        trimmed
    }
}

/// 推断 MIME essence（如 `text/html`、`application/javascript`）
fn mime_essence(path: &str) -> String {
    MimeGuess::from_path(path)
        .first_or_octet_stream()
        .essence_str()
        .to_owned()
}

/// 构造带正确 MIME 的响应；`Response::builder()` 是 infallible builder
fn build_response(path: &str, data: Cow<'static, [u8]>) -> Response {
    Response::builder()
        .header(header::CONTENT_TYPE, mime_essence(path))
        .body(Body::from(data))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mime_推断覆盖前端常见类型() {
        assert_eq!(mime_essence("index.html"), "text/html");
        // `.js` 在 WHATWG 现代标准下为 text/javascript（取代旧的 application/javascript）
        assert_eq!(mime_essence("assets/app.js"), "text/javascript");
        assert_eq!(mime_essence("assets/style.css"), "text/css");
        assert_eq!(mime_essence("favicon.ico"), "image/x-icon");
        assert_eq!(mime_essence("logo.svg"), "image/svg+xml");
    }

    #[test]
    fn mime_未知扩展名回退_octet_stream() {
        assert_eq!(mime_essence("data.xyzunknown"), "application/octet-stream");
    }

    #[test]
    fn 根路径与空路径回退_index_html() {
        assert_eq!(resolve_asset_path("/"), "index.html");
        assert_eq!(resolve_asset_path(""), "index.html");
    }

    #[test]
    fn 普通路径去除前导斜杠() {
        assert_eq!(resolve_asset_path("/assets/app.js"), "assets/app.js");
        assert_eq!(resolve_asset_path("assets/app.js"), "assets/app.js");
    }

    #[test]
    fn 前端路由路径保留原样交给_spa_fallback() {
        // resolve_asset_path 不区分静态资源与前端路由，全部去前导斜杠；
        // 命中检测由 static_handler 通过 Asset::get 完成，未命中则回退到 index.html
        assert_eq!(resolve_asset_path("/login"), "login");
        assert_eq!(
            resolve_asset_path("/settings/security"),
            "settings/security"
        );
    }
}
