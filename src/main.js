const { app, BrowserWindow, globalShortcut, clipboard, ipcMain, protocol, net } = require('electron');
const path = require('path');
const MdictParser = require('./mdict-parser');
const Store = require('electron-store');

// 持久化存储配置
const store = new Store();
const DEFAULT_HOTKEY = 'Alt+M';

let mainWindow = null;
let lookupWindow = null;
let settingsWindow = null;
let dictionary = null;
let currentHotkey = store.get('hotkey', DEFAULT_HOTKEY);
let clipboardMonitorEnabled = store.get('clipboardMonitor', true);

// 显示设置
let displaySettings = {
  fontFamily: store.get('fontFamily', 'Segoe UI'),
  fontSize: store.get('fontSize', '14'),
  lineHeight: store.get('lineHeight', '1.6')
};

// 配置词典路径
const DICTIONARY_PATH = 'D:\\Documents\\词典\\牛津高阶英汉双解词典(第9版)_v20191111';
const MDX_FILE = path.join(DICTIONARY_PATH, '牛津高阶英汉双解词典(第9版).mdx');
const MDD_FILE = path.join(DICTIONARY_PATH, '牛津高阶英汉双解词典(第9版).mdd');
const CSS_FILE = path.join(DICTIONARY_PATH, 'oalecd9.css');

// 资源缓存
const resourceCache = new Map();

// 转换快捷键格式为 Electron accelerator 格式
function parseHotkey(hotkey) {
  const parts = hotkey.split('+').map(p => p.trim().toLowerCase());
  const acceleratorParts = [];

  parts.forEach(part => {
    switch(part) {
      case 'ctrl':
      case 'control':
        acceleratorParts.push('CommandOrControl');
        break;
      case 'cmd':
      case 'command':
        acceleratorParts.push('CommandOrControl');
        break;
      case 'alt':
        acceleratorParts.push('Alt');
        break;
      case 'shift':
        acceleratorParts.push('Shift');
        break;
      default:
        // 主键
        acceleratorParts.push(part.toUpperCase());
    }
  });

  return acceleratorParts.join('+');
}

// 注册全局快捷键
function registerGlobalHotkey(hotkey) {
  const accelerator = parseHotkey(hotkey);

  // 注销旧的快捷键
  globalShortcut.unregisterAll();

  // 注册新的快捷键
  const success = globalShortcut.register(accelerator, () => {
    const selectedText = clipboard.readText();
    if (selectedText && selectedText.trim()) {
      createLookupWindow();
      lookupWord(selectedText);
    }
  });

  if (success) {
    console.log(`Global hotkey registered: ${hotkey} (${accelerator})`);
    currentHotkey = hotkey;
    store.set('hotkey', hotkey);
  } else {
    console.error(`Failed to register hotkey: ${hotkey} (${accelerator})`);
  }

  return success;
}

// 创建主窗口（配置界面）
function createMainWindow() {
  mainWindow = new BrowserWindow({
    width: 550,
    height: 650,
    resizable: false,
    webPreferences: {
      nodeIntegration: true,
      contextIsolation: false
    },
    title: 'QuickDict Configuration'
  });

  mainWindow.loadFile('src/index.html');

  mainWindow.on('closed', () => {
    mainWindow = null;
  });
}

// 创建查询弹窗
function createLookupWindow() {
  if (lookupWindow) {
    lookupWindow.focus();
    return lookupWindow;
  }

  lookupWindow = new BrowserWindow({
    width: 600,
    height: 700,
    frame: true,
    resizable: true,
    alwaysOnTop: true,
    skipTaskbar: false,
    autoHideMenuBar: true,
    webPreferences: {
      nodeIntegration: true,
      contextIsolation: false,
      webSecurity: false // 允许加载本地资源
    },
    title: 'Dictionary Lookup'
  });

  lookupWindow.loadFile('src/lookup.html');

  // 失去焦点时自动关闭（可选）
  lookupWindow.on('blur', () => {
    // lookupWindow.close();
  });

  lookupWindow.on('closed', () => {
    lookupWindow = null;
  });

  return lookupWindow;
}

