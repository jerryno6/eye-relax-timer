#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::missing_panics_doc)]

use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tauri::{
    image::Image,
    menu::{IconMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, RunEvent, State, WebviewUrl,
    WebviewWindowBuilder, WindowEvent, Wry,
};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

const DEFAULT_DURATION_MINUTES: u32 = 20;
const MIN_DURATION_MINUTES: u32 = 1;
const MAX_DURATION_MINUTES: u32 = 240;
const BREAK_SECONDS: u64 = 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    duration_minutes: u32,
    repeat_enabled: bool,
    autostart_enabled: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            duration_minutes: DEFAULT_DURATION_MINUTES,
            repeat_enabled: true,
            autostart_enabled: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum TimerStatus {
    Stopped,
    Running,
    Paused,
    BreakVisible,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimerSnapshot {
    status: TimerStatus,
    remaining_seconds: u64,
    break_remaining_seconds: u64,
}

impl TimerSnapshot {
    fn stopped(settings: &AppSettings) -> Self {
        Self {
            status: TimerStatus::Stopped,
            remaining_seconds: duration_seconds(settings),
            break_remaining_seconds: 0,
        }
    }
}

struct TrayMenuHandles {
    status: MenuItem<Wry>,
    start: IconMenuItem<Wry>,
    pause: IconMenuItem<Wry>,
    stop: IconMenuItem<Wry>,
}

struct SharedState {
    settings: Mutex<AppSettings>,
    timer: Mutex<TimerSnapshot>,
    tray_menu: Mutex<Option<TrayMenuHandles>>,
    generation: AtomicU64,
    allow_exit: AtomicBool,
}

impl SharedState {
    fn new(settings: AppSettings) -> Self {
        Self {
            timer: Mutex::new(TimerSnapshot::stopped(&settings)),
            settings: Mutex::new(settings),
            tray_menu: Mutex::new(None),
            generation: AtomicU64::new(1),
            allow_exit: AtomicBool::new(false),
        }
    }

    fn next_generation(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    fn is_current_generation(&self, generation: u64) -> bool {
        self.generation.load(Ordering::SeqCst) == generation
    }
}

#[tauri::command]
fn get_settings(state: State<'_, Arc<SharedState>>) -> Result<AppSettings, String> {
    Ok(state.settings.lock().map_err(lock_error)?.clone())
}

#[tauri::command]
fn save_settings(
    app: AppHandle,
    state: State<'_, Arc<SharedState>>,
    settings: AppSettings,
) -> Result<(), String> {
    validate_settings(&settings)?;
    apply_autostart(&app, settings.autostart_enabled)?;
    persist_settings(&app, &settings)?;

    {
        *state.settings.lock().map_err(lock_error)? = settings.clone();
        let mut timer = state.timer.lock().map_err(lock_error)?;
        if timer.status == TimerStatus::Stopped {
            *timer = TimerSnapshot::stopped(&settings);
        }
    }

    emit_timer_state(&app, &state)?;
    Ok(())
}

#[tauri::command]
fn get_timer_state(state: State<'_, Arc<SharedState>>) -> Result<TimerSnapshot, String> {
    Ok(state.timer.lock().map_err(lock_error)?.clone())
}

#[tauri::command]
fn start_timer(app: AppHandle, state: State<'_, Arc<SharedState>>) -> Result<(), String> {
    start_timer_inner(&app, state.inner().clone(), None)
}

#[tauri::command]
fn pause_timer(app: AppHandle, state: State<'_, Arc<SharedState>>) -> Result<(), String> {
    toggle_pause_timer(&app, state.inner().clone())
}

#[tauri::command]
fn stop_timer(app: AppHandle, state: State<'_, Arc<SharedState>>) -> Result<(), String> {
    stop_timer_inner(&app, state.inner().clone())
}

#[tauri::command]
fn close_break_popup(app: AppHandle, state: State<'_, Arc<SharedState>>) -> Result<(), String> {
    finish_break(&app, state.inner().clone())
}

pub fn run() {
    let context = tauri::generate_context!();
    let settings = AppSettings::default();
    let shared = Arc::new(SharedState::new(settings));
    let run_state = shared.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec![]),
        ))
        .manage(shared.clone())
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            get_timer_state,
            start_timer,
            pause_timer,
            stop_timer,
            close_break_popup
        ])
        .setup(move |app| {
            let app_handle = app.handle().clone();
            if let Ok(settings) = load_settings(&app_handle) {
                if let Ok(mut stored_settings) = shared.settings.lock() {
                    *stored_settings = settings.clone();
                }
                if let Ok(mut timer) = shared.timer.lock() {
                    *timer = TimerSnapshot::stopped(&settings);
                }
            }
            create_tray(&app_handle, shared.clone())?;
            update_tray_menu(&shared)?;
            let _ = open_settings_window(&app_handle);
            Ok(())
        })
        .build(context)
        .expect("error while building tauri application")
        .run(move |_app_handle, event| {
            if let RunEvent::ExitRequested { api, .. } = event {
                if !run_state.allow_exit.load(Ordering::SeqCst) {
                    api.prevent_exit();
                }
            }
        });
}

