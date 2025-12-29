# TagFlow UI

TagFlow 资源管理系统的前端界面，基于 Vue 3 + TypeScript + Vite 构建。

## 技术栈

- **Vue 3** - 渐进式 JavaScript 框架
- **TypeScript** - 类型安全
- **Vite** - 下一代前端构建工具
- **Pinia** - Vue 状态管理
- **Axios** - HTTP 客户端
- **TailwindCSS** - 实用优先的 CSS 框架
- **vue-virtual-scroller** - 虚拟滚动组件
- **lucide-vue-next** - 图标库

## 开发指南

### 安装依赖

```bash
npm install
```

### 启动开发服务器

```bash
npm run dev
```

前端运行在 `http://localhost:5173`，API 请求会代理到后端 `http://localhost:8080`。

### 构建生产版本

```bash
npm run build
```

### 预览生产构建

```bash
npm run preview
```

## 项目结构

```
tagflow-ui/
├── src/
│   ├── components/     # Vue 组件
│   │   ├── TagItem.vue      # 标签树递归组件
│   │   └── FileGrid.vue     # 文件虚拟滚动网格
│   ├── stores/         # Pinia 状态管理
│   │   └── useResourceStore.ts  # 资源状态 Store
│   ├── App.vue         # 主应用组件
│   ├── main.ts         # 应用入口
│   └── style.css       # 全局样式
├── index.html
├── package.json
├── vite.config.ts
├── tsconfig.json
└── tailwind.config.js
```

## 功能特性

- **层级标签树** - 递归展示标签层级结构
- **虚拟滚动网格** - 高性能渲染大量文件
- **实时筛选** - 点击标签即时过滤文件
- **响应式设计** - 适配不同屏幕尺寸

## 后端要求

确保后端服务运行在 `http://localhost:8080`。

启动后端：
```bash
cd ../tagflow-core
cargo run
```
