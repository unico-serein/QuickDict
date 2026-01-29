# RDict 构建指南

## 环境要求

1. **Rust** (1.75 或更高版本)
   - 安装: https://rustup.rs/
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **Tauri CLI**
   ```bash
   cargo install tauri-cli
   ```

3. **WebView2** (Windows)
   - 通常已预装在 Windows 10/11 上
   - 如未安装，可从 Microsoft 官网下载

## 构建步骤

### 1. 进入项目目录

```bash
cd rdict/src-tauri
```

### 2. 开发模式运行

```bash
cargo tauri dev
```

这会启动开发服务器，自动编译 Rust 代码并加载前端页面。

### 3. 构建发布版本

```bash
cargo tauri build
```

构建完成后，安装包位于:
- **Windows**: `target/release/bundle/msi/` 或 `target/release/bundle/nsis/`
- **macOS**: `target/release/bundle/dmg/`
- **Linux**: `target/release/bundle/deb/` 或 `target/release/bundle/appimage/`

## 项目结构说明

```
rdict/
├── src/                      # 前端代码 (HTML/CSS/JS)
│   ├── index.html           # 主配置界面
│   └── lookup.html          # 查询弹窗界面
│
└── src-tauri/               # Rust + Tauri 后端
    ├── src/
    │   ├── main.rs          # 主程序入口，包含 Tauri 命令
    │   ├── mdict.rs         # MDX/MDD 文件解析器
    │   ├── config.rs        # 配置管理
    │   └── lib.rs           # 库入口
    ├── icons/               # 应用图标
    ├── Cargo.toml           # Rust 依赖配置
    └── tauri.conf.json      # Tauri 应用配置
```

## 常见问题

### 1. 构建失败: "could not find native static library"

确保已安装平台特定的依赖:

**Windows:**
- 安装 Visual Studio Build Tools 或 Visual Studio Community
- 确保安装了 "Desktop development with C++" 工作负载

**macOS:**
- 安装 Xcode Command Line Tools:
  ```bash
  xcode-select --install
  ```

**Linux:**
```bash
# Ubuntu/Debian
sudo apt update
sudo apt install libwebkit2gtk-4.0-dev build-essential curl wget libssl-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev

# Fedora
sudo dnf install webkit2gtk4.0-devel openssl-devel curl wget libappindicator-gtk3-devel librsvg2-devel
```

### 2. 运行时找不到词典文件

- 在设置界面选择正确的词典文件夹
- 确保文件夹中包含 `.mdx` 文件
- 可选: 同时放置 `.mdd` (资源) 和 `.css` (样式) 文件

### 3. 全局快捷键不工作

- 某些快捷键可能被系统占用，尝试使用其他组合
- Windows 上需要管理员权限才能注册某些快捷键
- 尝试使用 `Alt+M`、`Ctrl+Shift+D` 等组合

## 开发调试

### 查看日志

开发模式下，日志会输出到终端。可以通过以下方式添加日志:

```rust
// 在 Rust 代码中
tracing::info!("这是一条信息日志");
tracing::debug!("这是一条调试日志: {:?}", some_variable);
```

### 前端调试

按 `F12` 或 `Ctrl+Shift+I` 打开开发者工具。

## 自定义配置

配置文件位置:
- **Windows**: `%APPDATA%\rdict\config.json`
- **macOS**: `~/Library/Application Support/rdict/config.json`
- **Linux**: `~/.config/rdict/config.json`

可以手动编辑此文件来修改:
- 词典路径
- 快捷键
- 显示设置