// 创建设置窗口
function createSettingsWindow() {
  if (settingsWindow) {
    settingsWindow.focus();
    return settingsWindow;
  }

  settingsWindow = new BrowserWindow({
    width: 520,
    height: 550,
    resizable: false,
    modal: true,
    parent: mainWindow,
    webPreferences: {
      nodeIntegration: true,
      contextIsolation: false
    },
    title: 'QuickDict Settings'
  });

  settingsWindow.loadFile('src/settings.html');

  settingsWindow.on('closed', () => {
    settingsWindow = null;
  });

  return settingsWindow;
}

// 查询单词
async function lookupWord(word) {
  if (!word || !word.trim()) return;

  if (!dictionary) {
    console.log('Loading dictionary...');
    try {
      dictionary = new MdictParser(MDX_FILE, MDD_FILE, CSS_FILE, displaySettings);
      await dictionary.load();
      console.log('Dictionary loaded successfully');
    } catch (error) {
      console.error('Failed to load dictionary:', error);
      return;
    }
  } else {
    // 更新显示设置
    dictionary.updateDisplaySettings(displaySettings);
  }

  const result = await dictionary.lookup(word.trim());

  // 发送结果到查询窗口
  if (lookupWindow && lookupWindow.webContents) {
    lookupWindow.webContents.send('lookup-result', {
      word: word,
      result: result
    });
  }
}

// 监听剪贴板变化
let lastClipboardText = '';

