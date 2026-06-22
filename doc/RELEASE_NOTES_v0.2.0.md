# TagFlow v0.2.0

首个正式版（非 Pre-release）。从 v0.1.0 Beta 升级，聚焦自动同步、视图增强与稳定性。

## ✨ 新功能

- **定时增量扫描**：后台 scheduler 每 `TAGFLOW_SCAN_INTERVAL`（默认 3600s，可配）自动扫描所有资源库，启动后立即首轮；与手动 `POST /scan` 共享 409 并发锁，同库不并发；前端展示「上次/下次扫描」时间。
- **文件视图增强**：无限滚动加载、文件名搜索、卡片/列表视图切换、卡片重叠修复。
- **手动用户标签**：文件抽屉打层级 user 标签、按 user 标签过滤（递归命中子树）、移除后空节点自动清理。
- **媒体预览抽屉**：文本（GBK→UTF-8）/ Markdown / PDF / 图片全屏 / 视频 Range seek / 音频 + 下载。
- **自动标签四维**：path / type / ext / time（`#year:` / `#month:`），多标签 AND 分面过滤。
- **e2e 测试框架**：Playwright 14 用例（登录 / 文件 / 搜索 / 视图 / 标签树 / 扫描 / 缩略图 / 定时同步 / 标签清理）。

## 🐛 修复（相对 v0.1.0 Beta）

- **SQLite 并发写 `database is locked`**：`busy_timeout` 15s + `foreign_keys` per-connection（修 CASCADE）+ worker 重试兜底。
- **非媒体文件请求缩略图 404**：前端按 `MEDIA_EXTENSIONS` 白名单请求，非媒体不发。
- **删库/删文件后孤儿标签残留**：标签树过滤在线文件（status=1）+ 删库后清理孤儿 tags。
- 密码门槛前后端统一（≥12 字节）、资源库路径校验（杜绝幽灵空库）、媒体 `?token=` 鉴权兜底。

## ⬆️ 升级

从 v0.1.0 Beta 升级：重新 `docker compose build`（或拉新镜像）+ `docker compose up -d --force-recreate`。DB 迁移自动、历史文件按 `app_meta.tagger_version` 回填 type/ext/time 标签。

## ⚠️ 已知限制（计划 v0.3+）

- WebDAV 资源库未实现（v0.3）
- 批量打标签未实现（v0.3）
- 文件操作（改名 / 移动 / 删除）暂不支持

## 📦 部署

Docker：`docker compose up -d`，必填 `TAGFLOW_JWT_SECRET`（≥32B，`openssl rand -hex 32`）+ `TAGFLOW_ADMIN_PASSWORD`（≥12B）。可选 `TAGFLOW_SCAN_INTERVAL`（定时扫描间隔，默认 3600s）。详见 [`doc/部署指南.md`](部署指南.md)。

---

**Full Changelog**: https://github.com/sayeui/tagflow/compare/v0.1.0...v0.2.0
