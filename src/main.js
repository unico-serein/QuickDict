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
let clipboardMonitorEnabled = store.get('clipboardMonitor', false);

// 显示设置
let displaySettings = {
  fontFamily: store.get('fontFamily', 'Segoe UI'),
  fontSize: store.get('fontSize', '14'),
  lineHeight: store.get('lineHeight', '1.6')
};

// 配置词典路径
const DICTIONARY_PATH = '/Users/fuyanxu/Documents/dict';
const MDX_FILE = path.join(DICTIONARY_PATH, '牛津高阶英汉双解词典（第9版）.mdx');
const MDD_FILE = path.join(DICTIONARY_PATH, '牛津高阶英汉双解词典（第9版）.mdd');
const CSS_FILE = path.join(DICTIONARY_PATH, 'oalecd9.css');

// 资源缓存（带大小限制的 LRU 缓存）
const MAX_CACHE_SIZE = 100;
const resourceCache = new Map();

// 简单的 LRU 缓存管理
function cacheSet(key, value) {
  // 如果缓存已满，删除最早的条目
  if (resourceCache.size >= MAX_CACHE_SIZE) {
    const firstKey = resourceCache.keys().next().value;
    resourceCache.delete(firstKey);
  }
  resourceCache.set(key, value);
}

function cacheGet(key) {
  if (resourceCache.has(key)) {
    // 移动到末尾（最近使用）
    const value = resourceCache.get(key);
    resourceCache.delete(key);
    resourceCache.set(key, value);
    return value;
  }
  return null;
}

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
    // 如果窗口已打开，则关闭窗口
    if (lookupWindow && !lookupWindow.isDestroyed()) {
      lookupWindow.close();
      return;
    }
    
    // 否则，打开空白窗口
    createLookupWindow();
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

// 默认窗口尺寸配置
const DEFAULT_WINDOW_SIZES = {
  initial: { width: 600, height: 52 },    // 只有搜索框
  list: { width: 600, height: 400 },      // 搜索框 + 单词列表
  definition: { width: 600, height: 600 } // 搜索框 + 详细解释
};

// 获取保存的窗口配置
function getSavedWindowBounds() {
  return store.get('lookupWindowBounds', null);
}

// 保存窗口配置
function saveWindowBounds() {
  if (lookupWindow && !lookupWindow.isDestroyed()) {
    const bounds = lookupWindow.getBounds();
    store.set('lookupWindowBounds', bounds);
  }
}

// 标记用户是否手动调整了窗口大小
let userResizedWindow = false;

// 创建查询弹窗
function createLookupWindow() {
  if (lookupWindow) {
    lookupWindow.focus();
    return lookupWindow;
  }

  // 获取保存的窗口配置
  const savedBounds = getSavedWindowBounds();
  
  // 如果有保存的配置，使用保存的配置；否则使用默认配置
  const windowConfig = savedBounds ? {
    x: savedBounds.x,
    y: savedBounds.y,
    width: savedBounds.width,
    height: savedBounds.height
  } : {
    width: DEFAULT_WINDOW_SIZES.initial.width,
    height: DEFAULT_WINDOW_SIZES.initial.height
  };

  // 如果有保存的配置，标记为用户已调整
  userResizedWindow = !!savedBounds;

  lookupWindow = new BrowserWindow({
    ...windowConfig,
    frame: false,           // 无标题栏
    resizable: true,
    alwaysOnTop: true,
    skipTaskbar: false,
    transparent: false,
    hasShadow: true,
    vibrancy: 'dark',       // macOS 毛玻璃效果
    visualEffectState: 'active',
    roundedCorners: true,
    webPreferences: {
      nodeIntegration: true,
      contextIsolation: false,
      webSecurity: false
    }
  });

  lookupWindow.loadFile('src/lookup.html');

  // 监听用户手动调整窗口大小
  lookupWindow.on('will-resize', () => {
    userResizedWindow = true;
  });

  // 监听窗口移动
  lookupWindow.on('moved', () => {
    saveWindowBounds();
  });

  // 监听窗口大小改变完成
  lookupWindow.on('resized', () => {
    saveWindowBounds();
  });

  lookupWindow.on('blur', () => {
    // 可选：失去焦点时关闭
    // lookupWindow.close();
  });

  lookupWindow.on('closed', () => {
    lookupWindow = null;
    userResizedWindow = false;
  });

  return lookupWindow;
}

