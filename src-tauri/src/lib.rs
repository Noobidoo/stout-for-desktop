use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};
use tauri::{
    menu::MenuBuilder,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, WindowEvent,
};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt as AutostartExt};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

const DEFAULT_REMOTE_URL: &str = "https://stoat.chat/app";
const INIT_SCRIPT: &str = r#"
(() => {
  const tauri = window.__TAURI__;
  if (!tauri || !tauri.core || !tauri.core.invoke) return;

  let desktopConfig = {
    firstLaunch: true,
    customFrame: true,
    minimiseToTray: true,
    startMinimisedToTray: false,
    spellchecker: true,
    hardwareAcceleration: true,
    discordRpc: true,
    windowState: { x: -1, y: -1, width: -1, height: -1, isMaximised: false }
  };

  tauri.core.invoke("get_config").then((cfg) => {
    desktopConfig = cfg;
  }).catch(() => {});

  tauri.event.listen("config", (event) => {
    desktopConfig = event.payload;
  });

  window.native = {
    versions: {
      node: () => "tauri",
      chrome: () => "webview",
      electron: () => "tauri",
      desktop: () => "1.3.0"
    },
    minimise: () => tauri.core.invoke("minimise_window"),
    maximise: () => tauri.core.invoke("maximise_window"),
    close: () => tauri.core.invoke("close_window"),
    setBadgeCount: (count) => tauri.core.invoke("set_badge_count", { count }),
    ptt: {
      onPress: (cb) => tauri.event.listen("ptt-press", cb),
      onRelease: (cb) => tauri.event.listen("ptt-release", cb),
    }
  };

  window.desktopConfig = {
    get: () => desktopConfig,
    set: (config) => tauri.core.invoke("set_config", { newConfig: config }),
    getAutostart: () => tauri.core.invoke("get_autostart"),
    setAutostart: (value) => tauri.core.invoke("set_autostart", { state: value }),
    registerPttKey: (key) => tauri.core.invoke("register_ptt_key", { key }),
    unregisterPttKey: () => tauri.core.invoke("unregister_ptt_key"),
  };
})();
"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WindowState {
    x: i32,
    y: i32,
    width: f64,
    height: f64,
    is_maximised: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DesktopConfig {
    first_launch: bool,
    custom_frame: bool,
    minimise_to_tray: bool,
    start_minimised_to_tray: bool,
    spellchecker: bool,
    hardware_acceleration: bool,
    discord_rpc: bool,
    #[serde(default)]
    ptt_key: Option<String>,
    window_state: WindowState,
}

impl Default for DesktopConfig {
    fn default() -> Self {
        Self {
            first_launch: true,
            custom_frame: true,
            minimise_to_tray: true,
            start_minimised_to_tray: false,
            spellchecker: true,
            hardware_acceleration: true,
            discord_rpc: true,
            ptt_key: None,
            window_state: WindowState {
                x: -1,
                y: -1,
                width: -1.0,
                height: -1.0,
                is_maximised: false,
            },
        }
    }
}

#[derive(Clone)]
struct AppState {
    config: Arc<Mutex<DesktopConfig>>,
    config_path: PathBuf,
    quitting: Arc<Mutex<bool>>,
    ptt_shortcut: Arc<Mutex<Option<String>>>,
}

impl AppState {
    fn load(path: PathBuf) -> Self {
        let config = fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str::<DesktopConfig>(&raw).ok())
            .unwrap_or_default();

        Self {
            config: Arc::new(Mutex::new(config)),
            config_path: path,
            quitting: Arc::new(Mutex::new(false)),
            ptt_shortcut: Arc::new(Mutex::new(None)),
        }
    }

    fn save(&self) -> Result<(), String> {
        let config = self.config.lock().map_err(|e| e.to_string())?.clone();
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let raw = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
        fs::write(&self.config_path, raw).map_err(|e| e.to_string())
    }
}

