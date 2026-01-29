# RDict - Rust 版 MDX/MDD 词典查询工具

基于 Rust + Tauri 的高性能词典查询工具，是 QuickDict 的 Rust 重新实现版本。

## 特性

- 🚀 **高性能**: 基于 Rust 实现，内存安全，执行效率高
- 📚 **MDX/MDD 支持**: 完整支持 MDX/MDD 格式词典文件
- 🎨 **现代 UI**: 基于 Tauri，使用 Web 技术构建美观界面
- ⌨️ **全局快捷键**: 支持自定义全局快捷键快速唤出
- 📋 **剪贴板监听**: 自动监听剪贴板，复制英文单词即查
- 🌐 **在线词典**: 本地查不到时自动使用 Free Dictionary API
- 💾 **LRU 缓存**: 智能缓存机制，提升重复查询速度
- 🖼️ **多媒体资源**: 支持词典中的图片、音频等资源

## 项目结构

```
rdict/
├── src-tauri/           # Rust + Tauri 后端
│   ├── src/
│   │   ├── main.rs      # 主程序入口
│   │   ├── mdict.rs     # MDX/MDD 解析器
│   │   ├── config.rs    # 配置管理
│   │   └── lib.rs       # 库入口
│   ├── icons/           # 应用图标
│   ├── Cargo.toml       # Rust 依赖配置
│   └── tauri.conf.json  # Tauri 配置
├── src/                 # 前端界面
│   ├── index.html       # 配置界面
│   ├── lookup.html      # 查询窗口
│   └── assets/          # 静态资源
└── README.md            # 本文件
```

## 快速开始

### 前置要求

- [Rust](https://rustup.rs/) (1.75+)
- [Node.js](https://nodejs.org/) (18+)

### 安装依赖

```bash
cd rdict/src-tauri
cargo build
```

### 开发模式运行

```bash
cd rdict/src-tauri
cargo tauri dev
```

### 构建发布版本

```bash
cd rdict/src-tauri
cargo tauri build
```

## 配置

应用配置存储在系统配置目录：

- **Windows**: `%APPDATA%\rdict\config.json`
- **macOS**: `~/Library/Application Support/rdict/config.json`
- **Linux**: `~/.config/rdict/config.json`

### 配置词典路径

1. 打开主界面（配置界面）
2. 点击"选择文件夹"选择包含 MDX/MDD 文件的文件夹
3. 点击"加载词典"

程序会自动检测文件夹中的：
- `.mdx` 文件 - 词典主文件
- `.mdd` 文件 - 资源文件（可选）
- `.css` 文件 - 样式文件（可选）

## 使用方法

### 查询单词

1. **全局快捷键**: 按 `Alt+M`（可自定义）唤出查询窗口
2. **剪贴板查询**: 复制英文单词后自动弹出查询结果（需在设置中开启）
3. **直接输入**: 在查询窗口输入单词，支持模糊搜索

### 快捷键

| 快捷键 | 功能 |
|--------|------|
| `Alt+M` (默认) | 唤出/关闭查询窗口 |
| `↑/↓` | 在单词列表中导航 |
| `Enter` | 查看选中单词详情 |
| `Escape` | 返回/关闭 |
| `Ctrl+1~9` | 快速选择列表中的单词 |

## 与原 Electron 版本的对比

| 特性 | Electron 版 | Rust/Tauri 版 |
|------|-------------|---------------|
| 安装包大小 | ~100MB+ | ~5MB |
| 内存占用 | ~200MB+ | ~50MB |
| 启动速度 | 较慢 | 快 |
| MDX 解析 | js-mdict (Node.js) | 原生 Rust 实现 |
| 安全性 | 良好 | 更好 (Rust 内存安全) |

## 技术栈

- **后端**: Rust + Tauri
- **前端**: HTML/CSS/JavaScript (Vanilla)
- **MDX/MDD 解析**: 自定义 Rust 实现
- **配置存储**: JSON 文件
- **HTTP 客户端**: reqwest (用于在线词典)

## 许可证

MIT License

## 致谢

- [Tauri](https://tauri.app/) - 构建跨平台桌面应用
- [js-mdict](https://github.com/terasum/js-mdict) - MDX/MDD 格式参考
- [Free Dictionary API](https://dictionaryapi.dev/) - 在线词典服务
