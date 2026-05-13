# ClipboardCat - 项目记忆文档

## 项目概述

ClipboardCat（clipcat）是一个基于 Tauri 2 + Vue 3 的 macOS 剪切板管理工具，参考 Paste 软件的交互设计。应用以浮动面板形式从屏幕底部升起，监听系统剪切板变化并以卡片形式展示历史内容。

## 技术栈

- **后端**: Rust + Tauri 2
- **前端**: Vue 3 + Vite
- **关键依赖**:
  - `tauri-nspanel` (v2.1 branch) - macOS NSPanel 支持，使窗口层级高于 Dock
  - `tauri-plugin-global-shortcut` - 全局快捷键
  - `tauri-plugin-clipboard-manager` - 剪切板读写
  - `tokio` - 异步运行时（动画）
  - `cocoa` / `objc` - macOS 原生 API（已引入但当前未使用原生样式代码）

## 项目结构

```
clipcat/
├── src/                          # 前端源码
│   └── App.vue                   # 主界面组件（搜索、标签、卡片列表）
├── src-tauri/                    # 后端源码
│   ├── src/
│   │   ├── lib.rs                # 核心逻辑：窗口管理、动画、事件处理
│   │   ├── shortcuts.rs          # 快捷键模块
│   │   └── clipboard.rs          # 剪切板监听模块
│   ├── capabilities/default.json # Tauri 权限配置
│   ├── tauri.conf.json           # Tauri 窗口配置
│   └── Cargo.toml                # Rust 依赖
└── package.json                  # 前端依赖
```

## 核心功能实现

### 1. 窗口管理 (lib.rs)

- **窗口类型**: NSPanel（通过 tauri-nspanel），层级设为 `PanelLevel::Dock`，确保在 Dock 之上
- **窗口尺寸**: 宽度 = 屏幕宽度，高度 = 屏幕高度 × 40%（`WINDOW_HEIGHT_RATIO = 0.40`）
- **窗口位置**: 水平居中，垂直方向紧贴屏幕底部
- **圆角**: 左上和右上 16px 圆角（通过 CSS `border-top-left-radius` / `border-top-right-radius` 实现）
- **背景色**: `#b5b6b9`
- **透明背景**: `transparent: true` + `macOSPrivateApi: true`

### 2. 窗口动画 (lib.rs)

- **升起动画**: 从屏幕下方 800px 处（`RISE_OFFSET = 800`）缓动上升至目标位置，使用 ease-out 三次方曲线
- **下落动画**: 从当前位置缓动下降 800px 后隐藏窗口，使用 ease-in 三次方曲线
- **动画参数**: 30 步（`ANIMATION_STEPS = 30`），每步 16ms（`ANIMATION_STEP_DURATION_MS = 16`）
- **触发时机**:
  - 升起: 点击 Dock 图标 / 快捷键 Cmd+Shift+F
  - 下落: 窗口失焦 / 按 ESC

### 3. 失焦处理 (lib.rs)

- 使用 `last_show_time` 时间戳机制，防止窗口刚显示就因失焦而关闭
- 失焦后延迟 100ms 才允许触发下落动画（`time_since_show > 100`）
- 通过 `AtomicBool` (`window_visible`) 跟踪窗口可见状态，防止重复触发

### 4. 快捷键 (shortcuts.rs)

| 平台 | 打开窗口 | 关闭窗口 |
|------|---------|---------|
| macOS | Cmd+Shift+F | Escape |
| 其他 | Ctrl+F1 | Escape |

- 使用 `tauri-plugin-global-shortcut` 注册全局快捷键
- 打开/关闭分别调用 `animate_window_rise` / `animate_window_fall`

### 5. 剪切板监听 (clipboard.rs)

- **监听方式**: 每 500ms 轮询一次剪切板内容
- **支持内容**: 纯文本（Text）和图片（Image，含 RGBA 原始数据）
- **去重机制**: 通过自定义哈希算法对比上次内容，仅内容变化时触发事件
- **事件发射**: 通过 `app_handle.emit("clipboard-change", &item)` 发送到前端
- **数据结构**:
  ```rust
  enum ClipboardContent { Text(String), Image { width, height, rgba: Vec<u8> } }
  struct ClipboardItem { content_type: String, content: ClipboardContent, timestamp: u64 }
  ```

### 6. 前端界面 (App.vue)

#### 顶部栏（浮动居中）
- **搜索**: 默认显示放大镜图标，点击展开输入框（140px 宽），支持文本搜索，失焦或 ESC 收起
- **标签**:
  - "Clipboard" 为默认标签（无圆点），点击显示所有历史内容
  - 自定义标签带彩色小圆点
  - 点击 + 按钮展开窄输入框（70px）新增标签
  - 新标签自动分配颜色（6 色循环）
- **过渡动画**: 搜索框和新增标签输入框均有宽度+透明度过渡动画

#### 卡片列表
- **布局**: 水平排列，每张卡片 70vh × 70vh
- **卡片结构**: 彩色头部（类型+时间）→ 内容区 → 底部信息栏
- **文本卡片**: 保留换行格式，最多显示 200 字符
- **图片卡片**: 通过 Canvas 将 RGBA 数据转为 Blob URL 显示
- **动画**: 新卡片缩放淡入，悬停上浮，最多保留 30 条记录
- **空状态**: 显示图标 + 提示文字

## 已知问题与注意事项

1. **原生样式代码已移除**: 曾尝试通过 cocoa/objc 实现 macOS 原生圆角和背景色，但导致运行时 panic 和 foreign exception，已改用 CSS 实现
2. **图片数据量大**: RGBA 原始数据通过 Tauri 事件传递，大图片可能导致性能问题
3. **标签过滤未实现**: 标签可以切换选中状态，但尚未实现按标签过滤剪切板内容
4. **数据未持久化**: 标签和剪切板内容仅在内存中，应用重启后丢失
5. **cocoa/objc 依赖**: Cargo.toml 中仍保留但当前未使用

## Tauri 权限配置 (capabilities/default.json)

- 窗口操作: set-size, set-position, set-always-on-top, current-monitor, show, hide, set-focus
- 全局快捷键: register, unregister, is-registered
- 剪切板: read-text, read-image, write-text, write-image

## 开发命令

```bash
cd clipcat
pnpm tauri dev    # 开发模式
pnpm tauri build  # 构建生产版本
```
