//! User-triggered plain Unicode text capture. Restore old clipboard data only when it is safely
//! representable as Unicode text; otherwise keep the user's newly copied selection.
use serde::Serialize;
use std::{
    ptr::null_mut,
    thread::sleep,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{GlobalFree, HWND},
    System::{DataExchange::*, Memory::*},
    UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::GetForegroundWindow},
};
const UNICODE: u32 = 13;
#[derive(Clone, Default, Serialize)]
pub struct Draft {
    pub text: String,
    pub notice: String,
}
struct Clipboard;
impl Clipboard {
    fn open() -> Result<Self, String> {
        for _ in 0..10 {
            if unsafe { OpenClipboard(null_mut()) } != 0 {
                return Ok(Self);
            }
            sleep(Duration::from_millis(20));
        }
        Err("剪贴板被占用，请手动粘贴".into())
    }
}
impl Drop for Clipboard {
    fn drop(&mut self) {
        unsafe {
            CloseClipboard();
        }
    }
}
unsafe fn read_text() -> Result<String, String> {
    let h = GetClipboardData(UNICODE);
    if h.is_null() {
        return Ok(String::new());
    }
    let size = GlobalSize(h);
    if size > 128 * 1024 {
        return Err("文本过长，请手动粘贴".into());
    }
    let p = GlobalLock(h) as *const u16;
    if p.is_null() {
        return Err("无法读取剪贴板，请手动粘贴".into());
    }
    let slice = std::slice::from_raw_parts(p, size / 2);
    let len = slice.iter().position(|x| *x == 0).unwrap_or(slice.len());
    let result = String::from_utf16_lossy(&slice[..len]);
    GlobalUnlock(h);
    Ok(result)
}
unsafe fn write_text(text: &str) -> Result<(), String> {
    let data: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    let h = GlobalAlloc(GMEM_MOVEABLE, data.len() * 2);
    if h.is_null() {
        return Err("剪贴板恢复内存分配失败".into());
    }
    let p = GlobalLock(h) as *mut u16;
    if p.is_null() {
        GlobalFree(h);
        return Err("剪贴板恢复失败".into());
    }
    std::ptr::copy_nonoverlapping(data.as_ptr(), p, data.len());
    GlobalUnlock(h);
    if EmptyClipboard() == 0 || SetClipboardData(UNICODE, h).is_null() {
        GlobalFree(h);
        return Err("剪贴板恢复失败，请检查当前剪贴板".into());
    }
    Ok(())
}
fn capture_inner(owner: HWND) -> Result<Draft, String> {
    unsafe {
        let foreground: HWND = GetForegroundWindow();
        let old;
        let before;
        let can_restore;
        {
            let _guard = Clipboard::open()?;
            let mut format = EnumClipboardFormats(0);
            let mut text_only = true;
            while format != 0 {
                if ![1, 7, 13, 16].contains(&format) {
                    text_only = false;
                }
                format = EnumClipboardFormats(format);
            }
            can_restore = text_only;
            old = if IsClipboardFormatAvailable(UNICODE) != 0 {
                Some(read_text()?)
            } else {
                None
            };
            before = GetClipboardSequenceNumber();
        }
        // Wait until the physical shortcut modifiers are released; never focus our card first.
        let deadline = Instant::now() + Duration::from_secs(2);
        while [VK_CONTROL, VK_SHIFT, VK_MENU]
            .iter()
            .any(|k| GetAsyncKeyState(*k as i32) < 0)
        {
            if Instant::now() > deadline {
                return Err("快捷键按住过久，请松开后重试".into());
            }
            sleep(Duration::from_millis(15));
        }
        if GetForegroundWindow() != foreground || GetClipboardSequenceNumber() != before {
            return Err("前台窗口或剪贴板已变化，请重新触发".into());
        }
        keybd_event(VK_CONTROL as u8, 0, 0, 0);
        keybd_event(b'C', 0, 0, 0);
        keybd_event(b'C', 0, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_CONTROL as u8, 0, KEYEVENTF_KEYUP, 0);
        let deadline = Instant::now() + Duration::from_millis(900);
        while GetClipboardSequenceNumber() == before {
            if Instant::now() > deadline {
                return Err("未获取到选中文字，请手动输入或粘贴".into());
            }
            sleep(Duration::from_millis(20));
        }
        let (text, captured_seq);
        {
            let _guard = Clipboard::open()?;
            captured_seq = GetClipboardSequenceNumber();
            text = read_text();
        }
        let mut notice = if can_restore {
            "已提取选中文字；原纯文本剪贴板已恢复".to_string()
        } else {
            "已提取选中文字；原剪贴板含富文本，本次复制结果已保留".to_string()
        };
        if can_restore {
            let _guard = Clipboard::open()?;
            if GetClipboardSequenceNumber() == captured_seq {
                // Only restore while the clipboard is exclusively locked and its sequence matches.
                // OpenClipboard needs a real owner for SetClipboardData after EmptyClipboard.
                drop(_guard);
                if OpenClipboard(owner) != 0 {
                    let _locked = Clipboard;
                    if GetClipboardSequenceNumber() == captured_seq {
                        if let Some(old) = old {
                            write_text(&old)?;
                        } else if EmptyClipboard() == 0 {
                            return Err("原空剪贴板恢复失败".into());
                        }
                    } else {
                        notice = "期间剪贴板已变化，已保留新内容".into();
                    }
                } else {
                    notice = "内容已采集，但原剪贴板未恢复".into();
                }
            } else {
                notice = "期间剪贴板已变化，已保留新内容".into();
            }
        }
        let text = text?;
        if text.trim().is_empty() {
            return Ok(Draft {
                text: String::new(),
                notice: "选中文字为空，请手动输入".into(),
            });
        }
        Ok(Draft {
            text: text.chars().take(2000).collect(),
            notice,
        })
    }
}
pub fn capture(owner: usize) -> Draft {
    capture_inner(owner as HWND).unwrap_or_else(|notice| Draft {
        text: String::new(),
        notice,
    })
}
