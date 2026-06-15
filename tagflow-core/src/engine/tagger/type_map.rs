//! 扩展名 → 宏类型分桶映射。
//!
//! 未列出的扩展名返回 `None`，调用方据此跳过 type 标签生成，
//! 避免出现 `#type:other` 兜底桶膨胀。
//!
//! 输入约定为**已小写**的扩展名（scanner 在入库时已统一小写）。

/// 将小写扩展名映射到 type 桶名（`image`/`video`/`audio`/`code`/`document`/`text`）。
///
/// 未知扩展名返回 `None`。
pub fn ext_to_type(ext: &str) -> Option<&'static str> {
    match ext {
        // 图片
        "jpg" | "jpeg" | "png" | "gif" | "heic" | "webp" | "bmp" | "svg" => Some("image"),
        // 视频
        "mp4" | "mkv" | "mov" | "avi" | "webm" | "flv" => Some("video"),
        // 音频
        "mp3" | "flac" | "wav" | "aac" | "ogg" | "m4a" => Some("audio"),
        // 代码
        "js" | "ts" | "py" | "go" | "rs" | "java" | "c" | "cpp" | "h" | "cs" | "rb" | "php"
        | "swift" | "kt" | "vue" | "html" | "css" | "sql" | "sh" => Some("code"),
        // 文档
        "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "odt" | "ods" | "odp" => {
            Some("document")
        }
        // 纯文本（原需求 5 桶外的新增桶）
        "txt" | "md" | "log" | "csv" => Some("text"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_image_extensions() {
        for ext in ["jpg", "jpeg", "png", "gif", "heic", "webp", "bmp", "svg"] {
            assert_eq!(ext_to_type(ext), Some("image"), "应映射为 image: {}", ext);
        }
    }

    #[test]
    fn maps_video_and_audio() {
        assert_eq!(ext_to_type("mp4"), Some("video"));
        assert_eq!(ext_to_type("flac"), Some("audio"));
    }

    #[test]
    fn maps_code_and_document() {
        assert_eq!(ext_to_type("rs"), Some("code"));
        assert_eq!(ext_to_type("pdf"), Some("document"));
        assert_eq!(ext_to_type("xlsx"), Some("document"));
    }

    #[test]
    fn maps_text_bucket() {
        for ext in ["txt", "md", "log", "csv"] {
            assert_eq!(ext_to_type(ext), Some("text"), "应映射为 text: {}", ext);
        }
    }

    #[test]
    fn unknown_extension_returns_none() {
        assert_eq!(ext_to_type("epub"), None);
        assert_eq!(ext_to_type("mobi"), None);
        assert_eq!(ext_to_type("xyz"), None);
        assert_eq!(ext_to_type(""), None);
    }

    #[test]
    fn is_case_sensitive_expecting_lowercase() {
        // 契约：调用方负责小写。这里验证大写不被识别，提醒调用方。
        assert_eq!(ext_to_type("TXT"), None);
    }
}
