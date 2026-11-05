#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod capture;
mod crypto;
mod dates;
mod repository;
mod windows;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Mutex,
};
use tauri::{Emitter, Manager, State, WebviewWindow};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
struct AppState {
    store: Mutex<repository::Store>,
    draft: Mutex<capture::Draft>,
    shortcut: Mutex<String>,
    capture_busy: AtomicBool,
    move_version: AtomicU64,
}
fn allowed(w: &WebviewWindow, labels: &[&str]) -> Result<(), String> {
    if labels.contains(&w.label()) {
        Ok(())
    } else {
        Err("此窗口无权执行该操作".into())
    }
}
fn changed(app: &tauri::AppHandle) {
    let _ = app.emit("domain-changed", ());
}
#[tauri::command]
fn snapshot(state: State<AppState>) -> Result<repository::Snapshot, String> {
    state.store.lock().map_err(|_| "状态忙")?.snapshot()
}
#[tauri::command]
fn clock(
    w: WebviewWindow,
    app: tauri::AppHandle,
    state: State<AppState>,
    action: String,
) -> Result<(), String> {
    allowed(&w, &["main", "widget"])?;
    let mut store = state.store.lock().map_err(|_| "状态忙")?;
    if store.is_locked() { return Err("应用已锁定，请先解锁".into()); }
    store.clock(&action)?;
    changed(&app);
    Ok(())
}
#[tauri::command]
fn add_capture(
    w: WebviewWindow,
    app: tauri::AppHandle,
    state: State<AppState>,
    title: String,
) -> Result<(), String> {
    allowed(&w, &["main", "quick_add"])?;
    let parsed = dates::parse(&title);
    state.store.lock().map_err(|_| "状态忙")?.save_task(repository::TaskInput {
        id: None, title, notes: None, planned_date: parsed.planned_date,
        date_kind: Some(parsed.date_kind), due_at: parsed.due_at,
        priority: Some("medium".into()), project: None, source: Some("capture".into()),
    })?;
    *state.draft.lock().map_err(|_| "状态忙")? = capture::Draft::default();
    changed(&app);
    Ok(())
}
#[tauri::command]
fn draft(w: WebviewWindow, state: State<AppState>) -> Result<capture::Draft, String> {
    allowed(&w, &["quick_add"])?;
    Ok(state.draft.lock().map_err(|_| "状态忙")?.clone())
}
#[tauri::command]
fn window_action(w: WebviewWindow, app: tauri::AppHandle, action: String) -> Result<(), String> {
    match action.as_str() {
        "main" => windows::show(&app, "main"),
        "widget" => {
            allowed(&w, &["main"])?;
            if let Some(widget) = app.get_webview_window("widget") {
                windows::place(&widget, true)?;
                widget.show().map_err(|_| "窗口显示失败")?;
            }
        }
        "quick" => {
            allowed(&w, &["main"])?;
            *app.state::<AppState>().draft.lock().map_err(|_| "状态忙")? =
                capture::Draft::default();
            windows::show(&app, "quick_add");
            let _ = app.emit_to("quick_add", "draft-changed", ());
        }
        "hide" => {
            w.hide().map_err(|_| "窗口隐藏失败")?;
        }
        "exit" => {
            allowed(&w, &["main"])?;
            app.exit(0);
        }
        _ => return Err("无效窗口操作".into()),
    };
    Ok(())
}
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Settings {
    autostart: bool,
    shortcut: String,
    lock_enabled: bool,
    locked: bool,
}
#[tauri::command]
fn settings(
    w: WebviewWindow,
    app: tauri::AppHandle,
    state: State<AppState>,
) -> Result<Settings, String> {
    allowed(&w, &["main"])?;
    let (lock_enabled, locked) = {
        let store = state.store.lock().map_err(|_| "状态忙")?;
        (store.lock_enabled(), store.is_locked())
    };
    Ok(Settings {
        autostart: app
            .autolaunch()
            .is_enabled()
            .map_err(|_| "启动项查询失败")?,
        shortcut: state.shortcut.lock().map_err(|_| "状态忙")?.clone(),
        lock_enabled,
        locked,
    })
}

#[tauri::command]
fn parse_task_date(text: String) -> dates::DateParse { dates::parse(&text) }

#[tauri::command]
fn tasks(w: WebviewWindow, state: State<AppState>, view: String, query: String) -> Result<Vec<repository::Task>, String> {
    allowed(&w, &["main"])?;
    state.store.lock().map_err(|_| "状态忙")?.tasks(&view, &query)
}

#[tauri::command]
fn save_task(w: WebviewWindow, app: tauri::AppHandle, state: State<AppState>, input: repository::TaskInput) -> Result<i64, String> {
    allowed(&w, &["main", "quick_add"])?;
    let id = state.store.lock().map_err(|_| "状态忙")?.save_task(input)?;
    *state.draft.lock().map_err(|_| "状态忙")? = capture::Draft::default(); changed(&app); Ok(id)
}