// 调整窗口大小（仅在用户未手动调整时生效）
ipcMain.on('resize-window', (event, view) => {
  if (lookupWindow && DEFAULT_WINDOW_SIZES[view] && !userResizedWindow) {
    const { width, height } = DEFAULT_WINDOW_SIZES[view];
    lookupWindow.setSize(width, height, true); // true 表示动画过渡
  }
});

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
      // 发送单词到搜索框并触发搜索
      if (lookupWindow && lookupWindow.webContents) {
        lookupWindow.webContents.once('did-finish-load', () => {
          lookupWindow.webContents.send('set-search-word', text.trim());
        });
        // 如果窗口已经加载完成，直接发送
        if (!lookupWindow.webContents.isLoading()) {
          lookupWindow.webContents.send('set-search-word', text.trim());
        }
      }
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

// 网络词典 API
const ONLINE_DICT_API = 'https://api.dictionaryapi.dev/api/v2/entries/en/';

// IPC 通信处理
ipcMain.on('lookup-word', (event, word) => {
  createLookupWindow();
  lookupWord(word);
});

ipcMain.on('open-settings', () => {
  createSettingsWindow();
});

// 关闭查询窗口
ipcMain.on('close-lookup-window', () => {
  if (lookupWindow) {
    lookupWindow.close();
  }
});

