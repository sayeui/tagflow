# 轻量级标签化个人资源管理系统架构与需求定义报告
[在线链接](https://gemini.google.com/share/3baed5413dc7) [产品介绍页面](https://gemini.google.com/share/c1e4d6b68b2c)

## 1. 执行摘要

### 1.1 项目愿景与定位

在个人数据资产日益膨胀的今天，传统基于层级目录（Hierarchical Directory）的文件管理方式已逐渐显露出其局限性。用户往往需要在不同的上下文中访问同一个文件——例如，一张照片既属于“2023年旅行”目录，又具备“风景”、“日本”、“高分辨率”等属性。传统的文件夹结构强制文件仅存在于单一物理路径中，导致跨维度的资源检索变得困难且低效。本项目旨在构建一套**轻量级标签化资源管理系统**（以下简称“TagFlow”），通过引入“元数据覆盖层”（Metadata Overlay）的概念，将物理存储与逻辑组织解耦。

系统的核心理念是**“非侵入式管理”**与**“自动化组织”**。不同于需要用户手动为每个文件添加标签的繁琐流程，TagFlow 利用算法自动从文件的物理路径、拓展名、MIME 类型及元数据中提取基础标签，构建初始的语义网络。这使得用户在连接资源库的瞬间即可获得一个多维度的检索视图，而无需进行大量的前期整理工作。

### 1.2 目标用户与部署环境

本系统专为拥有一定技术背景的个人用户（Prosumer）设计，典型的运行环境为家庭网络存储设备（NAS）、软路由、树莓派或小型云服务器。鉴于目标硬件环境通常具有资源受限（低 CPU 功耗、有限内存）的特点，软件架构必须遵循**极简主义**原则：

- **单体架构（Monolithic Application）：** 拒绝微服务带来的运维复杂性，所有功能模块打包在单一的可执行文件或容器中。
    
- **低依赖（Zero Dependency）：** 摒弃 PostgreSQL、Redis、Elasticsearch 等重量级中间件，采用内嵌式数据库（SQLite）和内存缓存。
    
- **容器化交付（Docker Native）：** 提供开箱即用的 Docker 镜像，支持通过 Docker Compose 进行一键部署和版本更新。
    

### 1.3 核心功能范畴

根据原始需求，本报告将详细定义以下核心功能域：

1. **多源资源库管理：** 支持挂载本地文件系统及远程 WebDAV 服务作为资源输入源。
    
2. **智能扫描与索引：** 实现高效的文件系统遍历算法，建立文件物理路径与逻辑元数据的映射关系。
    
3. **多维自动标签系统：** 基于路径分词、文件类型映射和时间戳分析的自动化标签生成引擎。
    
4. **虚拟化资源视图：** 基于 Web 的交互界面，支持通过标签组合（Faceted Search）过滤资源，提供网格（Grid）与列表（List）视图。
    
5. **元数据持久化：** 设计高性能的 SQLite 模式以存储文件指纹、标签关联及缓存数据。
    

---

## 2. 市场现状与技术选型背景

在着手开发新系统之前，深入分析现有的开源解决方案对于规避设计陷阱、确立产品独特性至关重要。目前市场上的个人文件管理工具主要分为“传统文件管理器”、“资产管理系统（DAM）”和“笔记/知识库工具”三类。

### 2.1 现有解决方案分析

#### 2.1.1 FileBrowser：极致轻量的纯文件管理

FileBrowser 是目前自托管领域最受欢迎的轻量级文件管理器之一 1。

- **架构优势：** 它采用 Go 语言开发，前端基于 Vue.js，最终编译为单一二进制文件。这种架构带来了极低的内存占用（空闲时仅需 10-20MB）和极简的部署体验，完美契合 NAS 环境 1。
    
- **功能局限：** FileBrowser 严格遵循物理文件系统结构，缺乏逻辑层的抽象。用户无法跨文件夹聚合文件，也不支持标签系统。搜索功能仅限于文件名的字符串匹配，无法满足“查找所有 2023 年的 PDF 文档”这类语义化需求 1。
    
- **启示：** TagFlow 应继承 FileBrowser 的部署形态（Go/Rust 后端 + 单一二进制），但在逻辑层引入数据库以支持标签。
    

#### 2.1.2 TagSpaces：标签管理的先驱

TagSpaces 是标签化文件管理的代表性产品，强调“离线优先”和“数据主权” 2。

- **架构特点：** 它采用“Sidecar 文件”或“文件名重写”的方式存储标签。例如，将 `photo.jpg` 重命名为 `photo[vacation].jpg` 3。
    
- **局限性：** 这种“侵入式”设计是其最大的争议点。修改文件名会破坏其他依赖该文件路径的软件（如 Plex、Sonarr）的索引，且文件路径长度限制（特别是在 Windows 上）限制了标签的数量 3。此外，其 Web 版本（TagSpaces Lite Web）通常需要配合对象存储或复杂的后端设置，不具备开箱即用的 NAS 友好性 4。
    
- **启示：** TagFlow 必须坚持**非侵入式**原则。标签数据应存储在系统内部数据库中，严禁修改用户的源文件名称或结构，确保用户数据的完整性与其他系统的兼容性。
    

#### 2.1.3 PhotoPrism / Immich：AI 驱动的资产管理

这类软件专注于照片和视频管理，具备强大的 AI 识别能力。

- **局限性：** 它们通常资源占用极高（需要运行 TensorFlow/PyTorch 模型），且数据库结构复杂（通常依赖外部 PostgreSQL 和 Redis），对于只想管理通用文件（文档、安装包、代码片段）的用户来说过于臃肿 5。
    

### 2.2 技术栈选型分析

基于“轻量级”、“高性能”和“WebDAV 支持”的需求，后端语言的选择主要集中在 **Go (Golang)** 和 **Rust** 之间。

|**特性**|**Go (Golang)**|**Rust**|**决策建议**|
|---|---|---|---|
|**内存管理**|垃圾回收 (GC)。内存占用虽低但存在波动，GC 暂停可能影响高并发扫描时的响应 6。|仿射类型系统 (Ownership)。无 GC，内存占用极低且确定性强，适合资源极度受限的嵌入式环境 6。|Rust 在极致资源受限环境下表现更佳。|
|**WebDAV 生态**|标准库 `net/http` 及扩展库 `x/net/webdav` 提供了基础支持，但作为客户端的高级封装较少，需手动处理 XML 解析和认证细节 7。|**OpenDAL** 库提供了革命性的统一数据访问层，能够以统一的 API 操作本地文件系统、WebDAV、S3 等多种存储后端，极大降低了多源支持的开发成本 8。|Rust 配合 OpenDAL 具有显著架构优势。|
|**开发效率**|语法简单，编译速度快，不仅有大量现成的 Web 框架（Gin, Echo），且 Docker 生态工具链极其成熟 9。|学习曲线陡峭，编译时间长。但在并发安全性及类型系统的严谨性上远超 Go，能有效避免运行时错误 6。|Go 更适合快速迭代 MVP。|
|**社区案例**|FileBrowser, Syncthing, Rclone 等知名 NAS 工具均由 Go 构建 1。|Spacedrive, Immich (部分组件) 等新一代高性能工具开始转向 Rust 11。|两者皆为主流选择。|

架构决策：

尽管 Go 语言在 NAS 工具开发中占据主导地位，但鉴于本项目对 WebDAV 及其未来扩展性（如 S3 支持）的明确需求，Rust 结合 OpenDAL 库将提供更优雅的底层抽象 8。OpenDAL 能够屏蔽本地文件系统与 WebDAV 协议之间的差异，使上层业务逻辑（扫描、标签化）完全复用。然而，若考虑到开发者的上手难度和现有开源参考项目（如 FileBrowser）的丰富程度，Go 依然是一个稳健的选择。本报告后续的架构设计将兼容这两种语言的特性，但在涉及存储抽象层时，会重点参考 OpenDAL 的设计模式。

对于数据库，**SQLite** 是唯一符合“单文件、零配置”需求的选择 12。为解决 SQLite 在高并发写入（扫描时）和读取（浏览时）下的锁竞争问题，系统将强制开启 **WAL (Write-Ahead Logging)** 模式，并采用适当的连接池策略 13。

---

## 3. 产品核心理念与功能需求详解

### 3.1 资源库管理 (Resource Libraries)

资源库（Library）是 TagFlow 管理文件的顶层容器。与传统文件管理器直接挂载根目录不同，TagFlow 允许用户定义多个独立的资源库，每个资源库可以对应不同的物理位置和存储协议。

#### 3.1.1 本地资源库 (Local Library)

- **定义：** 指向 Docker 容器内挂载的某个路径（如 `/data/photos` 或 `/mnt/documents`）。
    
- **功能要求：**
    
    - 支持路径校验：系统应检查路径是否存在且具备读取权限。
        
    - 忽略规则（Ignore Patterns）：用户可配置 `.gitignore` 风格的规则，自动忽略系统文件（如 `.DS_Store`, `@eaDir`, `Thumbs.db`）或特定子目录 14。
        

#### 3.1.2 WebDAV 资源库 (WebDAV Library)

- **定义：** 指向远程 WebDAV 服务（如 Nextcloud, Synology Drive, Alist）的 URL。
    
- **技术挑战与要求：**
    
    - **连接管理：** 系统需存储 WebDAV 的 Endpoint URL、用户名及密码（需加密存储）。
        
    - **网络延迟优化：** 由于 WebDAV 建立在 HTTP 之上，遍历目录树（Tree Walk）会产生大量的网络请求往返（RTT）。需求要求系统在扫描时采用**持久连接（Keep-Alive）** 并行发出 `PROPFIND` 请求，以最大化吞吐量 15。
        
    - **缓存策略：** 为避免每次用户点击文件夹都产生网络请求，系统必须将 WebDAV 的目录结构缓存至本地 SQLite 数据库中。读取操作应优先查询本地索引，仅在执行“刷新”或“扫描”操作时与远程同步。
        

### 3.2 自动标签引擎 (Auto-Tagging Engine)

这是 TagFlow 区别于 FileBrowser 的核心功能。系统需要在文件索引阶段，通过一系列**“标签生成器（Taggers）”**流水线，将文件的元属性转化为语义标签。

#### 3.2.1 路径分词标签 (Path-to-Tag)

文件在文件系统中的路径本身就蕴含了用户的分类逻辑。系统应将路径中的每一层目录转换为一个独立的标签。

- **逻辑详述：**
    
    - 假设资源库根路径为 `/Libraries/Work`。
        
    - 文件路径为 `/Libraries/Work/Projects/2024/Design/logo.png`。
        
    - **相对路径提取：** 系统首先计算相对路径 `Projects/2024/Design/logo.png`。
        
    - **分词处理：** 提取目录层级 `Projects`, `2024`, `Design`。
        
    - **标签生成：** 系统自动创建（或关联）三个标签：`#Projects`, `#2024`, `#Design`。
        
- **层级控制需求：** 为了防止产生过多无意义的标签（如 `New Folder`），用户应能配置“根目录忽略层级”（Root Depth Ignore）。例如，设置忽略前 0 层，则所有子目录皆生成标签。
    

#### 3.2.2 拓展名与类型标签 (Extension & Type Tagging)

系统应内置常见文件拓展名的映射表，不仅生成拓展名标签，还应生成更高级的“类型”标签。

- **拓展名标签：** 直接基于后缀，如 `#ext:jpg`, `#ext:pdf`, `#ext:md`。
    
- **宏类型标签（Macro-Type）：**
    
    - `#type:image` ← `.jpg`, `.jpeg`, `.png`, `.gif`, `.heic`, `.webp`
        
    - `#type:video` ← `.mp4`, `.mkv`, `.mov`, `.avi`
        
    - `#type:audio` ← `.mp3`, `.flac`, `.wav`
        
    - `#type:code` ← `.js`, `.py`, `.go`, `.rs`, `.html`, `.css`
        
    - `#type:document` ← `.pdf`, `.doc`, `.docx`, `.xls`, `.ppt`
        
- **价值：** 用户可以直接点击 `#type:image` 查看所有图片，而无需分别筛选 jpg 和 png。
    

#### 3.2.3 基础元数据标签 (Metadata Tagging)

针对特定文件类型，系统应进行轻量级的头部解析（Header Parsing）以提取关键信息，避免全文件扫描带来的性能损耗。

- **时间维度：** 基于文件的 `mtime`（修改时间）生成时间标签，采用层级结构：`#year:2024`, `#month:2024-05`。
    
- **媒体维度（进阶需求）：**
    
    - 对于图片，若性能允许，提取 EXIF 中的相机型号或拍摄日期。
        
    - 对于视频，提取分辨率（如 `#720p`, `#1080p`）和时长区间（如 `#short`, `#long`）16。
        

### 3.3 基础文件操作与元数据管理

#### 3.3.1 虚拟视图与过滤

前端界面不再主要展示文件夹树，而是展示一个**分面搜索（Faceted Search）** 界面。

- **标签云/过滤器：** 侧边栏列出所有自动生成的标签类别（类型、年份、路径标签）。
    
- **多选过滤逻辑：** 用户选择 `#2023` 和 `#type:image` 时，系统执行 `AND` 逻辑，仅展示 2023 年的所有图片。
    
- **面包屑导航：** 面包屑不再表示物理路径，而是表示当前的过滤上下文（例如：`Home > #type:image > #2023`）。
    

#### 3.3.2 文件管理操作

尽管系统强调非侵入性，但基础的文件管理功能依然必要。

- **支持操作：** 重命名（Rename）、移动（Move）、删除（Delete）、下载（Download）。
    
- **数据一致性：** 当用户在 TagFlow 中重命名文件时，系统必须同步修改物理文件系统（或 WebDAV），并更新数据库中的索引，确保标签关联不丢失。这要求数据库设计中文件 ID 必须是持久的，且与物理路径解耦（通过 ID 关联标签，而非通过路径关联）。
    

---

## 4. 系统架构设计 (System Architecture)

### 4.1 总体架构模式

系统采用**六边形架构（Hexagonal Architecture）**，也称为端口与适配器模式。这种设计将核心业务逻辑（资源管理、标签引擎）与外部基础设施（Web 接口、文件系统、数据库）解耦，使得系统能够灵活地在本地磁盘和 WebDAV 之间切换底层实现。

代码段

```
graph TD
    User -->|HTTP/REST| WebAdapter
    
    subgraph "TagFlow 核心域 (Core Domain)"
        LibService[资源库服务]
        TagEngine[标签引擎]
        SearchEngine[检索引擎]
        MetaExtractor[元数据提取器]
    end
    
    WebAdapter --> LibService
    WebAdapter --> SearchEngine
    
    LibService --> Scanner[扫描编排器]
    LibService --> TagEngine
    Scanner --> MetaExtractor
    
    subgraph "基础设施适配层 (Infrastructure Adapters)"
        DBAdapter
        FSAdapter
        ThumbAdapter[缩略图生成器 (FFmpeg/Imaging)]
    end
    
    LibService --> FSAdapter
    Scanner --> FSAdapter
    SearchEngine --> DBAdapter
    TagEngine --> DBAdapter
    MetaExtractor --> ThumbAdapter
    
    DBAdapter --> SQLite
    FSAdapter --> LocalDisk[本地磁盘]
    FSAdapter --> RemoteDAV
```

### 4.2 后端技术栈深度分析

#### 4.2.1 语言与框架

鉴于 WebDAV 的复杂性，若采用 **Go** 语言：

- **Web 框架：** 使用 **Gin** 或 **Echo**。它们轻量、高性能且路由定义直观。
    
- **WebDAV 客户端：** 使用 `github.com/studio-b12/gowebdav` 库进行基础操作。需注意 Go 的标准库 `net/http` 对长连接支持良好，但需手动处理 XML 响应解析。
    
- **并发模型：** Go 的 Goroutines 非常适合并发扫描大量文件。可以为每个目录启动一个 Goroutine（受 Worker Pool 限制），并行发送 `PROPFIND` 请求。
    

若采用 **Rust** 语言（推荐用于追求极致性能与抽象）：

- **Web 框架：** 使用 **Axum**。它基于 Tokio 异步运行时，吞吐量极高。
    
- **存储抽象：** 核心组件集成 **OpenDAL**。OpenDAL 8 提供了统一的 `Operator` 接口，无论是 `fs` 还是 `webdav`，上层调用的 API 完全一致（如 `op.list()`, `op.read()`）。这将极大地简化扫描器的代码逻辑，减少为不同协议编写适配器的工作量。
    
- **编译产物：** Rust 编译出的二进制文件通常比 Go 更小（Go 二进制包含运行时和 GC），且内存占用更稳定（无 GC 抖动）。
    

**结论：** 考虑到长期的维护性和对 WebDAV 的深度支持，建议优先考虑 Rust + OpenDAL 方案。如果开发团队对 Go 更熟悉，Go 方案在性能上也是完全可接受的（FileBrowser 证明了这一点）。

### 4.3 数据持久层设计 (SQLite Strategy)

数据库是系统的核心，存储着文件索引与标签网络。设计必须兼顾写入性能（扫描时）和查询性能（浏览时）。

#### 4.3.1 数据库模式设计 (Schema Design)

SQL

```
-- 资源库配置表
CREATE TABLE libraries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    type TEXT NOT NULL, -- 'local', 'webdav'
    config JSON NOT NULL, -- 存储路径、URL、认证信息
    last_scanned_at DATETIME
);

-- 标签定义表
CREATE TABLE tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    type TEXT NOT NULL, -- 'system' (类型), 'path' (路径), 'user' (手动)
    parent_id INTEGER, -- 支持标签层级（可选）
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(name, type) -- 防止重复标签
);

-- 文件索引表
CREATE TABLE files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id INTEGER NOT NULL,
    parent_path TEXT NOT NULL, -- 父目录路径，用于快速构建目录树
    filename TEXT NOT NULL,
    extension TEXT,
    size INTEGER,
    mtime INTEGER, -- 修改时间戳，用于检测变更
    mime_type TEXT,
    hash TEXT, -- 内容哈希（部分哈希），用于去重和移动检测
    FOREIGN KEY(library_id) REFERENCES libraries(id) ON DELETE CASCADE
);

-- 文件-标签 关联表 (多对多)
CREATE TABLE file_tags (
    file_id INTEGER NOT NULL,
    tag_id INTEGER NOT NULL,
    source TEXT DEFAULT 'auto', -- 'auto' 或 'manual'
    PRIMARY KEY(file_id, tag_id),
    FOREIGN KEY(file_id) REFERENCES files(id) ON DELETE CASCADE,
    FOREIGN KEY(tag_id) REFERENCES tags(id) ON DELETE CASCADE
);

-- 索引优化
CREATE INDEX idx_files_library_path ON files(library_id, parent_path);
CREATE INDEX idx_file_tags_tag_id ON file_tags(tag_id); -- 加速标签过滤
CREATE INDEX idx_files_mtime ON files(mtime); -- 加速时间线视图
```

#### 4.3.2 性能优化策略

- **WAL 模式：** 必须执行 `PRAGMA journal_mode=WAL;`。这允许读取操作与写入操作并发执行，避免扫描大文件夹时界面卡顿 13。
    
- **批量插入：** 扫描器在入库时应使用事务（Transaction），每 1000 条记录提交一次，而非逐条插入，这能提升 SQLite 写入性能数个数量级。
    
- **规范化与反规范化：** 虽然 `file_tags` 表提供了最大的灵活性，但为了查询性能，可以在 `files` 表中增加一个 `cached_tags` JSON 列，存储该文件的所有标签 ID 列表，以减少复杂查询时的 JOIN 操作（视实际性能测试结果而定）。
    

### 4.4 前端架构与交互设计

#### 4.4.1 虚拟滚动 (Virtual Scrolling)

当一个标签（如 `#type:image`）下关联了 50,000 张图片时，直接渲染 DOM 会导致浏览器崩溃。前端必须实现**虚拟滚动** 17。

- **实现原理：** 仅渲染视口（Viewport）内可见的元素（例如 20 行 x 5 列 = 100 个项目）。随着用户滚动，动态回收并更新这些 DOM 元素的内容。
    
- **技术选型：** Vue 生态中使用 `vue-virtual-scroller`，React 生态中使用 `tanstack-virtual` 或 `react-window`。
    
- **布局挑战：** 网格布局（Grid Layout）的虚拟化比列表更复杂，需要预先计算每个 Item 的高度。对于等高网格（常见文件管理视图），计算较为简单；若实现瀑布流（Masonry），则需要服务端预先返回图片的宽高比 19。
    

#### 4.4.2 状态管理

前端是一个富客户端应用（SPA）。需要使用 Pinia (Vue) 或 Redux/Zustand (React) 管理以下全局状态：

- **CurrentQuery:** 当前选中的标签集合、搜索关键词、排序方式。
    
- **Selection:** 当前选中的文件 ID 列表（用于批量操作）。
    
- **ScanningStatus:** 扫描任务的进度，用于在界面展示进度条或 Loading 状态。
    

---

## 5. 关键算法与实现细节

### 5.1 扫描与差异同步算法 (Scanning & Diff Algorithm)

为了保证“轻量级”，扫描器不能每次都全量重写数据库。必须实现基于“差异比对（Differential Sync）”的更新逻辑。

**算法伪代码逻辑：**

1. **快照获取：** 从数据库中加载该 Library 下所有文件的 `(path, size, mtime)` 映射表，记为 `DB_Snapshot`。
    
2. **文件系统遍历：**
    
    - 开始递归遍历（Walk）文件系统。
        
    - 对于遍历到的每个文件 `File_F`：
        
        - **路径检查：** 检查 `File_F.path` 是否存在于 `DB_Snapshot`。
            
        - **情况 A（未变）：** 若存在，且 `size` 和 `mtime` 均一致 -> 从 `DB_Snapshot` 中移除该记录，跳过后续处理。
            
        - **情况 B（修改）：** 若存在，但 `size` 或 `mtime` 不一致 -> 更新数据库中的记录，重新触发“自动标签引擎”解析元数据，从 `DB_Snapshot` 中移除。
            
        - **情况 C（新增）：** 若不存在 -> 在数据库插入新记录，触发“自动标签引擎”。
            
3. **清理阶段：**
    
    - 遍历结束后，`DB_Snapshot` 中剩余的记录即为**已删除**的文件。
        
    - 执行批量删除操作，清理数据库中对应的文件及其关联关系。
        

移动检测优化：

为了防止文件移动导致的用户手动标签丢失（视为删除+新增），系统可以在“情况 C”发生时，先计算新文件的“部分哈希”（读取文件头尾各 4KB 进行哈希）。若在“已删除”列表中找到相同哈希和大小的文件，则判定为**移动（Move）**操作，仅更新 path 字段，保留其关联的标签 ID。

### 5.2 WebDAV 集成策略

WebDAV 协议基于 HTTP，本质上是无状态的。

- **ETag 利用：** 标准 WebDAV 服务器会在 `PROPFIND` 响应中返回 `getetag` 属性 20。系统应存储此 ETag。在下次扫描时，如果 ETag 未变，则无需读取文件内容即可判定文件未修改。
    
- **深度限制与分页：** 部分 WebDAV 服务器不支持无限深度的递归 `PROPFIND`（Header `Depth: infinity`）。扫描器必须实现基于层级的递归逻辑，即先列出根目录，再对每个子目录发起请求。
    
- **并发控制：** 远程服务器通常有连接数限制。必须实现一个**信号量（Semaphore）**机制，限制并发 HTTP 请求数（例如默认为 4 或 8），防止触发服务器的 DDoS 防护或导致超时。
    

### 5.3 缩略图与预览生成管线

由于是 NAS 部署，CPU 资源宝贵，缩略图生成必须是**异步**且**低优先级**的。

- **生成时机：** 不要在扫描阶段生成缩略图（会极度拖慢扫描速度）。应在文件入库后，将生成任务推入一个后台队列（Queue）。
    
- **存储策略：** 缩略图不应写入用户的文件目录（保持非侵入性）。应存储在 Docker 容器挂载的专用 `/cache` 卷中。建议使用文件内容的哈希（如 SHA256）作为缩略图文件名，这样即使文件移动，缩略图依然有效，无需重新生成。
    
- **视频处理：** 使用 `ffmpeg` 截取视频前 10% 处的一帧作为封面。需限制 `ffmpeg` 的 CPU 权重（如使用 `nice` 命令）以避免系统过载 21。
    
- **格式选择：** 建议生成 **WebP** 格式的缩略图，相比 JPEG 在同等画质下体积更小，且支持透明通道（对 PNG 预览很重要）。
    

---

## 6. 非功能性需求与部署 (NFR & Deployment)

### 6.1 性能指标要求

- **冷启动内存：** 容器启动后空闲内存占用应 < **50MB**。
    
- **扫描内存峰值：** 在扫描包含 10 万文件的库时，内存占用应控制在 **300MB** 以内。这要求扫描过程中避免将整个文件列表加载到内存，而是采用流式处理或分批处理。
    
- **浏览响应：** 在 10 万级文件库中，标签过滤查询的 API 响应时间应 < **200ms**。这强依赖于数据库索引的正确建立。
    

### 6.2 容器化部署方案

为了满足“个人用户易于部署”的需求，必须提供 Docker 镜像。

**Dockerfile 策略（多阶段构建）：**

1. **前端构建阶段（Node.js）：** 编译 Vue/React 代码，生成静态资源（HTML/CSS/JS）。
    
2. **后端构建阶段（Go/Rust）：** 静态编译二进制文件，去除符号表（Strip symbols）以减小体积。
    
3. **最终镜像（Alpine/Distroless）：**
    
    - 复制后端二进制文件。
        
    - 复制前端静态资源到 `/public` 目录。
        
    - 安装必要的运行时依赖：`ffmpeg`（用于视频缩略图）、`ca-certificates`（用于 HTTPS WebDAV）。
        
    - 最终镜像大小目标：< **100MB**（含 ffmpeg）。
        

**Docker Compose 示例配置：**

YAML

```
version: '3.8'
services:
  tagflow:
    image: tagflow/core:latest
    container_name: tagflow
    restart: unless-stopped
    ports:
      - "8080:8080"
    volumes:
      -./data:/app/data         # 存放 SQLite 数据库
      -./cache:/app/cache       # 存放缩略图缓存
      - /mnt/photos:/library/photos:ro  # 只读挂载本地资源
    environment:
      - TZ=Asia/Shanghai
      - TAGFLOW_DB_PATH=/app/data/tagflow.db
```

### 6.3 安全性考量

- **只读模式（Read-Only Mode）：** 默认情况下，系统应以只读模式挂载用户资源库，仅在数据库中写入元数据。这能给予用户最大的安全感，防止软件 Bug 误删文件。
    
- **WebDAV 凭证安全：** 存储在数据库中的 WebDAV 密码必须经过加密（如 AES-256）处理，密钥可由用户在启动时通过环境变量提供，或随机生成并存储在配置卷中。
    

---

## 7. 开发路线图与阶段规划 (Roadmap)

### 第一阶段：核心原型 (MVP)

- **目标：** 实现本地文件夹扫描、自动标签生成、列表展示。
    
- **交付物：**
    
    - 基于 Go/Rust 的扫描器 CLI 工具。
        
    - SQLite 数据库 schema 实现。
        
    - 简易 Web 界面，支持按“类型”和“路径”标签过滤文件。
        
    - 仅支持本地文件系统。
        

### 第二阶段：WebDAV 与 缩略图

- **目标：** 支持远程资源库，优化视觉体验。
    
- **交付物：**
    
    - 集成 WebDAV 客户端，实现远程索引。
        
    - 集成 FFmpeg 和图像处理库，实现后台缩略图生成队列。
        
    - 前端实现虚拟滚动网格视图。
        

### 第三阶段：高级交互与管理

- **目标：** 提升可用性，支持文件操作。
    
- **交付物：**
    
    - 实现文件重命名、移动、删除的 API 及前端交互。
        
    - 引入“手动标签”功能，允许用户添加自定义标签。
        
    - 发布 Docker Hub 官方镜像。
        

---

## 8. 结论

本报告定义了一款名为 **TagFlow** 的个人资源管理系统的完整架构。通过放弃传统的层级管理执念，转向基于元数据的灵活组织，TagFlow 能够解决个人数据管理中日益严重的“分类瘫痪”问题。

架构设计的核心权衡在于**性能**与**功能**的平衡：通过采用 Go/Rust 编译型语言和 SQLite 嵌入式数据库，系统成功将资源占用控制在极低水平，使其能够在低端 NAS 上流畅运行；通过引入**自动化标签引擎**和**虚拟文件系统视图**，系统在不改变用户原有文件结构的前提下，提供了现代化的管理体验。这种“非侵入式”、“轻量级”且“自动化”的设计思路，精准击中了当前自托管社区的痛点，具备极高的实用价值和开源推广潜力。