#[tauri::command]
fn task_action(w: WebviewWindow, app: tauri::AppHandle, state: State<AppState>, id: i64, action: String, result: Option<String>) -> Result<(), String> {
    allowed(&w, &["main", "widget"])?;
    state.store.lock().map_err(|_| "状态忙")?.task_action(id, &action, result.as_deref())?; changed(&app); Ok(())
}

#[tauri::command]
fn preferences(w: WebviewWindow, state: State<AppState>) -> Result<repository::Preferences, String> { allowed(&w, &["main", "widget"])?; state.store.lock().map_err(|_| "状态忙")?.preferences() }

#[tauri::command]
fn save_preferences(w: WebviewWindow, app: tauri::AppHandle, state: State<AppState>, input: repository::Preferences) -> Result<(), String> { allowed(&w, &["main"])?; state.store.lock().map_err(|_| "状态忙")?.save_preferences(&input)?; changed(&app); Ok(()) }

#[tauri::command]
fn workdays(w: WebviewWindow, state: State<AppState>) -> Result<Vec<repository::Workday>, String> { allowed(&w, &["main"])?; state.store.lock().map_err(|_| "状态忙")?.workdays() }

#[tauri::command]
fn save_workday(w: WebviewWindow, app: tauri::AppHandle, state: State<AppState>, input: repository::WorkdayInput) -> Result<(), String> { allowed(&w, &["main"])?; state.store.lock().map_err(|_| "状态忙")?.save_workday(input)?; changed(&app); Ok(()) }

#[tauri::command]
fn backups(w: WebviewWindow, state: State<AppState>) -> Result<Vec<repository::BackupInfo>, String> { allowed(&w, &["main"])?; state.store.lock().map_err(|_| "状态忙")?.backups() }

#[tauri::command]
fn create_backup(w: WebviewWindow, state: State<AppState>, password: Option<String>) -> Result<repository::BackupInfo, String> { allowed(&w, &["main"])?; state.store.lock().map_err(|_| "状态忙")?.create_backup(password.as_deref()) }

#[tauri::command]
fn restore_backup(w: WebviewWindow, app: tauri::AppHandle, state: State<AppState>, id: String, password: Option<String>) -> Result<(), String> { allowed(&w, &["main"])?; state.store.lock().map_err(|_| "状态忙")?.restore_backup(&id, password.as_deref())?; changed(&app); Ok(()) }

#[tauri::command]
fn unlock(w: WebviewWindow, app: tauri::AppHandle, state: State<AppState>, password: String) -> Result<(), String> { allowed(&w, &["main"])?; state.store.lock().map_err(|_| "状态忙")?.unlock(&password)?; changed(&app); Ok(()) }

