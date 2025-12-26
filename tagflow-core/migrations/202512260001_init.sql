-- 1. 资源库表
CREATE TABLE IF NOT EXISTS libraries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    protocol TEXT NOT NULL,         -- 'local', 'webdav'
    base_path TEXT NOT NULL,
    config_json TEXT,               -- 存储加密凭据或排除规则
    last_scanned_at DATETIME
);

-- 2. 标签表 (层级结构)
CREATE TABLE IF NOT EXISTS tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    category TEXT NOT NULL,         -- 'path', 'type', 'user', 'time'
    parent_id INTEGER REFERENCES tags(id) ON DELETE CASCADE,
    UNIQUE(name, parent_id)
);

-- 3. 文件索引表
CREATE TABLE IF NOT EXISTS files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    parent_path TEXT NOT NULL,      -- 相对路径
    filename TEXT NOT NULL,
    extension TEXT,                 -- 后缀
    size INTEGER NOT NULL,
    mtime INTEGER NOT NULL,         -- 修改时间戳
    hash TEXT,                      -- 用于检测移动
    status INTEGER DEFAULT 1,       -- 1:在线, 0:丢失
    indexed_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- 4. 文件-标签关联表
CREATE TABLE IF NOT EXISTS file_tags (
    file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    source TEXT DEFAULT 'auto',     -- 'auto', 'manual'
    PRIMARY KEY(file_id, tag_id)
);

-- 5. 创建基础索引
CREATE INDEX IF NOT EXISTS idx_files_lookup ON files(library_id, parent_path, filename);
CREATE INDEX IF NOT EXISTS idx_tags_parent ON tags(parent_id);