use opendal::{services, Operator};
use crate::models::db::Library;

pub struct StorageManager;

impl StorageManager {
    /// 根据 Library 配置初始化 OpenDAL 算子
    pub fn get_operator(library: &Library) -> anyhow::Result<Operator> {
        match library.protocol.as_str() {
            "local" => {
                let builder = services::Fs::default().root(&library.base_path);
                let op = Operator::new(builder)?.finish();
                Ok(op)
            }
            "webdav" => {
                // 预留 WebDAV 实现空间
                anyhow::bail!("WebDAV 协议暂未在此阶段启用")
            }
            _ => anyhow::bail!("不支持的协议: {}", library.protocol),
        }
    }
}
