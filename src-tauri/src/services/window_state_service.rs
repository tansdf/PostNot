use std::{
    fs,
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};
use tauri::{
    AppHandle, Manager, PhysicalPosition, PhysicalSize, Position, Size, WebviewWindow, WindowEvent,
};

use crate::{
    error::{AppError, AppResult},
    storage::paths,
};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct WindowState {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    maximized: bool,
}

pub fn restore_and_track_main_window(window: &WebviewWindow) {
    let restored_state = match restore_window(window) {
        Ok(state) => state,
        Err(error) => {
            eprintln!("Failed to restore window state: {}", error);
            None
        }
    };

    let cached_state =
        Arc::new(Mutex::new(restored_state.unwrap_or_else(|| {
            snapshot_window_state(window).unwrap_or_default()
        })));

    let tracked_window = window.clone();
    let tracked_state = Arc::clone(&cached_state);

    window.on_window_event(move |event| match event {
        WindowEvent::Moved(_)
        | WindowEvent::Resized(_)
        | WindowEvent::ScaleFactorChanged { .. } => {
            refresh_cached_state(&tracked_window, &tracked_state);
        }
        WindowEvent::CloseRequested { .. } => {
            refresh_cached_state(&tracked_window, &tracked_state);
            persist_cached_state(tracked_window.app_handle(), &tracked_state);
        }
        _ => {}
    });
}

fn restore_window(window: &WebviewWindow) -> AppResult<Option<WindowState>> {
    let Some(state) = load_window_state(window.app_handle())? else {
        return Ok(None);
    };

    if state.width > 0 && state.height > 0 {
        window
            .set_size(Size::Physical(PhysicalSize::new(state.width, state.height)))
            .map_err(|error| AppError::Message(error.to_string()))?;
    }

    if is_visible_on_any_monitor(window, &state)? {
        window
            .set_position(Position::Physical(PhysicalPosition::new(state.x, state.y)))
            .map_err(|error| AppError::Message(error.to_string()))?;
    }

    if state.maximized {
        window
            .maximize()
            .map_err(|error| AppError::Message(error.to_string()))?;
    }

    Ok(Some(state))
}

fn refresh_cached_state(window: &WebviewWindow, cache: &Arc<Mutex<WindowState>>) {
    let Ok(updated_state) = merged_window_state(window, cache) else {
        return;
    };

    if let Ok(mut guard) = cache.lock() {
        *guard = updated_state;
    }
}

fn persist_cached_state(app: &AppHandle, cache: &Arc<Mutex<WindowState>>) {
    let Some(state) = cache.lock().ok().map(|guard| (*guard).clone()) else {
        return;
    };

    if let Err(error) = save_window_state(app, &state) {
        eprintln!("Failed to persist window state: {}", error);
    }
}

fn merged_window_state(
    window: &WebviewWindow,
    cache: &Arc<Mutex<WindowState>>,
) -> AppResult<WindowState> {
    let mut state = cache
        .lock()
        .ok()
        .map(|guard| (*guard).clone())
        .unwrap_or_default();

    state.maximized = window
        .is_maximized()
        .map_err(|error| AppError::Message(error.to_string()))?;

    if !state.maximized {
        let position = window
            .outer_position()
            .map_err(|error| AppError::Message(error.to_string()))?;
        let size = window
            .inner_size()
            .map_err(|error| AppError::Message(error.to_string()))?;

        state.x = position.x;
        state.y = position.y;
        state.width = size.width;
        state.height = size.height;
    }

    Ok(state)
}

fn snapshot_window_state(window: &WebviewWindow) -> AppResult<WindowState> {
    let position = window
        .outer_position()
        .map_err(|error| AppError::Message(error.to_string()))?;
    let size = window
        .inner_size()
        .map_err(|error| AppError::Message(error.to_string()))?;
    let maximized = window
        .is_maximized()
        .map_err(|error| AppError::Message(error.to_string()))?;

    Ok(WindowState {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
        maximized,
    })
}

fn load_window_state(app: &AppHandle) -> AppResult<Option<WindowState>> {
    let path = paths::window_state_path(app)?;

    if !path.exists() {
        return Ok(None);
    }

    let contents = fs::read_to_string(path)?;
    let state = serde_json::from_str(&contents)?;
    Ok(Some(state))
}

fn save_window_state(app: &AppHandle, state: &WindowState) -> AppResult<()> {
    let path = paths::window_state_path(app)?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let contents = serde_json::to_string_pretty(state)?;
    fs::write(path, contents)?;
    Ok(())
}

fn is_visible_on_any_monitor(window: &WebviewWindow, state: &WindowState) -> AppResult<bool> {
    let monitors = window
        .available_monitors()
        .map_err(|error| AppError::Message(error.to_string()))?;

    if monitors.is_empty() {
        return Ok(true);
    }

    let right = i64::from(state.x) + i64::from(state.width);
    let bottom = i64::from(state.y) + i64::from(state.height);

    Ok(monitors.into_iter().any(|monitor| {
        let area = monitor.work_area();
        let area_left = i64::from(area.position.x);
        let area_top = i64::from(area.position.y);
        let area_right = area_left + i64::from(area.size.width);
        let area_bottom = area_top + i64::from(area.size.height);

        right > area_left
            && i64::from(state.x) < area_right
            && bottom > area_top
            && i64::from(state.y) < area_bottom
    }))
}
