use clipboard_win::{get_clipboard_string, set_clipboard_string};
use serde::Serialize;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const COPY_SETTLE_TIMEOUT: Duration = Duration::from_millis(700);
const COPY_POLL_INTERVAL: Duration = Duration::from_millis(25);
const SENTINEL_PREFIX: &str = "__EASY_LAUNCHER_SELECTION_CAPTURE__";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionCaptureResult {
    pub ok: bool,
    pub text: String,
    pub message: String,
}

pub fn capture_selected_text() -> SelectionCaptureResult {
    match capture_selected_text_inner() {
        Ok(text) if !text.trim().is_empty() => SelectionCaptureResult {
            ok: true,
            text,
            message: "已获取选中文本".into(),
        },
        Ok(_) => SelectionCaptureResult {
            ok: false,
            text: String::new(),
            message: "没有读取到选中文本，请确认当前应用中已有文本选区".into(),
        },
        Err(error) => SelectionCaptureResult {
            ok: false,
            text: String::new(),
            message: error,
        },
    }
}

fn capture_selected_text_inner() -> Result<String, String> {
    let original_clipboard = get_clipboard_string().ok();
    let sentinel = selection_capture_sentinel();

    set_clipboard_string(&sentinel).map_err(|error| format!("准备读取选中文本失败：{error}"))?;

    let capture_result = send_copy_shortcut().and_then(|_| wait_for_copied_text(&sentinel));
    restore_text_clipboard(original_clipboard)?;

    capture_result
}

fn wait_for_copied_text(sentinel: &str) -> Result<String, String> {
    let started_at = Instant::now();
    let mut last_error = None;

    while started_at.elapsed() <= COPY_SETTLE_TIMEOUT {
        match get_clipboard_string() {
            Ok(text) if text != sentinel => return Ok(text),
            Ok(_) => {}
            Err(error) => last_error = Some(error.to_string()),
        }
        thread::sleep(COPY_POLL_INTERVAL);
    }

    if let Some(error) = last_error {
        Err(format!("读取剪贴板失败：{error}"))
    } else {
        Ok(String::new())
    }
}

fn selection_capture_sentinel() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{SENTINEL_PREFIX}{now}")
}

fn restore_text_clipboard(original_clipboard: Option<String>) -> Result<(), String> {
    let text = original_clipboard.unwrap_or_default();
    set_clipboard_string(&text).map_err(|error| format!("恢复剪贴板失败：{error}"))
}

#[cfg(windows)]
fn send_copy_shortcut() -> Result<(), String> {
    use std::mem::size_of;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_C, VK_CONTROL,
    };

    fn key_input(vk: u16, flags: u32) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    let inputs = [
        key_input(VK_CONTROL, 0),
        key_input(VK_C, 0),
        key_input(VK_C, KEYEVENTF_KEYUP),
        key_input(VK_CONTROL, KEYEVENTF_KEYUP),
    ];

    let sent = unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            size_of::<INPUT>() as i32,
        )
    };
    if sent == inputs.len() as u32 {
        Ok(())
    } else {
        Err("模拟 Ctrl+C 失败".into())
    }
}

#[cfg(not(windows))]
fn send_copy_shortcut() -> Result<(), String> {
    Err("当前平台暂不支持模拟复制取词".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_capture_result_has_empty_text() {
        let result = SelectionCaptureResult {
            ok: false,
            text: String::new(),
            message: "没有读取到选中文本".into(),
        };

        assert!(!result.ok);
        assert!(result.text.is_empty());
    }

    #[test]
    fn selection_capture_sentinel_has_expected_prefix() {
        assert!(selection_capture_sentinel().starts_with(SENTINEL_PREFIX));
    }
}
