# TagFlow NAS 部署验证报告

- **URL**: http://fnos.pve.saye:18080/
- **访问时间**: 2026-06-15 12:08 (CST)
- **验证手段**: chrome-devtools MCP **不可用**(本会话仅 Read/Write/Bash/Skill 工具),改用 `curl` 直接调用 REST API 做只读核对。无法对前端 UI 截图。
- **截图清单**: 无(浏览器自动化工具不在环境内)。截图目录 `.trellis/workspace/sayeui/nas-shots/` 已建但为空。
- **网络**: DNS 解析正常(10.10.11.100),HTTP 200 可达,登录 `admin / Saye@0094` 成功签发 JWT。

## 与代码结论一致项(功能确实缺失)

- **类型/扩展名/年月标签全无**: `GET /api/v1/tags/tree` 仅返回 `[{id:1, name:"500+", category:"path", children:[]}]`,只有一个 path 类根节点,无 `type/ext/year/month` 任何节点。代码结论准确。
- **文件操作 API 未注册**: `DELETE/PATCH/POST /api/v1/files/:id` 与 `/api/v1/files/:id/tags` 全部退回 SPA `index.html`(content-type: text/html),而合法 API 路由(`tags/tree`)返回 `application/json`,对照之下确认后端未注册这些路由 → 无重命名/移动/删除/手动贴标签 API。
- **WebDAV 未实现**: `POST /api/v1/libraries/test` 对 `protocol:"webdav"` 返回 `{"reachable":false,"message":"WebDAV 协议暂未实现"}`;`local` 协议返回 `{"reachable":true}`。
- **缩略图全 404**: 抽样 6 个 file id(386/390/391/1/2/1000)thumbnail 全 404。资源库 `/library/novel` 是纯 `.txt` 小说(523 个文件),无图片/视频,worker 未生成任何缩略图 → 与"缩略图仅对媒体文件生成"的代码逻辑一致。

## 与代码结论不符项

- **无**(代码分析的"应缺失"清单 100% 命中,无一项被实际部署打脸)。
- 注:`?category=type` 和 `?tag_ids=1,2` 参数被后端静默忽略(返回与无参相同的 523 条),并非"AND 多标签过滤生效"——只是参数未实现,与"多标签 AND 不支持"结论一致。

## 额外发现

- 资源库仅 1 个:`id=1, name="小说", protocol="local", base_path="/library/novel"`,已扫描时间 `2026-06-15T03:45:40Z`(今日),共 523 个 txt 文件全部 `parent_path="500+/"`。
- 所有 file-ops 路由返回 200 + HTML 而非 404,会让前端 fetch 误判成功(若将来加调用方需注意 fallback 行为)。
- 未触发任何 5xx 错误,服务稳定。

## 结论

代码功能盘点 100% 准确:线上部署与代码结论完全吻合,无意外功能、无 phantom API。**唯一遗憾**:无法对前端 UI 做视觉核对(无 chrome-devtools MCP),所有结论基于后端 API 行为;若需 UI 层(右键菜单、视图切换按钮、表单字段)的最终确认,需用真实浏览器复跑。
