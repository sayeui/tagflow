# 修复 list_files 分页 total 计数与错误处理

## Goal

`GET /api/v1/files` 的 `total` 字段当前返回**当页条数**（`items.len()`）而非满足条件的**总记录数**，前端虚拟滚动/分页依赖该值，10 万+文件场景下分页彻底失效。同时该 handler 用 `unwrap_or_default()` 吞掉 DB 错误（500 伪装成空列表 200），违反项目错误处理规范，已在 `.trellis/spec/backend/error-handling.md` 标注为已知技术债，本次一并清除。

## Requirements

1. **total 真实计数**（`api/file.rs::list_files`）
   - 三个查询分支各配一条 COUNT 查询，WHERE 条件与取数查询完全一致（含 `status = 1`）：
     - 递归分支：`WITH RECURSIVE sub_tags ... SELECT COUNT(DISTINCT f.id) FROM files f JOIN file_tags ft ... WHERE ft.tag_id IN (...) AND f.status = 1`
     - 直接标签分支：`SELECT COUNT(*) ... WHERE ft.tag_id = ? AND f.status = 1`
     - 无过滤分支：`SELECT COUNT(*) FROM files WHERE status = 1`
   - 用 `sqlx::query_scalar::<_, i64>`，与 `engine/worker.rs` 现有风格一致
2. **错误处理修正**
   - 签名改为 `Result<Json<FileResponse>, StatusCode>`，DB 错误 `map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)`，错误时 `error!` 日志（中文）
   - 移除 `unwrap_or_default()`
3. **响应结构不变**：`FileResponse { items, total }` 字段不动，前端零改动（`useResourceStore.fetchFiles` 仅读 `items`，`total` 留给后续无限滚动使用）

## Acceptance Criteria

* [x] 库中 N 个文件（N > limit）时，`limit=50&page=1` 返回 50 条 items 且 `total = N`（e2e：60 文件，page1=50/total60，page2=10/total60）
* [x] 带 `tag_id`（recursive=true/false）时 total 与对应条件的实际总数一致，丢失文件（status=0）不计入（e2e：DirA total=55；删 1 个重扫后 59/54）
* [x] DB 错误返回 500（而非空列表 200），有 error 日志
* [x] `cargo fmt && cargo clippy && cargo test` 通过

## Definition of Done

* 既有测试全绿；端到端用真实数据验证 total（>50 个文件 + 标签过滤场景）

## Technical Approach

每个分支先 COUNT 后取数（两条独立查询，不引入事务——单用户 SQLite WAL 场景下读间隙写入造成的 total 微小偏差可接受，不值得为此加复杂度）。

## Out of Scope

* 前端无限滚动/加载更多（total 修对后另起任务）
* keyset/cursor 分页优化（当前 OFFSET 分页在性能目标内）

## Technical Notes

* 现状代码：`tagflow-core/src/api/file.rs:14-66`（上一任务刚加过 `status = 1` 过滤，COUNT 条件必须同步包含）
* 规范：`.trellis/spec/backend/error-handling.md`（明确点名此 unwrap_or_default 为勿模仿的技术债）、`database-guidelines.md`（query_scalar 模式）