fn create_tray(app: &AppHandle, state: Arc<SharedState>) -> tauri::Result<()> {
    let status = MenuItem::with_id(app, "status", "Status: Ready", false, None::<&str>)?;
    let start_icon = Image::from_bytes(include_bytes!("../icons/menu-start.png"))?;
    let start = IconMenuItem::with_id(app, "start", "Start", true, Some(start_icon), None::<&str>)?;
    let pause_icon = Image::from_bytes(include_bytes!("../icons/menu-pause.png"))?;
    let pause = IconMenuItem::with_id(app, "pause", "Pause", false, Some(pause_icon), None::<&str>)?;
    let stop_icon = Image::from_bytes(include_bytes!("../icons/menu-stop.png"))?;
    let stop = IconMenuItem::with_id(app, "stop", "Stop", true, Some(stop_icon), None::<&str>)?;
    let settings_icon = Image::from_bytes(include_bytes!("../icons/menu-settings.png"))?;
    let settings = IconMenuItem::with_id(app, "settings", "Settings", true, Some(settings_icon), None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let separator_one = PredefinedMenuItem::separator(app)?;
    let separator_two = PredefinedMenuItem::separator(app)?;

    let menu = Menu::with_items(
        app,
        &[
            &status,
            &separator_one,
            &start,
            &pause,
            &stop,
            &settings,
            &separator_two,
            &quit,
        ],
    )?;

    *state
        .tray_menu
        .lock()
        .expect("failed to lock tray menu state") = Some(TrayMenuHandles {
        status,
        start,
        pause,
        stop,
    });

    let menu_state = state.clone();
    let tray_icon = Image::from_bytes(include_bytes!("../icons/icon.png"))?;
    TrayIconBuilder::with_id("main-tray")
        .icon(tray_icon)
        .icon_as_template(false)
        .tooltip("Eye Relax Timer")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| {
            let app_handle = app.clone();
            let state = menu_state.clone();
            match event.id().as_ref() {
                "start" => {
                    let _ = start_timer_inner(&app_handle, state, None);
                }
                "pause" => {
                    let _ = toggle_pause_timer(&app_handle, state);
                }
                "stop" => {
                    let _ = stop_timer_inner(&app_handle, state);
                }
                "settings" => {
                    let _ = open_settings_window(&app_handle);
                }
                "quit" => {
                    state.allow_exit.store(true, Ordering::SeqCst);
                    app_handle.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let _ = open_settings_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

fn open_settings_window(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("settings") {
        window.show().map_err(error_to_string)?;
        window.set_focus().map_err(error_to_string)?;
        return Ok(());
    }

    let window = WebviewWindowBuilder::new(
        app,
        "settings",
        WebviewUrl::App("index.html?window=settings".into()),
    )
    .title("Eye Relax Timer")
    .inner_size(420.0, 560.0)
    .min_inner_size(360.0, 480.0)
    .resizable(false)
    .build()
    .map_err(error_to_string)?;

    let window_to_hide = window.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = window_to_hide.hide();
        }
    });

    Ok(())
}

fn show_break_window(app: &AppHandle) -> Result<(), String> {
    let (size, position) = break_window_geometry(app)?;

    if let Some(window) = app.get_webview_window("break") {
        window.set_size(size).map_err(error_to_string)?;
        window.set_position(position).map_err(error_to_string)?;
        window.set_always_on_top(true).map_err(error_to_string)?;
        window.show().map_err(error_to_string)?;
        window.set_focus().map_err(error_to_string)?;
        return Ok(());
    }

    WebviewWindowBuilder::new(app, "break", WebviewUrl::App("index.html?window=break".into()))
        .title("Eye Break")
        .inner_size(size.width, size.height)
        .position(position.x, position.y)
        .resizable(false)
        .decorations(false)
        .always_on_top(true)
        .focused(true)
        .skip_taskbar(true)
        .build()
        .map_err(error_to_string)?;

    Ok(())
}

fn break_window_geometry(
    app: &AppHandle,
) -> Result<(LogicalSize<f64>, LogicalPosition<f64>), String> {
    let monitor = app
        .primary_monitor()
        .map_err(error_to_string)?
        .ok_or_else(|| "No primary monitor found".to_string())?;
    let scale = monitor.scale_factor();
    let physical_size = monitor.size();
    let physical_position = monitor.position();

    let monitor_width = f64::from(physical_size.width) / scale;
    let monitor_height = f64::from(physical_size.height) / scale;
    let width = monitor_width * 0.8;
    let height = monitor_height * 0.8;
    let x = f64::from(physical_position.x) / scale + (monitor_width - width) / 2.0;
    let y = f64::from(physical_position.y) / scale + (monitor_height - height) / 2.0;

    Ok((LogicalSize::new(width, height), LogicalPosition::new(x, y)))
}

fn start_timer_inner(
    app: &AppHandle,
    state: Arc<SharedState>,
    override_remaining: Option<u64>,
) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("break") {
        let _ = window.close();
    }

    let settings = state.settings.lock().map_err(lock_error)?.clone();
    let remaining = override_remaining.unwrap_or_else(|| duration_seconds(&settings));
    if remaining == 0 {
        return Err("Timer duration must be greater than zero".to_string());
    }

    let generation = state.next_generation();
    {
        let mut timer = state.timer.lock().map_err(lock_error)?;
        timer.status = TimerStatus::Running;
        timer.remaining_seconds = remaining;
        timer.break_remaining_seconds = 0;
    }
    emit_timer_state(app, &state)?;

    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        run_countdown(app_handle, state, generation, remaining).await;
    });

    Ok(())
}

