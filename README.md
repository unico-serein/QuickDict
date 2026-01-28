# QuickDict - MDX/MDD 词典查询工具

[English](README.en.md) | 简体中文

一个基于 Electron 的词典查询工具，支持 MDX/MDD 格式的词典文件。

## 功能特性

- 支持 MDX/MDD 格式词典
- 支持加载词典 CSS 样式
- 全局快捷键查询 (Ctrl+Alt+D)
- 剪贴板自动监听查询
- 弹窗显示查询结果
- 支持词典中的图片、音频等多媒体资源

## 使用方法

### 安装依赖

```bash
npm install
```

### 启动应用

```bash
npm start
```

### 查询单词

1. **全局快捷键**: 按 `Ctrl+Alt+D` 查询剪贴板中的文本
2. **复制查询**: 复制英文单词到剪贴板，应用会自动查询并弹出结果窗口
3. **测试查询**: 在配置界面输入单词并点击 "Lookup" 按钮

## 配置

词典路径配置在 `src/main.js` 中：

```javascript
const DICTIONARY_PATH = 'D:\\Documents\\词典\\牛津高阶英汉双解词典(第9版)_v20191111';
const MDX_FILE = path.join(DICTIONARY_PATH, '牛津高阶英汉双解词典(第9版).mdx');
const MDD_FILE = path.join(DICTIONARY_PATH, '牛津高阶英汉双解词典(第9版).mdd');
const CSS_FILE = path.join(DICTIONARY_PATH, 'oalecd9.css');
```

如需更换词典，请修改上述路径。

## 技术栈

- Electron - 桌面应用框架
- js-mdict - MDX/MDD 文件解析库
- Node.js - 后端运行时

## 项目结构

```
quickdict/
├── src/
│   ├── main.js           # 主进程代码
│   ├── mdict-parser.js   # MDX/MDD 解析器
│   ├── index.html        # 配置界面
│   ├── settings.html     # 设置界面
│   └── lookup.html       # 查询结果界面
├── package.json
├── README.md             # 中文文档
├── README.en.md          # 英文文档
└── .gitignore
```

## 注意事项

- 首次查询时需要加载词典文件，可能需要几秒钟时间
- 词典文件较大时，首次加载会有延迟
- 建议使用 SSD 存放词典文件以获得更好的性能

## 许可证

MIT