#[tauri::command]
fn set_app_lock(w: WebviewWindow, app: tauri::AppHandle, state: State<AppState>, enabled: bool, password: String) -> Result<(), String> { allowed(&w, &["main"])?; state.store.lock().map_err(|_| "状态忙")?.set_lock(enabled, &password)?; changed(&app); Ok(()) }
#[tauri::command]
fn set_autostart(w: WebviewWindow, app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    allowed(&w, &["main"])?;
    if enabled {
        app.autolaunch().enable()
    } else {
        app.autolaunch().disable()
    }
    .map_err(|_| "开机启动设置失败".into())
}
fn register(app: &tauri::AppHandle, key: &str) -> Result<(), String> {
    app.global_shortcut()
        .on_shortcut(key, |app, _, event| {
            if event.state == ShortcutState::Pressed {
                if app
                    .state::<AppState>()
                    .capture_busy
                    .swap(true, Ordering::SeqCst)
                {
                    return;
                }
                let owner = app
                    .get_webview_window("quick_add")
                    .and_then(|w| w.hwnd().ok())
                    .map(|h| h.0 as usize)
                    .unwrap_or(0);
                let app = app.clone();
                std::thread::spawn(move || {
                    let draft = capture::capture(owner);
                    if let Ok(mut d) = app.state::<AppState>().draft.lock() {
                        *d = draft;
                    }
                    let main = app.clone();
                    let _ = app.run_on_main_thread(move || {
                        if let Some(w) = main.get_webview_window("quick_add") {
                            let _ = w.center();
                        }
                        windows::show(&main, "quick_add");
                        let _ = main.emit_to("quick_add", "draft-changed", ());
                        main.state::<AppState>()
                            .capture_busy
                            .store(false, Ordering::SeqCst);
                    });
                });
            }
        })
        .map_err(|_| "快捷键无效或已被其他应用占用，请更换组合键".into())
}
#[tauri::command]
fn set_shortcut(
    w: WebviewWindow,
    app: tauri::AppHandle,
    state: State<AppState>,
    key: String,
) -> Result<(), String> {
    allowed(&w, &["main"])?;
    if key.len() > 80 || !key.contains('+') {
        return Err("请输入带修饰键的组合，例如 Ctrl+Shift+Space".into());
    }
    let mut old = state.shortcut.lock().map_err(|_| "状态忙")?;
    if *old == key {
        return Ok(());
    }
    register(&app, &key)?;
    if let Err(e) = state
        .store
        .lock()
        .map_err(|_| "状态忙")?
        .set("shortcut", &key)
    {
        let _ = app.global_shortcut().unregister(key.as_str());
        return Err(e);
    }
    if !old.is_empty() {
        let _ = app.global_shortcut().unregister(old.as_str());
    }
    *old = key;
    Ok(())
}
fn main() {
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            windows::show(app, "main")
        }))
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .args(["--autostart"])
                .build(),
        )
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            snapshot,
            clock,
            add_capture,
            draft,
            window_action,
            settings,
            set_autostart,
            set_shortcut,
            parse_task_date,
            tasks,
            save_task,
            task_action,
            preferences,
            save_preferences,
            workdays,
            save_workday,
            backups,
            create_backup,
            restore_backup,
            unlock,
            set_app_lock
        ])
        .setup(|app| {
            let dir = app.path().app_data_dir()?.join("data");
            let store = repository::Store::open(&dir).map_err(std::io::Error::other)?;
            let key = store
                .setting("shortcut")
                .unwrap_or("Ctrl+Shift+Space".into());
            app.manage(AppState {
                store: Mutex::new(store),
                draft: Mutex::new(capture::Draft::default()),
                shortcut: Mutex::new(String::new()),
                capture_busy: AtomicBool::new(false),
                move_version: AtomicU64::new(0),
            });
            for config in &app.config().app.windows {
                tauri::WebviewWindowBuilder::from_config(app, config)?.build()?;
            }
            if register(app.handle(), &key).is_ok() {
                *app.state::<AppState>().shortcut.lock().unwrap() = key;
            }
            use tauri::{
                menu::{Menu, MenuItem},
                tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
            };
            let menu = Menu::with_items(
                app,
                &[
                    &MenuItem::with_id(app, "main", "打开早点下班", true, None::<&str>)?,
                    &MenuItem::with_id(app, "widget", "显示 / 隐藏悬浮窗", true, None::<&str>)?,
                    &MenuItem::with_id(app, "exit", "退出应用", true, None::<&str>)?,
                ],
            )?;
            let mut pixels = vec![0u8; 32 * 32 * 4];
            for p in pixels.chunks_exact_mut(4) {
                p.copy_from_slice(&[109, 94, 203, 255]);
            }
            TrayIconBuilder::new()
                .icon(tauri::image::Image::new_owned(pixels, 32, 32))
                .tooltip("早点下班")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "main" => windows::show(app, "main"),
                    "widget" => {
                        if let Some(w) = app.get_webview_window("widget") {
                            if w.is_visible().unwrap_or(false) {
                                let _ = w.hide();
                            } else {
                                let _ = windows::place(&w, true);
                                let _ = w.show();
                            }
                        }
                    }
                    "exit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if matches!(
                        event,
                        TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        }
                    ) {
                        windows::show(tray.app_handle(), "main");
                    }
                })
                .build(app)?;
            if let Some(w) = app.get_webview_window("widget") {
                let _ = windows::place(&w, true);
                w.show()?;
            }
            if !std::env::args().any(|s| s == "--autostart") {
                windows::show(app.handle(), "main");
            }
            Ok(())
        })
        .on_window_event(|w, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                let _ = w.hide();
            }
            tauri::WindowEvent::Moved(_) | tauri::WindowEvent::ScaleFactorChanged { .. }
                if w.label() == "widget" =>
            {
                let app = w.app_handle().clone();
                let seq = app
                    .state::<AppState>()
                    .move_version
                    .fetch_add(1, Ordering::SeqCst)
                    + 1;
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    if app.state::<AppState>().move_version.load(Ordering::SeqCst) != seq {
                        return;
                    }
                    let a = app.clone();
                    let _ = app.run_on_main_thread(move || {
                        if let Some(w) = a.get_webview_window("widget") {
                            let _ = windows::place(&w, false);
                        }
                    });
                });
            }
            _ => {}
        })
        .run(tauri::generate_context!());
    if result.is_err() {
        unsafe {
            use windows_sys::Win32::UI::WindowsAndMessaging::*;
            let msg: Vec<u16> =
                "应用启动失败。请检查本地数据、账户密钥或磁盘权限；原数据未删除。\0"
                    .encode_utf16()
                    .collect();
            let title: Vec<u16> = "早点下班\0".encode_utf16().collect();
            MessageBoxW(
                std::ptr::null_mut(),
                msg.as_ptr(),
                title.as_ptr(),
                MB_OK | MB_ICONERROR,
            );
        }
    }
}