function checkClipboard() {
  const text = clipboard.readText();
  if (text && text !== lastClipboardText) {
    lastClipboardText = text;

    // 如果是英文单词，则查询
    if (/^[a-zA-Z\s\-']+$/.test(text.trim()) && text.trim().length > 0) {
      createLookupWindow();
      lookupWord(text);
    }
  }
}

// 监听剪贴板
let clipboardInterval = null;

function startClipboardMonitor() {
  if (clipboardInterval) return;

  clipboardInterval = setInterval(checkClipboard, 500);
}

function stopClipboardMonitor() {
  if (clipboardInterval) {
    clearInterval(clipboardInterval);
    clipboardInterval = null;
  }
}

// IPC 通信处理
ipcMain.on('lookup-word', (event, word) => {
  createLookupWindow();
  lookupWord(word);
});

ipcMain.on('open-settings', () => {
  createSettingsWindow();
});

// 同步获取显示设置
ipcMain.on('get-display-settings', (event) => {
  event.returnValue = {
    ...displaySettings,
    clipboardMonitor: clipboardMonitorEnabled
  };
});

// 设置字体
ipcMain.on('set-font-family', (event, fontFamily) => {
  displaySettings.fontFamily = fontFamily;
  store.set('fontFamily', fontFamily);

  // 更新解析器设置
  if (dictionary) {
    dictionary.updateDisplaySettings(displaySettings);
  }

  // 通知所有查询窗口更新字体
  BrowserWindow.getAllWindows().forEach(win => {
    if (win !== mainWindow && win !== settingsWindow) {
      win.webContents.send('update-display-settings', displaySettings);
    }
  });
});

// 设置字号
ipcMain.on('set-font-size', (event, fontSize) => {
  displaySettings.fontSize = fontSize;
  store.set('fontSize', fontSize);

  if (dictionary) {
    dictionary.updateDisplaySettings(displaySettings);
  }

  BrowserWindow.getAllWindows().forEach(win => {
    if (win !== mainWindow && win !== settingsWindow) {
      win.webContents.send('update-display-settings', displaySettings);
    }
  });
});

// 设置行高
ipcMain.on('set-line-height', (event, lineHeight) => {
  displaySettings.lineHeight = lineHeight;
  store.set('lineHeight', lineHeight);

  if (dictionary) {
    dictionary.updateDisplaySettings(displaySettings);
  }

  BrowserWindow.getAllWindows().forEach(win => {
    if (win !== mainWindow && win !== settingsWindow) {
      win.webContents.send('update-display-settings', displaySettings);
    }
  });
});

ipcMain.on('set-hotkey', (event, hotkey) => {
  const success = registerGlobalHotkey(hotkey);

  if (success) {
    if (mainWindow) {
      mainWindow.webContents.send('hotkey-updated', hotkey);
    }
    if (settingsWindow) {
      settingsWindow.webContents.send('hotkey-updated', hotkey);
    }
  }
});

ipcMain.on('toggle-clipboard-monitor', (event, enabled) => {
  clipboardMonitorEnabled = enabled;
  store.set('clipboardMonitor', enabled);

  if (enabled) {
    startClipboardMonitor();
  } else {
    stopClipboardMonitor();
  }
});

// 获取MDD资源
ipcMain.handle('get-mdd-resource', async (event, resourceName) => {
  if (!dictionary) {
    return null;
  }

  // 检查缓存
  const cacheKey = `mdd:${resourceName}`;
  if (resourceCache.has(cacheKey)) {
    return resourceCache.get(cacheKey);
  }

  try {
    const resource = await dictionary.getResource(resourceName);

    // 缓存资源
    if (resource) {
      resourceCache.set(cacheKey, resource);
    }

    return resource;
  } catch (error) {
    console.error('Failed to get resource:', error);
    return null;
  }
});

// 注册自定义协议处理MDD资源
function registerMddProtocol() {
  protocol.registerBufferProtocol('mdd-resource', (request, callback) => {
    const resourceName = request.url.replace('mdd-resource://', '');

    // 从缓存获取
    const cacheKey = `mdd:${resourceName}`;
    if (resourceCache.has(cacheKey)) {
      const resource = resourceCache.get(cacheKey);

      // 确定MIME类型
      let mimeType = 'application/octet-stream';
      if (resourceName.endsWith('.png')) {
        mimeType = 'image/png';
      } else if (resourceName.endsWith('.jpg') || resourceName.endsWith('.jpeg')) {
        mimeType = 'image/jpeg';
      } else if (resourceName.endsWith('.gif')) {
        mimeType = 'image/gif';
      } else if (resourceName.endsWith('.svg')) {
        mimeType = 'image/svg+xml';
      } else if (resourceName.endsWith('.mp3')) {
        mimeType = 'audio/mpeg';
      } else if (resourceName.endsWith('.wav')) {
        mimeType = 'audio/wav';
      } else if (resourceName.endsWith('.ogg')) {
        mimeType = 'audio/ogg';
      } else if (resourceName.endsWith('.css')) {
        mimeType = 'text/css';
      } else if (resourceName.endsWith('.js')) {
        mimeType = 'application/javascript';
      }

      callback({
        mimeType: mimeType,
        data: Buffer.from(resource)
      });
    } else {
      // 如果没有缓存，尝试异步加载
      dictionary.getResource(resourceName).then(resource => {
        if (resource) {
          resourceCache.set(cacheKey, resource);

          let mimeType = 'application/octet-stream';
          if (resourceName.endsWith('.png')) mimeType = 'image/png';
          else if (resourceName.endsWith('.jpg') || resourceName.endsWith('.jpeg')) mimeType = 'image/jpeg';
          else if (resourceName.endsWith('.gif')) mimeType = 'image/gif';
          else if (resourceName.endsWith('.mp3')) mimeType = 'audio/mpeg';
          else if (resourceName.endsWith('.wav')) mimeType = 'audio/wav';
          else if (resourceName.endsWith('.ogg')) mimeType = 'audio/ogg';
          else if (resourceName.endsWith('.css')) mimeType = 'text/css';
          else if (resourceName.endsWith('.js')) mimeType = 'application/javascript';

          callback({
            mimeType: mimeType,
            data: Buffer.from(resource)
          });
        } else {
          callback({ error: -2 }); // 找不到资源
        }
      }).catch(() => {
        callback({ error: -6 }); // 加载失败
      });
    }
  });
}

// 应用程序就绪
app.whenReady().then(() => {
  // 注册MDD资源协议
  registerMddProtocol();

  createMainWindow();

  // 注册全局快捷键
  registerGlobalHotkey(currentHotkey);

  // 启动剪贴板监听（如果启用）
  if (clipboardMonitorEnabled) {
    startClipboardMonitor();
  }

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createMainWindow();
    }
  });
});

// 应用程序退出
app.on('will-quit', () => {
  globalShortcut.unregisterAll();
  stopClipboardMonitor();
});

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') {
    app.quit();
  }
});
