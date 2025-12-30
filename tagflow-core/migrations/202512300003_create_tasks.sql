-- 任务队列表
-- 用于异步任务调度，如缩略图生成等
CREATE TABLE IF NOT EXISTS tasks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_id INTEGER NOT NULL,           -- 关联的文件 ID
    task_type TEXT NOT NULL DEFAULT 'thumb',  -- 任务类型: 'thumb' (缩略图)
    status INTEGER NOT NULL DEFAULT 0,  -- 状态: 0=待处理, 1=进行中, 2=已完成, 3=失败
    priority INTEGER NOT NULL DEFAULT 0, -- 优先级: 数字越大优先级越高
    error_msg TEXT,                     -- 错误信息
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    started_at DATETIME,
    completed_at DATETIME,
    FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE
);

-- 创建索引以优化查询性能
CREATE INDEX IF NOT EXISTS idx_tasks_status_priority ON tasks(status, priority DESC, id ASC);
CREATE INDEX IF NOT EXISTS idx_tasks_file_id ON tasks(file_id);
