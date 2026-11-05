use crate::AppState;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, PhysicalPosition, WebviewWindow};
#[derive(Serialize, Deserialize)]
struct Placement {
    monitor: String,
    x: f64,
    y: f64,
}
pub fn show(app: &AppHandle, label: &str) {
    if let Some(w) = app.get_webview_window(label) {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}
pub fn place(w: &WebviewWindow, restore: bool) -> Result<(), String> {
    let app = w.app_handle();
    let state = app.state::<AppState>();
    let saved = if restore {
        state
            .store
            .lock()
            .map_err(|_| "状态忙")?
            .setting("widget_position")
            .and_then(|s| serde_json::from_str::<Placement>(&s).ok())
    } else {
        None
    };
    let monitors = w.available_monitors().map_err(|_| "无法读取显示器")?;
    let current = w.outer_position().map_err(|_| "无法读取窗口位置")?;
    let selected = saved
        .as_ref()
        .and_then(|s| {
            monitors
                .iter()
                .find(|m| m.name().map(|n| n == &s.monitor).unwrap_or(false))
        })
        .or_else(|| {
            monitors.iter().find(|m| {
                let a = m.work_area();
                current.x >= a.position.x
                    && current.x < a.position.x + a.size.width as i32
                    && current.y >= a.position.y
                    && current.y < a.position.y + a.size.height as i32
            })
        })
        .or(monitors.first())
        .ok_or("没有可用显示器")?;
    let area = selected.work_area();
    let scale = selected.scale_factor();
    let size = w.outer_size().map_err(|_| "无法读取窗口尺寸")?;
    let (x, y) = saved
        .map(|s| {
            (
                area.position.x + (s.x * scale) as i32,
                area.position.y + (s.y * scale) as i32,
            )
        })
        .unwrap_or((current.x, current.y));
    let max_x = (area.position.x + area.size.width as i32 - size.width as i32).max(area.position.x);
    let max_y =
        (area.position.y + area.size.height as i32 - size.height as i32).max(area.position.y);
    let mut x = x.clamp(area.position.x, max_x);
    let mut y = y.clamp(area.position.y, max_y);
    let snap = (20.0 * scale) as i32;
    if x - area.position.x < snap {
        x = area.position.x;
    }
    if max_x - x < snap {
        x = max_x;
    }
    if y - area.position.y < snap {
        y = area.position.y;
    }
    if max_y - y < snap {
        y = max_y;
    }
    if x != current.x || y != current.y {
        w.set_position(PhysicalPosition::new(x, y))
            .map_err(|_| "窗口位置校正失败")?;
    }
    let position = Placement {
        monitor: selected.name().cloned().unwrap_or_default(),
        x: (x - area.position.x) as f64 / scale,
        y: (y - area.position.y) as f64 / scale,
    };
    let result = state.store.lock().map_err(|_| "状态忙")?.set(
        "widget_position",
        &serde_json::to_string(&position).map_err(|_| "窗口位置保存失败")?,
    );
    result
}