async fn run_countdown(
    app: AppHandle,
    state: Arc<SharedState>,
    generation: u64,
    mut remaining: u64,
) {
    while remaining > 0 {
        tokio::time::sleep(Duration::from_secs(1)).await;
        if !state.is_current_generation(generation) {
            return;
        }
        remaining -= 1;
        if let Ok(mut timer) = state.timer.lock() {
            timer.status = TimerStatus::Running;
            timer.remaining_seconds = remaining;
            timer.break_remaining_seconds = 0;
        }
        let _ = emit_timer_state(&app, &state);
    }

    if !state.is_current_generation(generation) {
        return;
    }

    if show_break_window(&app).is_err() {
        let _ = stop_timer_inner(&app, state);
        return;
    }

    if let Ok(mut timer) = state.timer.lock() {
        timer.status = TimerStatus::BreakVisible;
        timer.remaining_seconds = 0;
        timer.break_remaining_seconds = BREAK_SECONDS;
    }
    let _ = emit_timer_state(&app, &state);

    run_break_countdown(app, state, generation).await;
}

async fn run_break_countdown(app: AppHandle, state: Arc<SharedState>, generation: u64) {
    let mut remaining = BREAK_SECONDS;
    while remaining > 0 {
        tokio::time::sleep(Duration::from_secs(1)).await;
        if !state.is_current_generation(generation) {
            return;
        }
        remaining -= 1;
        if let Ok(mut timer) = state.timer.lock() {
            timer.status = TimerStatus::BreakVisible;
            timer.remaining_seconds = 0;
            timer.break_remaining_seconds = remaining;
        }
        let _ = emit_timer_state(&app, &state);
    }

    if state.is_current_generation(generation) {
        let _ = finish_break(&app, state);
    }
}

