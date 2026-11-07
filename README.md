# ClipW2 - Windows 剪贴板查看器

一个基于 Tauri v2 的 Windows 剪贴板查看工具，支持查看所有剪贴板格式的内容，包括文本、HTML、RTF、图片、文件列表和二进制数据。

## 功能特性

- **全格式支持**：读取所有标准剪贴板格式（CF_TEXT 到 CF_DIBV5）和注册的自定义格式
- **智能分类**：自动识别内容类型（文本/HTML/RTF/图片/文件/二进制），按可读性排序
- **HTML 预览**：支持源码/预览切换，预览使用 DOMPurify 清理并在沙箱 iframe 中渲染
- **图片显示**：支持 PNG、JPEG、GIF、BMP 格式，DIB/DIBv5 自动转换为可渲染格式
- **二进制查看**：Hex dump 显示，十六进制（红色）和 ASCII（绿色）着色
- **文件列表**：显示剪贴板中的文件路径，支持滚动和计数
- **自动刷新**：基于哈希的变化检测，仅在剪贴板内容改变时更新界面
- **分类过滤**：点击统计栏可按类型筛选显示

## 技术栈

### 后端（Rust）
- **Tauri v2**：应用框架
- **windows crate**：Windows API 访问（剪贴板、GDI、内存）
- **chardetng + encoding_rs**：字符编码检测和转换
- **serde + serde_json**：序列化

### 前端（TypeScript）
- **Vanilla TS**：无框架依赖
- **DOMPurify**：HTML 清理，防止 XSS
- **Vite**：构建工具

## 项目结构

```
ClipW2/
├── src/                    # 前端源码
│   ├── main.ts            # 主逻辑
│   ├── styles.css         # 样式
│   └── ...
├── src-tauri/             # Rust 后端
│   ├── src/
│   │   ├── lib.rs         # 入口，模块注册
│   │   ├── cache.rs       # 剪贴板数据缓存
│   │   ├── commands.rs    # Tauri IPC 命令
│   │   ├── readers.rs     # 格式读取器
│   │   ├── types.rs       # 数据结构
│   │   └── utils.rs       # 工具函数
│   └── Cargo.toml
└── index.html             # 主页面
```

## 构建和运行

### 前置要求
- Node.js 18+
- Rust (latest stable)
- Windows 10/11

### 安装依赖
```bash
npm install
```

### 开发模式
```bash
npm run tauri dev
```

### 构建发布版
```bash
npm run tauri build
```

构建产物位于 `src-tauri/target/release/`。

## 使用说明

1. **手动刷新**：点击右上角刷新按钮
2. **自动刷新**：勾选"自动刷新"复选框，每 2 秒检测一次剪贴板变化
3. **分类过滤**：点击顶部的统计标签（如 "3 TEXT"、"1 IMAGE"）筛选特定类型
4. **HTML 切换**：HTML 卡片支持"源码"和"预览"视图切换
5. **图片查看**：图片格式自动显示预览，DIB 格式会转换为 BMP 渲染

## 安全特性

- **XSS 防护**：HTML 预览使用 DOMPurify 清理，iframe 设置 `sandbox` 属性（无脚本执行）
- **类型安全**：使用 `ClipboardHandle` 枚举区分 HGLOBAL/HBITMAP/HENHMETAFILE/HDROP，防止堆损坏
- **输入清理**：所有用户可控数据在渲染前经过 `escapeHtml` 处理

## 技术细节

### 剪贴板格式处理

- **标准格式 1-17**：特殊处理，区分 HGLOBAL 和非 HGLOBAL 句柄
- **格式 2（CF_BITMAP）**：显示 HBITMAP 句柄值（无法直接渲染）
- **格式 8/17（CF_DIB/CF_DIBV5）**：前端转换为 BMP 格式渲染
- **格式 14（CF_ENHMETAFILE）**：显示为图片类型，无预览
- **格式 15（CF_HDROP）**：解析文件列表
- **注册格式（≥0xC000）**：通过格式名称智能识别内容类型

### 自动刷新优化

使用两层哈希检测避免不必要的数据传输：
1. `get_clipboard_hash`：快速检测剪贴板是否变化（格式 ID + 大小）
2. 仅在哈希变化时调用 `read_clipboard` 获取完整数据
3. 图片加载前通过 `get_format_hash` 比对，未变化则复用缓存

### 字符编码检测

使用 `chardetng` 进行编码检测，`encoding_rs` 进行解码，支持：
- UTF-8、UTF-16LE
- GBK、GB2312、GB18030
- Shift_JIS、EUC-JP
- 其他常见编码

## 许可证

MIT