#[tauri::command]
fn get_config(state: tauri::State<'_, AppState>) -> Result<DesktopConfig, String> {
    state
        .config
        .lock()
        .map(|c| c.clone())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn set_config(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    new_config: DesktopConfig,
) -> Result<DesktopConfig, String> {
    {
        let mut lock = state.config.lock().map_err(|e| e.to_string())?;
        *lock = new_config.clone();
    }
    state.save()?;
    let _ = app.emit("config", &new_config);
    Ok(new_config)
}

#[tauri::command]
fn minimise_window(window: tauri::Window) -> Result<(), String> {
    window.minimize().map_err(|e| e.to_string())
}

#[tauri::command]
fn maximise_window(window: tauri::Window) -> Result<(), String> {
    if window.is_maximized().map_err(|e| e.to_string())? {
        window.unmaximize().map_err(|e| e.to_string())
    } else {
        window.maximize().map_err(|e| e.to_string())
    }
}

#[tauri::command]
fn close_window(window: tauri::Window) -> Result<(), String> {
    window.close().map_err(|e| e.to_string())
}

#[tauri::command]
fn set_badge_count(_count: i64) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
fn get_autostart(app: tauri::AppHandle) -> Result<bool, String> {
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

#[tauri::command]
fn set_autostart(app: tauri::AppHandle, state: bool) -> Result<bool, String> {
    if state {
        app.autolaunch().enable().map_err(|e| e.to_string())?;
    } else {
        app.autolaunch().disable().map_err(|e| e.to_string())?;
    }
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

#[tauri::command]
fn register_ptt_key(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    key: String,
) -> Result<(), String> {
    {
        let mut current = state.ptt_shortcut.lock().map_err(|e| e.to_string())?;
        if let Some(ref existing) = *current {
            let _ = app.global_shortcut().unregister(existing.as_str());
        }
        *current = None;
    }
    let app_clone = app.clone();
    app.global_shortcut()
        .on_shortcut(key.as_str(), move |_app, _shortcut, event| {
            let name = match event.state {
                ShortcutState::Pressed => "ptt-press",
                ShortcutState::Released => "ptt-release",
            };
            let _ = app_clone.emit(name, ());
        })
        .map_err(|e| e.to_string())?;
    {
        let mut current = state.ptt_shortcut.lock().map_err(|e| e.to_string())?;
        *current = Some(key.clone());
    }
    {
        let mut config = state.config.lock().map_err(|e| e.to_string())?;
        config.ptt_key = Some(key);
    }
    state.save()
}

#[tauri::command]
fn unregister_ptt_key(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut current = state.ptt_shortcut.lock().map_err(|e| e.to_string())?;
    if let Some(ref key) = *current {
        app.global_shortcut()
            .unregister(key.as_str())
            .map_err(|e| e.to_string())?;
    }
    *current = None;
    drop(current);
    {
        let mut config = state.config.lock().map_err(|e| e.to_string())?;
        config.ptt_key = None;
    }
    state.save()
}

fn parse_force_server_arg() -> Option<String> {
    let mut iter = std::env::args();
    while let Some(arg) = iter.next() {
        if let Some(value) = arg.strip_prefix("--force-server=") {
            return Some(value.to_string());
        }
        if arg == "--force-server" {
            return iter.next();
        }
    }
    None
}

fn has_hidden_arg() -> bool {
    std::env::args().any(|arg| arg == "--hidden")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec![]),
        ))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            get_config,
            set_config,
            minimise_window,
            maximise_window,
            close_window,
            set_badge_count,
            get_autostart,
            set_autostart,
            register_ptt_key,
            unregister_ptt_key
        ])
        .setup(|app| {
            let app_config_dir = app
                .path()
                .app_config_dir()
                .unwrap_or_else(|_| PathBuf::from("."));
            let config_path = app_config_dir.join("stoat-desktop/config.json");
            let state = AppState::load(config_path);
            let start_hidden = has_hidden_arg()
                || state
                    .config
                    .lock()
                    .map(|c| c.start_minimised_to_tray)
                    .unwrap_or(false);

            let initial_ptt_key = state.config.lock().ok().and_then(|c| c.ptt_key.clone());

            app.manage(state);

            if let Some(key) = initial_ptt_key {
                let handle = app.handle().clone();
                let handle_for_closure = handle.clone();
                if handle
                    .global_shortcut()
                    .on_shortcut(key.as_str(), move |_app, _shortcut, event| {
                        let name = match event.state {
                            ShortcutState::Pressed => "ptt-press",
                            ShortcutState::Released => "ptt-release",
                        };
                        let _ = handle_for_closure.emit(name, ());
                    })
                    .is_ok()
                {
                    if let Some(st) = app.try_state::<AppState>() {
                        if let Ok(mut current) = st.ptt_shortcut.lock() {
                            *current = Some(key);
                        }
                    }
                }
            }

            if let Some(window) = app.get_webview_window("main") {
                let _ = window.eval(INIT_SCRIPT);

                if let Some(state) = app.try_state::<AppState>() {
                    if let Ok(config) = state.config.lock() {
                        if config.window_state.width > 0.0 && config.window_state.height > 0.0 {
                            let _ = window.set_size(tauri::Size::Physical(tauri::PhysicalSize {
                                width: config.window_state.width as u32,
                                height: config.window_state.height as u32,
                            }));
                        }

                        if config.window_state.x >= 0 && config.window_state.y >= 0 {
                            let _ = window.set_position(tauri::Position::Physical(
                                tauri::PhysicalPosition {
                                    x: config.window_state.x,
                                    y: config.window_state.y,
                                },
                            ));
                        }

                        if config.window_state.is_maximised {
                            let _ = window.maximize();
                        }
                    }
                }

                if start_hidden {
                    let _ = window.hide();
                }

                if let Some(force_server) = parse_force_server_arg() {
                    if let Ok(encoded) = serde_json::to_string(&force_server) {
                        let _ = window.eval(&format!("window.location.replace({encoded});"));
                    } else {
                        let _ = window.eval(&format!(
                            "window.location.replace(\"{}\");",
                            DEFAULT_REMOTE_URL
                        ));
                    }
                }
            }

            let menu = MenuBuilder::new(app)
                .text("show_hide", "Show/Hide App")
                .text("quit", "Quit App")
                .build()?;

            let _tray = TrayIconBuilder::with_id("main")
                .menu(&menu)
                .tooltip("Stoat for Desktop")
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| {
                    if let Some(window) = app.get_webview_window("main") {
                        match event.id().as_ref() {
                            "show_hide" => {
                                if window.is_visible().unwrap_or(false) {
                                    let _ = window.hide();
                                } else {
                                    let _ = window.show();
                                    let _ = window.set_focus();
                                }
                            }
                            "quit" => {
                                if let Some(state) = app.try_state::<AppState>() {
                                    if let Ok(mut quitting) = state.quitting.lock() {
                                        *quitting = true;
                                    }
                                }
                                app.exit(0);
                            }
                            _ => {}
                        }
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }

            let Some(state) = window.try_state::<AppState>() else {
                return;
            };

            match event {
                WindowEvent::CloseRequested { api, .. } => {
                    let quitting = state.quitting.lock().map(|q| *q).unwrap_or(false);
                    let minimise_to_tray = state
                        .config
                        .lock()
                        .map(|c| c.minimise_to_tray)
                        .unwrap_or(true);

                    if !quitting && minimise_to_tray {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                }
                WindowEvent::Focused(true) => {
                    if let Ok(mut config) = state.config.lock() {
                        config.window_state.is_maximised = window.is_maximized().unwrap_or(false);
                    }
                    let _ = state.save();
                }
                WindowEvent::Moved(position) => {
                    if let Ok(mut config) = state.config.lock() {
                        config.window_state.x = position.x;
                        config.window_state.y = position.y;
                    }
                    let _ = state.save();
                }
                WindowEvent::Resized(size) => {
                    if let Ok(mut config) = state.config.lock() {
                        config.window_state.width = size.width as f64;
                        config.window_state.height = size.height as f64;
                    }
                    let _ = state.save();
                }
                _ => {}
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
