const fs = require('fs');
const path = require('path');
const { MDX, MDD } = require('js-mdict');

class MdictParser {
  constructor(mdxFile, mddFile, cssFile, displaySettings = {}) {
    this.mdxFile = mdxFile;
    this.mddFile = mddFile;
    this.cssFile = cssFile;
    this.mdx = null;
    this.mdd = null;
    this.cssContent = '';
    this.displaySettings = displaySettings;
  }

  updateDisplaySettings(settings) {
    this.displaySettings = settings;
  }

  async load() {
    await this.loadCSS();

    try {
      // 加载MDX文件
      console.log('Loading MDX file:', this.mdxFile);
      this.mdx = new MDX(this.mdxFile);

      // 尝试加载MDD文件（资源文件）
      if (this.mddFile && fs.existsSync(this.mddFile)) {
        console.log('Loading MDD file:', this.mddFile);
        this.mdd = new MDD(this.mddFile);
      }

      console.log('Dictionary loaded successfully');
      return true;
    } catch (error) {
      console.error('Failed to load dictionary:', error);
      throw error;
    }
  }

  async loadCSS() {
    try {
      if (fs.existsSync(this.cssFile)) {
        this.cssContent = await fs.promises.readFile(this.cssFile, 'utf-8');
      }
    } catch (error) {
      console.error('Failed to load CSS:', error);
      this.cssContent = '';
    }
  }

  async lookup(word) {
    if (!this.mdx) {
      throw new Error('Dictionary not loaded');
    }

    try {
      // 查找单词
      let result = this.mdx.lookup(word);

      if (!result || !result.definition) {
        return `<div class="not-found">
          <h3>Not Found</h3>
          <p>Word "<strong>${this.escapeHtml(word)}</strong>" not found in dictionary.</p>
          <p style="color: #666; font-size: 12px; margin-top: 10px;">
            Did you mean: ${this.getSuggestions(word)}
          </p>
        </div>`;
      }

      // 获取定义内容
      let definition = result.definition;
      let displayWord = word;

      // 处理 @@@LINK= 重定向
      // 匹配到行尾或下一个标签，支持包含空格的目标词
      const linkMatch = definition.match(/@@@LINK=\s*(.+?)(?:\s*<|$)/i);
      if (linkMatch) {
        const targetWord = linkMatch[1].trim();
        console.log(`Redirecting: ${word} -> ${targetWord}`);

        // 查找目标词
        const targetResult = this.mdx.lookup(targetWord);
        if (targetResult && targetResult.definition) {
          definition = targetResult.definition;
          displayWord = targetWord;
        } else {
          // 如果目标词也没找到，显示重定向信息
          return `<div class="not-found">
            <h3>Redirect Failed</h3>
            <p>Word "<strong>${this.escapeHtml(word)}</strong>" redirects to "<strong>${this.escapeHtml(targetWord)}</strong>", but the target word was not found.</p>
          </div>`;
        }
      }

      const htmlContent = this.processDefinition(definition, word);

      // 构建内容片段（深色主题，不包含 html/body 标签）
      const fontFamily = this.displaySettings.fontFamily || 'Segoe UI';
      const fontSize = this.displaySettings.fontSize || '14';
      const lineHeight = this.displaySettings.lineHeight || '1.6';

      return `
        <style>
          .dict-content {
            font-family: '${fontFamily}', -apple-system, BlinkMacSystemFont, 'PingFang SC', 'Microsoft YaHei', sans-serif;
            font-size: ${fontSize}px;
            line-height: ${lineHeight};
            color: #e0e0e0;
          }

          .dict-content h2 {
            color: #4fc3f7;
            border-bottom: 2px solid #4fc3f7;
            padding-bottom: 5px;
          }

          .dict-content .word-title {
            font-size: ${parseInt(fontSize) + 6}px;
            font-weight: bold;
            color: #fff;
            margin-bottom: 10px;
          }

          .dict-content .redirect-info {
            font-size: ${parseInt(fontSize) - 2}px;
            color: #888;
            margin-bottom: 10px;
            font-style: italic;
          }

          ${this.cssContent}

          /* 深色主题覆盖样式 */
          .dict-content, .dict-content div, .dict-content span, .dict-content p, .dict-content td, .dict-content th {
            color: #e0e0e0 !important;
          }

          .dict-content img {
            max-width: 100%;
            height: auto;
          }

          .dict-content table {
              border-collapse: collapse;
              max-width: 100%;
              font-size: ${parseInt(fontSize) - 1}px;
            }

          .dict-content a {
              color: #6af !important;
              text-decoration: none;
            }

          .dict-content a:hover {
              text-decoration: underline;
            }

            /* 词典特定样式覆盖 */
          .dict-content .pos, .dict-content .gram {
              color: #6c9 !important;
            }

          .dict-content .phon {
              color: #888 !important;
            }

          .dict-content .def {
              color: #e0e0e0 !important;
            }

          .dict-content .x, .dict-content .example {
              color: #aaa !important;
              font-style: italic;
            }
        </style>
        <div class="dict-content">
          <div class="word-title">${this.escapeHtml(displayWord)}</div>
          ${displayWord !== word ? `<div class="redirect-info">(redirected from "${this.escapeHtml(word)}")</div>` : ''}
          ${htmlContent}
        </div>
      `;
    } catch (error) {
      console.error('Lookup error:', error);
      return `<div class="error">
        <h3>Error</h3>
        <p>Failed to lookup word: ${this.escapeHtml(word)}</p>
        <p style="color: #666; font-size: 12px;">${error.message}</p>
      </div>`;
    }
  }