fn toggle_pause_timer(app: &AppHandle, state: Arc<SharedState>) -> Result<(), String> {
    let snapshot = state.timer.lock().map_err(lock_error)?.clone();
    match snapshot.status {
        TimerStatus::Running => {
            state.next_generation();
            {
                let mut timer = state.timer.lock().map_err(lock_error)?;
                timer.status = TimerStatus::Paused;
                timer.break_remaining_seconds = 0;
            }
            emit_timer_state(app, &state)
        }
        TimerStatus::Paused => start_timer_inner(app, state, Some(snapshot.remaining_seconds)),
        _ => Ok(()),
    }
}

fn stop_timer_inner(app: &AppHandle, state: Arc<SharedState>) -> Result<(), String> {
    state.next_generation();
    if let Some(window) = app.get_webview_window("break") {
        let _ = window.close();
    }
    let settings = state.settings.lock().map_err(lock_error)?.clone();
    {
        *state.timer.lock().map_err(lock_error)? = TimerSnapshot::stopped(&settings);
    }
    emit_timer_state(app, &state)
}

fn finish_break(app: &AppHandle, state: Arc<SharedState>) -> Result<(), String> {
    state.next_generation();
    if let Some(window) = app.get_webview_window("break") {
        let _ = window.close();
    }

    let settings = state.settings.lock().map_err(lock_error)?.clone();
    if settings.repeat_enabled {
        start_timer_inner(app, state, Some(duration_seconds(&settings)))
    } else {
        {
            *state.timer.lock().map_err(lock_error)? = TimerSnapshot::stopped(&settings);
        }
        emit_timer_state(app, &state)
    }
}

fn emit_timer_state(app: &AppHandle, state: &Arc<SharedState>) -> Result<(), String> {
    let snapshot = state.timer.lock().map_err(lock_error)?.clone();
    update_tray_menu(state)?;
    app.emit("timer-state", snapshot).map_err(error_to_string)
}

fn update_tray_menu(state: &Arc<SharedState>) -> Result<(), String> {
    let snapshot = state.timer.lock().map_err(lock_error)?.clone();
    let tray_menu = state.tray_menu.lock().map_err(lock_error)?;
    let Some(menu) = tray_menu.as_ref() else {
        return Ok(());
    };

    let status = match snapshot.status {
        TimerStatus::Stopped => format!("Status: Ready ({})", format_seconds(snapshot.remaining_seconds)),
        TimerStatus::Running => format!("Status: {}", format_seconds(snapshot.remaining_seconds)),
        TimerStatus::Paused => format!("Status: Paused ({})", format_seconds(snapshot.remaining_seconds)),
        TimerStatus::BreakVisible => {
            format!("Status: Break ({})", format_seconds(snapshot.break_remaining_seconds))
        }
    };

    menu.status.set_text(status).map_err(error_to_string)?;
    menu.start
        .set_enabled(true)
        .map_err(error_to_string)?;
    menu.pause
        .set_text(if snapshot.status == TimerStatus::Paused {
            "Resume"
        } else {
            "Pause"
        })
        .map_err(error_to_string)?;
    menu.pause
        .set_enabled(matches!(
            snapshot.status,
            TimerStatus::Running | TimerStatus::Paused
        ))
        .map_err(error_to_string)?;
    menu.stop
        .set_enabled(snapshot.status != TimerStatus::Stopped)
        .map_err(error_to_string)?;

    Ok(())
}