// 搜索单词（返回列表）
ipcMain.on('search-words', async (event, query) => {
  if (!query || !query.trim()) {
    if (lookupWindow && lookupWindow.webContents) {
      lookupWindow.webContents.send('search-results', []);
    }
    return;
  }

  const results = [];
  const searchQuery = query.trim().toLowerCase();

  // 确保词典已加载
  if (!dictionary) {
    try {
      dictionary = new MdictParser(MDX_FILE, MDD_FILE, CSS_FILE, displaySettings);
      await dictionary.load();
    } catch (error) {
      console.error('Failed to load dictionary:', error);
    }
  }

  // 1. 本地词典搜索
  if (dictionary && dictionary.mdx) {
    try {
      // js-mdict 支持 prefix 和 suggest 方法
      let suggestions = [];
      
      // 尝试使用 prefix 方法（前缀匹配）
      if (typeof dictionary.mdx.prefix === 'function') {
        suggestions = dictionary.mdx.prefix(searchQuery);
      }
      
      // 如果 prefix 不可用或结果为空，使用 suggest 方法
      if ((!suggestions || suggestions.length === 0) && typeof dictionary.mdx.suggest === 'function') {
        suggestions = dictionary.mdx.suggest(searchQuery, 10);
      }
      
      if (suggestions && suggestions.length > 0) {
        for (const item of suggestions.slice(0, 10)) {
          const word = item.keyText || item.key || (typeof item === 'string' ? item : null);
          if (word) {
            const brief = await getWordBrief(word);
            results.push({
              word: word,
              brief: brief,
              source: 'local'
            });
          }
        }
      }
    } catch (error) {
      console.error('Local search error:', error);
    }
  }

  // 2. 网络词典搜索（如果本地结果不足且是英文单词）
  if (results.length < 3 && /^[a-zA-Z\-']+$/.test(searchQuery)) {
    try {
      const onlineResults = await searchOnlineDict(searchQuery);
      for (const item of onlineResults) {
        // 避免重复
        if (!results.find(r => r.word.toLowerCase() === item.word.toLowerCase())) {
          results.push(item);
        }
      }
    } catch (error) {
      console.error('Online search error:', error);
    }
  }

  // 发送结果
  if (lookupWindow && lookupWindow.webContents) {
    lookupWindow.webContents.send('search-results', results.slice(0, 10));
  }
});

// 获取单词简要释义
async function getWordBrief(word) {
  if (!dictionary) return '';

  try {
    const result = dictionary.mdx.lookup(word);
    if (result && result.definition) {
      // 移除 HTML 标签，提取第一行释义
      let text = result.definition
        .replace(/<script[^>]*>[\s\S]*?<\/script>/gi, '')
        .replace(/<style[^>]*>[\s\S]*?<\/style>/gi, '')
        .replace(/<[^>]+>/g, ' ')
        .replace(/\s+/g, ' ')
        .trim();

      // 提取简要释义（第一个句号或分号前的内容）
      const match = text.match(/^(.{0,100}?)[。.；;]/);
      if (match) {
        return match[1].trim();
      }
      return text.substring(0, 80).trim();
    }
  } catch (error) {
    console.error('Get brief error:', error);
  }
  return '';
}

// 网络词典搜索
async function searchOnlineDict(query) {
  try {
    const response = await net.fetch(`${ONLINE_DICT_API}${encodeURIComponent(query)}`);
    if (response.ok) {
      const data = await response.json();
      if (Array.isArray(data) && data.length > 0) {
        return data.slice(0, 3).map(entry => {
          const firstMeaning = entry.meanings?.[0];
          const partOfSpeech = firstMeaning?.partOfSpeech || '';
          const definition = firstMeaning?.definitions?.[0]?.definition || '';
          return {
            word: entry.word,
            brief: partOfSpeech ? `${partOfSpeech}. ${definition.substring(0, 60)}` : definition.substring(0, 80),
            source: 'online'
          };
        });
      }
    }
  } catch (error) {
    console.error('Online dict search error:', error);
  }
  return [];
}

// 查询单词详情（支持本地和网络）
ipcMain.on('lookup-word-detail', async (event, { word, source }) => {
  if (source === 'online') {
    const result = await lookupOnlineWord(word);
    if (lookupWindow && lookupWindow.webContents) {
      lookupWindow.webContents.send('lookup-result', { word, result });
    }
  } else {
    await lookupWord(word);
  }
});

// 网络词典详细查询
async function lookupOnlineWord(word) {
  try {
    const response = await net.fetch(`${ONLINE_DICT_API}${encodeURIComponent(word)}`);
    if (response.ok) {
      const data = await response.json();
      return formatOnlineResult(data, word);
    }
  } catch (error) {
    console.error('Online lookup error:', error);
  }
  return `<div class="error" style="padding: 20px; background: #3a2525; color: #e88; border-radius: 6px;">网络词典查询失败，请检查网络连接</div>`;
}

// 格式化网络词典结果
function formatOnlineResult(data, searchWord) {
  if (!Array.isArray(data) || data.length === 0) {
    return `<div class="not-found" style="padding: 20px; background: #3a3525; color: #da6; border-radius: 6px; text-align: center;">
      未找到单词 "<strong>${searchWord}</strong>" 的释义
    </div>`;
  }

  const entry = data[0];
  const fontFamily = displaySettings.fontFamily || 'Segoe UI';
  const fontSize = displaySettings.fontSize || '14';
  const lineHeight = displaySettings.lineHeight || '1.6';

  let html = `
    <!DOCTYPE html>
    <html>
    <head>
      <meta charset="utf-8">
      <style>
        body {
          font-family: '${fontFamily}', -apple-system, BlinkMacSystemFont, sans-serif;
          padding: 16px;
          margin: 0;
          font-size: ${fontSize}px;
          line-height: ${lineHeight};
          color: #e0e0e0;
          background: #1a1a1a;
        }
        .word-header {
          margin-bottom: 16px;
        }
        .word-title {
          font-size: ${parseInt(fontSize) + 6}px;
          font-weight: bold;
          color: #fff;
          margin-bottom: 8px;
        }
        .phonetic {
          color: #888;
          font-size: ${parseInt(fontSize) - 1}px;
          margin-bottom: 8px;
        }
        .phonetic-item {
          margin-right: 16px;
        }
        .meaning-section {
          margin-bottom: 20px;
        }
        .part-of-speech {
          display: inline-block;
          background: #2a4a3a;
          color: #6c9;
          padding: 3px 10px;
          border-radius: 4px;
          font-size: ${parseInt(fontSize) - 2}px;
          margin-bottom: 10px;
        }
        .definition-list {
          margin: 0;
          padding-left: 20px;
        }
        .definition-item {
          margin-bottom: 12px;
        }
        .definition-text {
          color: #e0e0e0;
        }
        .example {
          color: #888;
          font-style: italic;
          margin-top: 4px;
          padding-left: 12px;
          border-left: 2px solid #444;
        }
        .synonyms {
          margin-top: 8px;
          font-size: ${parseInt(fontSize) - 1}px;
          color: #888;
        }
        .synonyms span {
          color: #6af;
        }
        .source-info {
          margin-top: 24px;
          padding-top: 12px;
          border-top: 1px solid #333;
          font-size: ${parseInt(fontSize) - 2}px;
          color: #666;
        }
      </style>
    </head>
    <body>
      <div class="word-header">
        <div class="word-title">${entry.word}</div>
  `;

  // 音标
  if (entry.phonetics && entry.phonetics.length > 0) {
    html += '<div class="phonetic">';
    entry.phonetics.forEach(p => {
      if (p.text) {
        html += `<span class="phonetic-item">${p.text}</span>`;
      }
    });
    html += '</div>';
  } else if (entry.phonetic) {
    html += `<div class="phonetic">${entry.phonetic}</div>`;
  }

  html += '</div>';

  // 释义
  if (entry.meanings) {
    entry.meanings.forEach(meaning => {
      html += `
        <div class="meaning-section">
          <span class="part-of-speech">${meaning.partOfSpeech}</span>
          <ul class="definition-list">
      `;

      meaning.definitions?.slice(0, 4).forEach(def => {
        html += `
          <li class="definition-item">
            <div class="definition-text">${def.definition}</div>
            ${def.example ? `<div class="example">"${def.example}"</div>` : ''}
          </li>
        `;
      });

      html += '</ul>';

      // 同义词
      if (meaning.synonyms && meaning.synonyms.length > 0) {
        html += `<div class="synonyms">同义词: <span>${meaning.synonyms.slice(0, 5).join(', ')}</span></div>`;
      }

      html += '</div>';
    });
  }

  html += `
      <div class="source-info">来源: Free Dictionary API (网络词典)</div>
    </body>
    </html>
  `;

  return html;
}

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
  const cached = cacheGet(cacheKey);
  if (cached) {
    return cached;
  }

  try {
    const resource = await dictionary.getResource(resourceName);

    // 缓存资源
    if (resource) {
      cacheSet(cacheKey, resource);
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
    // 获取 MIME 类型的辅助函数
    const getMimeType = (name) => {
      if (name.endsWith('.png')) return 'image/png';
      if (name.endsWith('.jpg') || name.endsWith('.jpeg')) return 'image/jpeg';
      if (name.endsWith('.gif')) return 'image/gif';
      if (name.endsWith('.svg')) return 'image/svg+xml';
      if (name.endsWith('.mp3')) return 'audio/mpeg';
      if (name.endsWith('.wav')) return 'audio/wav';
      if (name.endsWith('.ogg')) return 'audio/ogg';
      if (name.endsWith('.css')) return 'text/css';
      if (name.endsWith('.js')) return 'application/javascript';
      return 'application/octet-stream';
    };

    // 从 LRU 缓存获取
    const cached = cacheGet(cacheKey);
    if (cached) {
      callback({
        mimeType: getMimeType(resourceName),
        data: Buffer.from(cached)
      });
    } else {
      // 如果没有缓存，尝试异步加载
      dictionary.getResource(resourceName).then(resource => {
        if (resource) {
          cacheSet(cacheKey, resource);
          callback({
            mimeType: getMimeType(resourceName),
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

  // 剪贴板监听默认禁用，需要在设置中手动开启
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