  processDefinition(definition, word) {
    // 处理定义内容
    let html = definition;

    // 处理相对路径的资源链接
    html = html.replace(
      /<img[^>]+src=["']([^"']+)["'][^>]*>/gi,
      (match, src) => {
        if (!src.startsWith('http') && !src.startsWith('data:') && !src.startsWith('mdd-resource://')) {
          const resourceName = path.basename(src).replace(/\\/g, '/');
          // 标记需要从MDD加载
          return match.replace(src, `mdd-resource://${resourceName}`);
        }
        return match;
      }
    );

    // 处理音频链接 - 保留原链接但添加class标记
    html = html.replace(
      /<a([^>]+)href=["']([^"']*\.(mp3|wav|ogg))["']([^>]*)>/gi,
      (match, before, href, ext, after) => {
        if (!href.startsWith('http') && !href.startsWith('mdd-resource://')) {
          const resourceName = path.basename(href).replace(/\\/g, '/');
          return `<a${before}href="mdd-resource://${resourceName}"${after} data-audio="true">`;
        }
        return match;
      }
    );

    return html;
  }

  async getResource(resourceName) {
    if (!this.mdd) {
      return null;
    }

    try {
      // MDD类使用locate()方法
      const result = this.mdd.locate(resourceName);
      if (result && result.definition) {
        return result.definition;
      }
    } catch (error) {
      console.error('Failed to load resource:', resourceName, error);
    }

    return null;
  }

  escapeHtml(text) {
    const map = {
      '&': '&amp;',
      '<': '&lt;',
      '>': '&gt;',
      '"': '&quot;',
      "'": '&#039;'
    };
    return text.replace(/[&<>"']/g, m => map[m]);
  }

  getSuggestions(word) {
    // 简单的建议词生成（可以改进）
    if (!this.mdx) {
      return 'try checking your spelling';
    }

    try {
      // 获取建议词
      const suggestions = this.mdx.suggest(word, 2);
      if (suggestions && suggestions.length > 0) {
        return suggestions.slice(0, 5).map(s => s.keyText).join(', ');
      }
      return 'try checking your spelling';
    } catch (error) {
      return 'try checking your spelling';
    }
  }
}

module.exports = MdictParser;
