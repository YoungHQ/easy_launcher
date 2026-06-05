use crate::storage::StorageState;
use crate::{emit_selection_capture, show_main_window, AppHandle};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

pub const DOUBLE_ALT_ENABLED_KEY: &str = "launcher.double_alt.enabled";
pub const SELECTION_TRIGGER_MODE_KEY: &str = "selection.trigger.mode";
pub const SELECTION_ENABLED_KEY: &str = "selection.enabled";
pub const SELECTION_TRIGGER_MODE_CTRL_MOUSE: &str = "ctrl_mouse";

#[cfg(windows)]
const DRAG_THRESHOLD_PX: i32 = 8;
#[cfg(windows)]
const POLL_INTERVAL: Duration = Duration::from_millis(10);
#[cfg(windows)]
const CAPTURE_DELAY: Duration = Duration::from_millis(120);
#[cfg(windows)]
const DOUBLE_ALT_INTERVAL: Duration = Duration::from_millis(360);

pub struct SelectionTriggerHandle {
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}

impl SelectionTriggerHandle {
    pub fn start(app: AppHandle, storage: StorageState) -> Self {
        start_platform_trigger(app, storage)
    }
}

impl Drop for SelectionTriggerHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(windows)]
fn start_platform_trigger(app: AppHandle, storage: StorageState) -> SelectionTriggerHandle {
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = stop.clone();
    let worker = thread::spawn(move || run_windows_trigger(app, storage, worker_stop));

    SelectionTriggerHandle {
        stop,
        worker: Some(worker),
    }
}

#[cfg(not(windows))]
fn start_platform_trigger(_app: AppHandle, _storage: StorageState) -> SelectionTriggerHandle {
    SelectionTriggerHandle {
        stop: Arc::new(AtomicBool::new(false)),
        worker: None,
    }
}

#[cfg(windows)]
fn run_windows_trigger(app: AppHandle, storage: StorageState, stop: Arc<AtomicBool>) {
    use willhook::event::{
        InputEvent, IsEventInjected, KeyPress, KeyboardKey, MouseButton, MouseButtonPress,
        MouseClick, MouseEventType,
    };

    let Some(hook) = willhook::willhook() else {
        return;
    };

    let mut left_down_at: Option<(i32, i32)> = None;
    let mut current_point: Option<(i32, i32)> = None;
    let mut last_alt_up_at: Option<Instant> = None;

    while !stop.load(Ordering::Relaxed) {
        let event = match hook.try_recv() {
            Ok(event) => event,
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                thread::sleep(POLL_INTERVAL);
                continue;
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
        };

        if !ctrl_mouse_trigger_enabled(&storage) {
            left_down_at = None;
            continue;
        }

        if !is_ctrl_pressed() {
            left_down_at = None;
        }

        match event {
            InputEvent::Keyboard(event) => {
                if matches!(event.is_injected, Some(IsEventInjected::Injected)) {
                    continue;
                }
                if matches!(
                    event.key,
                    Some(KeyboardKey::LeftAlt | KeyboardKey::RightAlt)
                ) && matches!(event.pressed, KeyPress::Up(_))
                {
                    if double_alt_enabled(&storage) {
                        let now = Instant::now();
                        if last_alt_up_at
                            .map(|last| now.duration_since(last) <= DOUBLE_ALT_INTERVAL)
                            .unwrap_or(false)
                        {
                            show_main_window(&app);
                            last_alt_up_at = None;
                        } else {
                            last_alt_up_at = Some(now);
                        }
                    } else {
                        last_alt_up_at = None;
                    }
                }
                if matches!(
                    event.key,
                    Some(KeyboardKey::LeftControl | KeyboardKey::RightControl)
                ) {
                    if matches!(event.pressed, KeyPress::Up(_)) {
                        left_down_at = None;
                    }
                }
            }
            InputEvent::Mouse(event) => {
                if matches!(event.is_injected, Some(IsEventInjected::Injected)) {
                    continue;
                }
                match event.event {
                    MouseEventType::Move(move_event) => {
                        if let Some(point) = move_event.point {
                            current_point = Some((point.x, point.y));
                        }
                    }
                    MouseEventType::Press(press_event) => {
                        if !matches!(
                            press_event.button,
                            MouseButton::Left(MouseClick::SingleClick)
                        ) {
                            continue;
                        }
                        match press_event.pressed {
                            MouseButtonPress::Down if is_ctrl_pressed() => {
                                left_down_at = current_point;
                            }
                            MouseButtonPress::Up => {
                                let should_capture = is_ctrl_pressed()
                                    && left_down_at
                                        .zip(current_point)
                                        .map(|(start, end)| moved_far_enough(start, end))
                                        .unwrap_or(false);
                                left_down_at = None;
                                if should_capture {
                                    let capture_point = current_point;
                                    thread::sleep(CAPTURE_DELAY);
                                    emit_selection_capture(&app, capture_point);
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
            InputEvent::Other(_) => {}
        }
    }
}

#[cfg(windows)]
fn ctrl_mouse_trigger_enabled(storage: &StorageState) -> bool {
    let selection_enabled = bool_setting(storage, SELECTION_ENABLED_KEY, true, |value| {
        value == "true"
    });
    selection_enabled
        && bool_setting(storage, SELECTION_TRIGGER_MODE_KEY, false, |value| {
            value == SELECTION_TRIGGER_MODE_CTRL_MOUSE
        })
}

#[cfg(windows)]
fn double_alt_enabled(storage: &StorageState) -> bool {
    bool_setting(storage, DOUBLE_ALT_ENABLED_KEY, true, |value| {
        value == "true"
    })
}

#[cfg(windows)]
fn bool_setting(
    storage: &StorageState,
    key: &str,
    default: bool,
    predicate: impl FnOnce(&str) -> bool,
) -> bool {
    storage
        .lock()
        .ok()
        .and_then(|storage| storage.get_setting(key).ok())
        .flatten()
        .map(|value| predicate(value.trim()))
        .unwrap_or(default)
}

#[cfg(windows)]
fn moved_far_enough(start: (i32, i32), end: (i32, i32)) -> bool {
    (start.0 - end.0).abs() >= DRAG_THRESHOLD_PX || (start.1 - end.1).abs() >= DRAG_THRESHOLD_PX
}

#[cfg(windows)]
fn is_ctrl_pressed() -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VK_CONTROL, VK_LCONTROL, VK_RCONTROL,
    };

    let pressed_mask = 0x8000u16 as i16;
    unsafe {
        GetAsyncKeyState(VK_CONTROL as i32) & pressed_mask != 0
            || GetAsyncKeyState(VK_LCONTROL as i32) & pressed_mask != 0
            || GetAsyncKeyState(VK_RCONTROL as i32) & pressed_mask != 0
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn moved_far_enough_requires_drag_threshold() {
        assert!(!moved_far_enough((10, 10), (17, 10)));
        assert!(moved_far_enough((10, 10), (18, 10)));
        assert!(moved_far_enough((10, 10), (10, 18)));
    }
}
