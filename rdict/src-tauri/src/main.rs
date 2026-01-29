// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::{Arc, Mutex};
use tauri::{
    Manager, State, 
    tray::{TrayIconBuilder, TrayIconEvent, MouseButton},
    menu::{Menu, MenuItem},
    webview::WebviewWindow,
};
use serde::{Deserialize, Serialize};
use anyhow::Result;
use tauri_plugin_global_shortcut::{Shortcut, Code, Modifiers};

mod mdict;
mod config;

use mdict::{MdxDictionary, MddResource, DictionaryEntry};
use config::{AppConfig, DisplaySettings};

// Application state
struct AppState {
    config: Mutex<AppConfig>,
    dictionary: Mutex<Option<MdxDictionary>>,
    mdd: Mutex<Option<MddResource>>,
    css_content: Mutex<String>,
    last_clipboard: Mutex<String>,
}

// Data structures for API
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SearchResult {
    word: String,
    brief: String,
    source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LookupResult {
    word: String,
    result: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OnlineDefinition {
    definition: String,
    example: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OnlineMeaning {
    part_of_speech: String,
    definitions: Vec<OnlineDefinition>,
    synonyms: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OnlineEntry {
    word: String,
    phonetic: Option<String>,
    phonetics: Vec<serde_json::Value>,
    meanings: Vec<OnlineMeaning>,
}

// Initialize dictionary
fn init_dictionary(state: &AppState) -> Result<()> {
    let config = state.config.lock().unwrap();
    
    if let Some(ref mdx_path) = config.mdx_file {
        let dict = MdxDictionary::new(mdx_path)?;
        *state.dictionary.lock().unwrap() = Some(dict);
        
        // Load MDD if available
        if let Some(ref mdd_path) = config.mdd_file {
            if let Ok(mdd) = MddResource::new(mdd_path) {
                *state.mdd.lock().unwrap() = Some(mdd);
            }
        }
        
        // Load CSS if available
        if let Some(ref css_path) = config.css_file {
            if let Ok(content) = std::fs::read_to_string(css_path) {
                *state.css_content.lock().unwrap() = content;
            }
        }
    }
    
    Ok(())
}

// Tauri commands
#[tauri::command]
fn get_config(state: State<AppState>) -> AppConfig {
    state.config.lock().unwrap().clone()
}

#[tauri::command]
fn set_dictionary_path(path: String, state: State<AppState>) -> Result<(), String> {
    let mut config = state.config.lock().map_err(|e| e.to_string())?;
    config.update_dictionary_path(std::path::PathBuf::from(&path));
    config.save().map_err(|e| e.to_string())?;
    
    drop(config);
    init_dictionary(&state).map_err(|e| e.to_string())?;
    
    Ok(())
}

#[tauri::command]
async fn set_hotkey(
    hotkey: String, 
    state: State<'_, AppState>, 
    app_handle: tauri::AppHandle
) -> Result<bool, String> {
    let mut config = state.config.lock().map_err(|e| e.to_string())?;
    config.hotkey = hotkey.clone();
    config.save().map_err(|e| e.to_string())?;
    drop(config);
    
    // Register new hotkey using global shortcut plugin
    match register_global_hotkey(&app_handle, &hotkey).await {
        Ok(_) => Ok(true),
        Err(e) => {
            eprintln!("Failed to register hotkey: {}", e);
            Ok(false)
        }
    }
}

#[tauri::command]
fn toggle_clipboard_monitor(enabled: bool, state: State<AppState>) -> Result<(), String> {
    let mut config = state.config.lock().map_err(|e| e.to_string())?;
    config.clipboard_monitor = enabled;
    config.save().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn set_display_settings(
    font_family: Option<String>,
    font_size: Option<String>,
    line_height: Option<String>,
    state: State<AppState>
) -> Result<(), String> {
    let mut config = state.config.lock().map_err(|e| e.to_string())?;
    
    if let Some(ff) = font_family {
        config.display.font_family = ff;
    }
    if let Some(fs) = font_size {
        config.display.font_size = fs;
    }
    if let Some(lh) = line_height {
        config.display.line_height = lh;
    }
    
    config.save().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn search_words(query: String, state: State<AppState>) -> Vec<SearchResult> {
    let mut results = Vec::new();
    let query_lower = query.to_lowercase();
    
    // Ensure dictionary is loaded
    if state.dictionary.lock().unwrap().is_none() {
        let _ = init_dictionary(&state);
    }
    
    // Local dictionary search
    if let Some(ref dict) = *state.dictionary.lock().unwrap() {
        let suggestions = dict.prefix_search(&query_lower);
        for word in suggestions.into_iter().take(10) {
            let brief = get_word_brief(dict, &word);
            results.push(SearchResult {
                word,
                brief,
                source: "local".to_string(),
            });
        }
    }
    
    // Online search if local results are insufficient
    if results.len() < 3 && query.chars().all(|c| c.is_ascii_alphabetic() || c == '-' || c == '\'') {
        if let Ok(online_results) = search_online_dict(&query_lower) {
            for item in online_results {
                if !results.iter().any(|r| r.word.to_lowercase() == item.word.to_lowercase()) {
                    results.push(item);
                }
            }
        }
    }
    
    results.into_iter().take(10).collect()
}

#[tauri::command]
fn lookup_word(word: String, state: State<AppState>) -> LookupResult {
    // Ensure dictionary is loaded
    if state.dictionary.lock().unwrap().is_none() {
        let _ = init_dictionary(&state);
    }
    
    if let Some(ref dict) = *state.dictionary.lock().unwrap() {
        if let Some(entry) = dict.lookup(&word) {
            let html = format_definition(&entry, &word, &state);
            return LookupResult {
                word: entry.word.clone(),
                result: html,
            };
        }
    }
    
    LookupResult {
        word: word.clone(),
        result: format_not_found(&word),
    }
}

#[tauri::command]
async fn lookup_word_online(word: String) -> LookupResult {
    match lookup_online_word(&word).await {
        Ok(html) => LookupResult {
            word: word.clone(),
            result: html,
        },
        Err(_) => LookupResult {
            word: word.clone(),
            result: format_online_error(),
        },
    }
}

#[tauri::command]
fn get_mdd_resource(resource_name: String, state: State<AppState>) -> Option<Vec<u8>> {
    if let Some(ref mdd) = *state.mdd.lock().unwrap() {
        mdd.locate(&resource_name)
    } else {
        None
    }
}

// Helper functions
fn get_word_brief(dict: &MdxDictionary, word: &str) -> String {
    if let Some(entry) = dict.lookup(word) {
        let text = html_escape::decode_html_entities(&entry.definition);
        let text = text
            .replace(|c: char| c.is_ascii_control(), " ")
            .split(|c: char| c == '。' || c == '.' || c == ';' || c == '；')
            .next()
            .unwrap_or("")
            .trim();
        
        if text.len() > 100 {
            format!("{}...", &text[..100])
        } else {
            text.to_string()
        }
    } else {
        String::new()
    }
}

fn format_definition(entry: &DictionaryEntry, original_word: &str, state: &AppState) -> String {
    let config = state.config.lock().unwrap();
    let css = state.css_content.lock().unwrap();
    
    let font_family = &config.display.font_family;
    let font_size = &config.display.font_size;
    let line_height = &config.display.line_height;
    
    let display_word = &entry.word;
    let mut definition = entry.definition.clone();
    
    // Handle @@@LINK= redirects
    if definition.contains("@@@LINK=") {
        let re = regex::Regex::new(r"@@@LINK=\s*(.+?)(?:\s*<|$)").unwrap();
        if let Some(cap) = re.captures(&definition) {
            let _target = cap[1].trim();
            // Try to resolve redirect
        }
    }
    
    // Process resource links
    definition = process_resource_links(&definition);
    
    let redirect_info = if display_word != original_word {
        format!(r#"<div class="redirect-info">(redirected from "{}")</div>"#, html_escape::encode_text(original_word))
    } else {
        String::new()
    };
    
    format!(r#"
        <style>
            .dict-content {{
                font-family: '{}', -apple-system, BlinkMacSystemFont, 'PingFang SC', 'Microsoft YaHei', sans-serif;
                font-size: {}px;
                line-height: {};
                color: #e0e0e0;
            }}
            .dict-content .word-title {{
                font-size: {}px;
                font-weight: bold;
                color: #fff;
                margin-bottom: 10px;
            }}
            .dict-content .redirect-info {{
                font-size: {}px;
                color: #888;
                margin-bottom: 10px;
                font-style: italic;
            }}
            {}
            .dict-content, .dict-content div, .dict-content span, .dict-content p, 
            .dict-content td, .dict-content th {{
                color: #e0e0e0 !important;
            }}
            .dict-content img {{
                max-width: 100%;
                height: auto;
            }}
            .dict-content table {{
                border-collapse: collapse;
                max-width: 100%;
                font-size: {}px;
            }}
            .dict-content a {{
                color: #6af !important;
                text-decoration: none;
            }}
            .dict-content a:hover {{
                text-decoration: underline;
            }}
            .dict-content .pos, .dict-content .gram {{
                color: #6c9 !important;
            }}
            .dict-content .phon {{
                color: #888 !important;
            }}
            .dict-content .def {{
                color: #e0e0e0 !important;
            }}
            .dict-content .x, .dict-content .example {{
                color: #aaa !important;
                font-style: italic;
            }}
        </style>
        <div class="dict-content">
            <div class="word-title">{}</div>
            {}
            {}
        </div>
    "#, 
        font_family, font_size, line_height,
        font_size.parse::<i32>().unwrap_or(14) + 6,
        font_size.parse::<i32>().unwrap_or(14) - 2,
        css,
        font_size.parse::<i32>().unwrap_or(14) - 1,
        html_escape::encode_text(display_word),
        redirect_info,
        definition
    )
}

fn process_resource_links(html: &str) -> String {
    let mut result = html.to_string();
    
    // Process image links
    let img_re = regex::Regex::new(r#"<img[^>]+src=["']([^"']+)["'][^>]*>"#).unwrap();
    result = img_re.replace_all(&result, |caps: &regex::Captures| {
        let src = &caps[1];
        if !src.starts_with("http") && !src.starts_with("data:") && !src.starts_with("mdd-resource://") {
            let resource_name = std::path::Path::new(src)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(src);
            caps[0].replace(src, &format!("mdd-resource://{}", resource_name))
        } else {
            caps[0].to_string()
        }
    }).to_string();
    
    // Process audio links
    let audio_re = regex::Regex::new(r#"<a([^>]+)href=["']([^"']*\.(mp3|wav|ogg))["']([^>]*)>"#).unwrap();
    result = audio_re.replace_all(&result, |caps: &regex::Captures| {
        let href = &caps[2];
        if !href.starts_with("http") && !href.starts_with("mdd-resource://") {
            let resource_name = std::path::Path::new(href)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(href);
            format!(r#"<a{}href="mdd-resource://{}"{} data-audio="true">"#, &caps[1], resource_name, &caps[4])
        } else {
            caps[0].to_string()
        }
    }).to_string();
    
    result
}

fn format_not_found(word: &str) -> String {
    format!(r#"
        <div class="not-found" style="padding: 20px; background: #3a3525; color: #da6; border-radius: 6px; text-align: center;">
            <h3>Not Found</h3>
            <p>Word "<strong>{}</strong>" not found in dictionary.</p>
            <p style="color: #666; font-size: 12px; margin-top: 10px;">
                Please check your spelling
            </p>
        </div>
    "#, html_escape::encode_text(word))
}

fn format_online_error() -> String {
    r#"
        <div class="error" style="padding: 20px; background: #3a2525; color: #e88; border-radius: 6px;">
            网络词典查询失败，请检查网络连接
        </div>
    "#.to_string()
}

async fn lookup_online_word(word: &str) -> Result<String> {
    let client = reqwest::Client::new();
    let url = format!("https://api.dictionaryapi.dev/api/v2/entries/en/{}", 
        urlencoding::encode(word));
    
    let response = client.get(&url).send().await?;
    
    if response.status().is_success() {
        let data: Vec<OnlineEntry> = response.json().await?;
        Ok(format_online_result(&data, word))
    } else {
        Err(anyhow::anyhow!("API request failed"))
    }
}

fn format_online_result(data: &[OnlineEntry], search_word: &str) -> String {
    if data.is_empty() {
        return format_not_found(search_word);
    }
    
    let entry = &data[0];
    
    let mut html = format!(r#"
        <!DOCTYPE html>
        <html>
        <head>
            <meta charset="utf-8">
            <style>
                body {{
                    font-family: 'Segoe UI', -apple-system, BlinkMacSystemFont, sans-serif;
                    padding: 16px;
                    margin: 0;
                    font-size: 14px;
                    line-height: 1.6;
                    color: #e0e0e0;
                    background: #1a1a1a;
                }}
                .word-header {{ margin-bottom: 16px; }}
                .word-title {{
                    font-size: 20px;
                    font-weight: bold;
                    color: #fff;
                    margin-bottom: 8px;
                }}
                .phonetic {{ color: #888; font-size: 13px; margin-bottom: 8px; }}
                .phonetic-item {{ margin-right: 16px; }}
                .meaning-section {{ margin-bottom: 20px; }}
                .part-of-speech {{
                    display: inline-block;
                    background: #2a4a3a;
                    color: #6c9;
                    padding: 3px 10px;
                    border-radius: 4px;
                    font-size: 12px;
                    margin-bottom: 10px;
                }}
                .definition-list {{ margin: 0; padding-left: 20px; }}
                .definition-item {{ margin-bottom: 12px; }}
                .definition-text {{ color: #e0e0e0; }}
                .example {{
                    color: #888;
                    font-style: italic;
                    margin-top: 4px;
                    padding-left: 12px;
                    border-left: 2px solid #444;
                }}
                .synonyms {{
                    margin-top: 8px;
                    font-size: 13px;
                    color: #888;
                }}
                .synonyms span {{ color: #6af; }}
                .source-info {{
                    margin-top: 24px;
                    padding-top: 12px;
                    border-top: 1px solid #333;
                    font-size: 12px;
                    color: #666;
                }}
            </style>
        </head>
        <body>
            <div class="word-header">
                <div class="word-title">{}</div>
    "#, entry.word);
    
    // Phonetics
    if !entry.phonetics.is_empty() {
        html.push_str(r#"<div class="phonetic">"#);
        for p in &entry.phonetics {
            if let Some(text) = p.get("text").and_then(|t| t.as_str()) {
                html.push_str(&format!(r#"<span class="phonetic-item">{}</span>"#, text));
            }
        }
        html.push_str("</div>");
    } else if let Some(ref phonetic) = entry.phonetic {
        html.push_str(&format!(r#"<div class="phonetic">{}</div>"#, phonetic));
    }
    
    html.push_str("</div>");
    
    // Meanings
    for meaning in &entry.meanings {
        html.push_str(&format!(r#"
            <div class="meaning-section">
                <span class="part-of-speech">{}</span>
                <ul class="definition-list">
        "#, meaning.part_of_speech));
        
        for def in meaning.definitions.iter().take(4) {
            html.push_str(&format!(r#"
                <li class="definition-item">
                    <div class="definition-text">{}</div>
                    {}
                </li>
            "#, 
                def.definition,
                def.example.as_ref()
                    .map(|e| format!(r#"<div class="example">"{}"</div>"#, e))
                    .unwrap_or_default()
            ));
        }
        
        html.push_str("</ul>");
        
        // Synonyms
        if !meaning.synonyms.is_empty() {
            html.push_str(&format!(r#"
                <div class="synonyms">同义词: <span>{}</span></div>
            "#, meaning.synonyms[..meaning.synonyms.len().min(5)].join(", ")));
        }
        
        html.push_str("</div>");
    }
    
    html.push_str(r#"
            <div class="source-info">来源: Free Dictionary API (网络词典)</div>
        </body>
        </html>
    "#);
    
    html
}

fn search_online_dict(query: &str) -> Result<Vec<SearchResult>> {
    // Synchronous version for local search fallback
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async_search_online(query))
}

async fn async_search_online(query: &str) -> Result<Vec<SearchResult>> {
    let client = reqwest::Client::new();
    let url = format!("https://api.dictionaryapi.dev/api/v2/entries/en/{}", 
        urlencoding::encode(query));
    
    let response = client.get(&url).send().await?;
    
    if response.status().is_success() {
        let data: Vec<OnlineEntry> = response.json().await?;
        let results: Vec<SearchResult> = data.into_iter().take(3).map(|entry| {
            let first_meaning = entry.meanings.first();
            let part_of_speech = first_meaning.map(|m| m.part_of_speech.clone()).unwrap_or_default();
            let definition = first_meaning
                .and_then(|m| m.definitions.first())
                .map(|d| d.definition.clone())
                .unwrap_or_default();
            
            let brief = if !part_of_speech.is_empty() {
                format!("{}. {}", part_of_speech, &definition[..definition.len().min(60)])
            } else {
                definition[..definition.len().min(80)].to_string()
            };
            
            SearchResult {
                word: entry.word,
                brief,
                source: "online".to_string(),
            }
        }).collect();
        
        Ok(results)
    } else {
        Ok(vec![])
    }
}

fn parse_hotkey(hotkey: &str) -> Option<Shortcut> {
    let parts: Vec<&str> = hotkey.split('+').map(|p| p.trim()).collect();
    let mut modifiers = Modifiers::empty();
    let mut key: Option<Code> = None;
    
    for part in parts {
        let lower = part.to_lowercase();
        match lower.as_str() {
            "ctrl" | "control" => modifiers |= Modifiers::CONTROL,
            "cmd" | "command" | "meta" => modifiers |= Modifiers::META,
            "alt" => modifiers |= Modifiers::ALT,
            "shift" => modifiers |= Modifiers::SHIFT,
            "option" => modifiers |= Modifiers::ALT,
            _ => {
                // Parse key
                key = parse_key_code(part);
            }
        }
    }
    
    key.map(|k| Shortcut::new(Some(modifiers), k))
}

fn parse_key_code(key: &str) -> Option<Code> {
    match key.to_uppercase().as_str() {
        "A" => Some(Code::KeyA),
        "B" => Some(Code::KeyB),
        "C" => Some(Code::KeyC),
        "D" => Some(Code::KeyD),
        "E" => Some(Code::KeyE),
        "F" => Some(Code::KeyF),
        "G" => Some(Code::KeyG),
        "H" => Some(Code::KeyH),
        "I" => Some(Code::KeyI),
        "J" => Some(Code::KeyJ),
        "K" => Some(Code::KeyK),
        "L" => Some(Code::KeyL),
        "M" => Some(Code::KeyM),
        "N" => Some(Code::KeyN),
        "O" => Some(Code::KeyO),
        "P" => Some(Code::KeyP),
        "Q" => Some(Code::KeyQ),
        "R" => Some(Code::KeyR),
        "S" => Some(Code::KeyS),
        "T" => Some(Code::KeyT),
        "U" => Some(Code::KeyU),
        "V" => Some(Code::KeyV),
        "W" => Some(Code::KeyW),
        "X" => Some(Code::KeyX),
        "Y" => Some(Code::KeyY),
        "Z" => Some(Code::KeyZ),
        "0" => Some(Code::Digit0),
        "1" => Some(Code::Digit1),
        "2" => Some(Code::Digit2),
        "3" => Some(Code::Digit3),
        "4" => Some(Code::Digit4),
        "5" => Some(Code::Digit5),
        "6" => Some(Code::Digit6),
        "7" => Some(Code::Digit7),
        "8" => Some(Code::Digit8),
        "9" => Some(Code::Digit9),
        "F1" => Some(Code::F1),
        "F2" => Some(Code::F2),
        "F3" => Some(Code::F3),
        "F4" => Some(Code::F4),
        "F5" => Some(Code::F5),
        "F6" => Some(Code::F6),
        "F7" => Some(Code::F7),
        "F8" => Some(Code::F8),
        "F9" => Some(Code::F9),
        "F10" => Some(Code::F10),
        "F11" => Some(Code::F11),
        "F12" => Some(Code::F12),
        "SPACE" => Some(Code::Space),
        "ESCAPE" => Some(Code::Escape),
        "ENTER" => Some(Code::Enter),
        "TAB" => Some(Code::Tab),
        _ => None,
    }
}

async fn register_global_hotkey(app: &tauri::AppHandle, hotkey: &str) -> Result<()> {
    let shortcut = parse_hotkey(hotkey)
        .ok_or_else(|| anyhow::anyhow!("Invalid hotkey format"))?;
    
    let app_clone = app.clone();
    
    // Register with global shortcut plugin
    app.global_shortcut().on_shortcut(shortcut, move |_app, _shortcut, _event| {
        let app_handle = _app.clone();
        
        // Toggle lookup window
        if let Some(window) = app_handle.get_webview_window("lookup") {
            if window.is_visible().unwrap_or(false) {
                let _ = window.hide();
            } else {
                let _ = window.show();
                let _ = window.set_focus();
            }
        } else {
            // Create lookup window
            if let Ok(window) = create_lookup_window(&app_handle) {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
    })?;
    
    Ok(())
}

fn create_lookup_window(app: &tauri::AppHandle) -> Result<WebviewWindow> {
    let window = tauri::WebviewWindowBuilder::new(
        app,
        "lookup",
        tauri::WebviewUrl::App("lookup.html".into())
    )
    .title("RDict")
    .inner_size(600.0, 52.0)
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(false)
    .transparent(false)
    .visible(false)
    .build()?;
    
    Ok(window)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    // Load configuration
    let config = AppConfig::load().unwrap_or_default();
    
    // Initialize state
    let app_state = Arc::new(AppState {
        config: Mutex::new(config.clone()),
        dictionary: Mutex::new(None),
        mdd: Mutex::new(None),
        css_content: Mutex::new(String::new()),
        last_clipboard: Mutex::new(String::new()),
    });
    
    // Initialize dictionary
    let _ = init_dictionary(&app_state);
    
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(AppState {
            config: Mutex::new(config.clone()),
            dictionary: Mutex::new(None),
            mdd: Mutex::new(None),
            css_content: Mutex::new(String::new()),
            last_clipboard: Mutex::new(String::new()),
        })
        .setup(move |app| {
            // Create tray icon
            let show_i = MenuItem::with_id(app, "show", "显示", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &quit_i])?;
            
            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .tooltip("RDict")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => {
                        app.exit(0);
                    }
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button, .. } = event {
                        if button == MouseButton::Left {
                            let app = tray.app_handle();
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;
            
            // Show main window
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
            
            // Register global hotkey
            let hotkey = config.hotkey.clone();
            let app_handle = app.app_handle().clone();
            tauri::async_runtime::spawn(async move {
                let _ = register_global_hotkey(&app_handle, &hotkey).await;
            });
            
            // Start clipboard monitor if enabled
            if config.clipboard_monitor {
                start_clipboard_monitor(app.app_handle().clone());
            }
            
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            set_dictionary_path,
            set_hotkey,
            toggle_clipboard_monitor,
            set_display_settings,
            search_words,
            lookup_word,
            lookup_word_online,
            get_mdd_resource
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn start_clipboard_monitor(app_handle: tauri::AppHandle) {
    std::thread::spawn(move || {
        let mut last_text = String::new();
        
        loop {
            std::thread::sleep(std::time::Duration::from_millis(500));
            
            // Read clipboard using clipboard manager plugin via command
            // Note: In Tauri 2.x, clipboard access from main thread is different
            // This is a simplified version - full implementation would need proper clipboard monitoring
        }
    });
}

fn main() {
    run();
}
