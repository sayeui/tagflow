# tagflow-e2e

TagFlow 的 Playwright 端到端测试。通过环境变量驱动一个**隔离的 Rust 后端进程**
（rust-embed 已把前端嵌入二进制，单进程访问完整 UI）。

## 设计要点

### 零后端改动隔离

所有隔离都靠环境变量注入到 `cargo run` 起的进程（见 `tagflow-core/src/infra/config.rs`
与 `main.rs` 的读取点）：

| 环境变量 | 取值 |
| --- | --- |
| `TAGFLOW_DB_PATH` | OS 临时目录下的 `tagflow-e2e-<random>/tagflow-e2e.db` |
| `TAGFLOW_CACHE_DIR` | 同上目录下的 `cache/` |
| `TAGFLOW_PORT` | `18080`（`lib/env.ts` `TEST_PORT`） |
| `TAGFLOW_ADMIN_PASSWORD` | 固定测试密码（≥12 字节） |
| `TAGFLOW_JWT_SECRET` | 固定测试密钥（≥32 字节） |
| `TAGFLOW_SCAN_INTERVAL` | `2`（让 scheduler 每 2s 扫一轮，使 `scheduled-scan.spec.ts` 能在可观测窗口内验证「无手动触发，scheduler 自动扫入新文件」） |
| `TAGFLOW_E2E_FAST_SCAN` | `1`（绕过后端 60s 的生产 clamp，让上一行的 2s 真正生效。仅供本套 e2e 使用，production 不应设置） |

> scheduler 每 2s 频扫 e2e-fixtures 库不影响其它用例：增量扫描幂等（mtime/size 未变即跳过），其它用例不新增/删除文件。`library-scan.spec.ts` 的手动 202 断言用 `triggerScanAcceptingSchedulerConflict` 包了短重试，容忍与 scheduler 的瞬时 409 冲突（后端契约未变，只是放宽了测试侧的时序假设）。

跑完用例后 `globalTeardown` 删除临时目录。仓库内真实的 `tagflow-core/tagflow.db`
与 `./cache` 不会被触碰。

### 时序（最关键的坑）

Playwright 顺序：**webServer 启动 → globalSetup → tests → globalTeardown → webServer 停止**。

因此临时 DB/cache 目录路径**必须**在 `playwright.config.ts` 模块加载**顶层**就创建
（`fs.mkdtempSync`）并拼进 `webServer.env`。如果在 `globalSetup` 里创建就太晚了——
那时后端已带着缺省路径起来，会污染真实 DB。

### ffmpeg 依赖

缩略图链路依赖外部 `ffmpeg`（`tagflow-core/src/infra/thumbnail.rs`）。
`globalSetup` 探测 `ffmpeg -version`，结果写到 `process.env.TAGFLOW_E2E_FFMPEG_AVAILABLE`。
`tests/thumbnails.spec.ts` 据此 `test.skip`：ffmpeg 不可用时整组缩略图用例跳过
（worker 无法生成 webp，断言会超时挂死），其余 spec 照跑。

缩略图用例的稳定性策略：先探测当前态——若已 200 走"稳定 200"断言，
若 404 用 `expect.poll` 轮询直到 200（覆盖 worker 5s 一轮 + ffmpeg 处理余量）。
两条路径都让用例在任意缓存状态下通过。

### 标签树清理用例（`tests/tag-tree-cleanup.spec.ts`）

验证「孤儿标签清理」后端契约：

- **删库孤儿清理**：临时库带独有子目录名（随机串），扫描后该子目录的 path 标签出现；
  删除临时库后该标签消失（`delete_library` 触发 `cleanup_orphan_tag` 真清理 tags 表），
  seeded 库的 path 标签仍健在（跨库共享保留语义）。
- **扫描删文件标签隐藏**（软删语义保留）：临时库文件物理消失后 scheduler `mark_as_lost`
  软删（status=0），独有标签被 `get_tag_tree` 过滤掉；恢复文件后 status→1，标签自动回归。

两条用例都用「临时库 + 随机子目录名」隔离，不触碰 seeded 库的 fixtures，scheduler 2s
频扫对增量幂等。`expect.poll` 轮询覆盖 2s 扫描周期带来的时序延迟。

## 运行

前置：本机已装 `cargo`、`node`、`ffmpeg`（缩略图用例需要；缺失会 skip）。

```bash
cd tagflow-e2e
npm install
npx playwright install chromium   # 首次需装浏览器
npm test                           # 等价于 npx playwright test
```

首次 `npm test` 会触发 `cargo run` 全量编译后端（数分钟，正常）；
之后增量编译很快。`reuseExistingServer: !CI`：本地若已有同 env 后端在 18080 端口
会复用提速，CI 强制新起。

## 故障排查

### `TypeError: context.conditions?.includes is not a function`

Playwright 1.61 在 Node 22 上检测到新的 `module.registerHooks` API 后用它注册
resolve hook，但该 hook 假设 `context.conditions` 是数组，Node 22.18 在某些路径下
传入的不是数组，导致从 `globalSetup.ts` 解析相对模块时崩溃。

`package.json` 的 `test` / `test:headed` 脚本已前置 `PLAYWRIGHT_FORCE_ASYNC_LOADER=1`，
强制 Playwright 走老的 `module.register` 异步加载器（无此 bug）。直接调
`npx playwright test` 而不经 npm script 时需手动 `export PLAYWRIGHT_FORCE_ASYNC_LOADER=1`。
Playwright 后续版本修复后此变通可移除。


## 目录结构

```
tagflow-e2e/
├── playwright.config.ts   # 顶层建临时目录 + webServer 拉隔离后端
├── globalSetup.ts         # ping /api/health + 探测 ffmpeg + seed 资源库
├── globalTeardown.ts      # 清理临时目录
├── lib/
│   ├── env.ts             # 共享常量（端口/凭据/JWT/seeded token）与 env 桥接
│   ├── api.ts             # APIRequestContext + 文件/资源库/缩略图 API 封装
│   └── auth.ts            # UI 登录助手（loginViaUi）
├── tests/                 # Playwright spec
│   ├── login.spec.ts          # PR1：登录 smoke
│   ├── files.spec.ts          # PR2：文件列表 / 搜索 / 视图切换 / 标签树
│   ├── library-scan.spec.ts   # PR3：资源库扫描触发（202 / 404 / 生命周期）
│   ├── thumbnails.spec.ts     # PR3：缩略图 404→200 轮询（ffmpeg skip 兜底）
│   └── scheduled-scan.spec.ts # 定时扫描 PR3：无手动触发，scheduler 自动扫入新文件
└── fixtures/library/      # 内置小图片夹具（嵌套目录 + 中英文文件名）
    ├── Projects/2024/
    ├── Photos/
    └── Reports/
```