fn load_settings(app: &AppHandle) -> Result<AppSettings, String> {
    let contents = fs::read_to_string(settings_path(app)?).map_err(error_to_string)?;
    let settings = serde_json::from_str(&contents).map_err(error_to_string)?;
    validate_settings(&settings)?;
    Ok(settings)
}

fn persist_settings(app: &AppHandle, settings: &AppSettings) -> Result<(), String> {
    let path = settings_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(error_to_string)?;
    }
    let contents = serde_json::to_string_pretty(settings).map_err(error_to_string)?;
    fs::write(path, contents).map_err(error_to_string)
}

fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    let mut path = app
        .path()
        .app_config_dir()
        .map_err(error_to_string)?;
    path.push("settings.json");
    Ok(path)
}

fn validate_settings(settings: &AppSettings) -> Result<(), String> {
    if !(MIN_DURATION_MINUTES..=MAX_DURATION_MINUTES).contains(&settings.duration_minutes) {
        return Err(format!(
            "Duration must be between {MIN_DURATION_MINUTES} and {MAX_DURATION_MINUTES} minutes"
        ));
    }
    Ok(())
}

fn apply_autostart(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let autostart = app.autolaunch();
    if enabled {
        autostart.enable().map_err(error_to_string)?;
    } else if let Err(err) = autostart.disable() {
        if !is_missing_autostart_entry_error(&err) {
            return Err(error_to_string(err));
        }
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn is_missing_autostart_entry_error(error: &impl std::fmt::Display) -> bool {
    error.to_string().contains("os error 2")
}

#[cfg(not(target_os = "windows"))]
fn is_missing_autostart_entry_error(_error: &impl std::fmt::Display) -> bool {
    false
}

fn duration_seconds(settings: &AppSettings) -> u64 {
    u64::from(settings.duration_minutes) * 60
}

fn format_seconds(seconds: u64) -> String {
    let minutes = seconds / 60;
    let seconds = seconds % 60;
    format!("{minutes:02}:{seconds:02}")
}

fn lock_error<T>(_: std::sync::PoisonError<T>) -> String {
    "Internal state lock failed".to_string()
}

fn error_to_string(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_match_v1_behavior() {
        let settings = AppSettings::default();

        assert_eq!(settings.duration_minutes, 20);
        assert!(settings.repeat_enabled);
        assert!(!settings.autostart_enabled);
    }

    #[test]
    fn timer_snapshot_starts_ready_with_full_duration() {
        let settings = AppSettings {
            duration_minutes: 30,
            repeat_enabled: false,
            autostart_enabled: false,
        };

        let timer = TimerSnapshot::stopped(&settings);

        assert_eq!(timer.status, TimerStatus::Stopped);
        assert_eq!(timer.remaining_seconds, 30 * 60);
        assert_eq!(timer.break_remaining_seconds, 0);
    }

    #[test]
    fn duration_validation_rejects_out_of_range_values() {
        let valid = AppSettings {
            duration_minutes: 1,
            repeat_enabled: true,
            autostart_enabled: false,
        };
        let too_short = AppSettings {
            duration_minutes: 0,
            ..valid.clone()
        };
        let too_long = AppSettings {
            duration_minutes: 241,
            ..valid.clone()
        };

        assert!(validate_settings(&valid).is_ok());
        assert!(validate_settings(&too_short).is_err());
        assert!(validate_settings(&too_long).is_err());
    }

    #[test]
    fn seconds_are_formatted_as_timer_text() {
        assert_eq!(format_seconds(0), "00:00");
        assert_eq!(format_seconds(9), "00:09");
        assert_eq!(format_seconds(65), "01:05");
        assert_eq!(format_seconds(3600), "60:00");
    }
}
