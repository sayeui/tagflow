-- 应用元数据表（单行键值）
-- 用于记录 tagger 流水线版本，触发存量数据回填。
CREATE TABLE IF NOT EXISTS app_meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
