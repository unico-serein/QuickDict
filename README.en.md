# QuickDict - MDX/MDD Dictionary Lookup Tool

English | [简体中文](README.md)

An Electron-based dictionary lookup tool that supports MDX/MDD format dictionary files.

## Features

- Support for MDX/MDD format dictionaries
- Load dictionary CSS styles
- Global hotkey lookup (Alt+M)
- Automatic clipboard monitoring
- Popup window for query results
- Support for images, audio, and other multimedia resources

## Usage

### Install Dependencies

```bash
npm install
```

### Start Application

```bash
npm start
```

### Look Up Words

1. **Global Hotkey**: Press `Alt+M` to lookup clipboard content
2. **Copy to Lookup**: Copy English words to clipboard, app will automatically lookup and show result popup
3. **Manual Lookup**: Type a word in the main window and press Enter

## Configuration

Dictionary paths are configured in `src/main.js`:

```javascript
const DICTIONARY_PATH = 'D:\\Documents\\Dictionary\\Oxford';
const MDX_FILE = path.join(DICTIONARY_PATH, 'oxford.mdx');
const MDD_FILE = path.join(DICTIONARY_PATH, 'oxford.mdd');
const CSS_FILE = path.join(DICTIONARY_PATH, 'style.css');
```

To use a different dictionary, modify the paths above.

## Tech Stack

- **Electron** - Desktop application framework
- **js-mdict** - MDX/MDD file parsing library
- **Node.js** - Backend runtime

## Project Structure

```
quickdict/
├── src/
│   ├── main.js           # Main process code
│   ├── mdict-parser.js   # MDX/MDD parser
│   ├── index.html        # Main window (config UI)
│   ├── settings.html     # Settings window
│   └── lookup.html       # Lookup result window
├── package.json
├── README.md             # Chinese documentation
├── README.en.md          # English documentation
└── .gitignore
```

## Notes

- First query requires loading dictionary files, may take a few seconds
- Large dictionary files will have initial loading delay
- SSD storage recommended for better performance

## License

MIT
