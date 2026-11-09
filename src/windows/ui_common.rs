//! Shared Win32 native control, font, dialog, and message-loop helpers.
use super::wide;
use crate::model::AppResult;
use std::{
    mem::zeroed,
    ptr::{null, null_mut},
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::{CreateFontW, HFONT},
    System::LibraryLoader::GetModuleHandleW,
    UI::WindowsAndMessaging::*,
};

const BM_GETCHECK_MSG: u32 = 0x00F0;
const BM_SETCHECK_MSG: u32 = 0x00F1;

pub fn confirm_dialog(owner: HWND, title: &str, message: &str) -> bool {
    unsafe {
        MessageBoxW(
            owner,
            wide(message).as_ptr(),
            wide(title).as_ptr(),
            MB_YESNO | MB_ICONWARNING | MB_DEFBUTTON2,
        ) == IDYES
    }
}

pub fn info_dialog(owner: HWND, title: &str, message: &str) {
    unsafe {
        MessageBoxW(
            owner,
            wide(message).as_ptr(),
            wide(title).as_ptr(),
            MB_OK | MB_ICONINFORMATION,
        );
    }
}

pub fn warning_dialog(owner: HWND, title: &str, message: &str) {
    unsafe {
        MessageBoxW(
            owner,
            wide(message).as_ptr(),
            wide(title).as_ptr(),
            MB_OK | MB_ICONWARNING,
        );
    }
}

pub fn create_control(
    parent: HWND,
    id: u16,
    class: &str,
    label: &str,
    style: u32,
) -> AppResult<HWND> {
    let h = unsafe {
        CreateWindowExW(
            if class == "EDIT" || class == "LISTBOX" {
                WS_EX_CLIENTEDGE
            } else {
                0
            },
            wide(class).as_ptr(),
            wide(label).as_ptr(),
            WS_CHILD | WS_VISIBLE | style,
            0,
            0,
            10,
            10,
            parent,
            id as usize as HMENU,
            GetModuleHandleW(null()),
            null(),
        )
    };
    if h.is_null() {
        return Err(format!(
            "Could not create native control {id}: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(h)
}

pub fn create_dpi_font(face: &str, size_pt: u32, weight: i32, dpi: u32) -> HFONT {
    unsafe {
        CreateFontW(
            -((size_pt * dpi / 96) as i32),
            0,
            0,
            0,
            weight,
            0,
            0,
            0,
            1,
            0,
            0,
            0,
            0,
            wide(face).as_ptr(),
        )
    }
}

pub fn get_window_text(hwnd: HWND) -> String {
    let length = unsafe { GetWindowTextLengthW(hwnd) }.clamp(0, 32767) as usize;
    let mut value = vec![0u16; length + 1];
    let n = unsafe { GetWindowTextW(hwnd, value.as_mut_ptr(), value.len() as i32) }.max(0) as usize;
    String::from_utf16_lossy(&value[..n])
}

pub fn set_window_text_if_changed(hwnd: HWND, text: &str) {
    if get_window_text(hwnd) != text {
        unsafe {
            SetWindowTextW(hwnd, wide(text).as_ptr());
        }
    }
}

pub fn get_check(hwnd: HWND) -> bool {
    unsafe { SendMessageW(hwnd, BM_GETCHECK_MSG, 0, 0) == 1 }
}

pub fn set_check(hwnd: HWND, checked: bool) {
    unsafe {
        SendMessageW(hwnd, BM_SETCHECK_MSG, usize::from(checked), 0);
    }
}

pub fn run_dialog_message_loop(window: HWND) -> AppResult<()> {
    unsafe {
        let mut message: MSG = zeroed();
        loop {
            let status = GetMessageW(&mut message, null_mut(), 0, 0);
            if status < 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            if status == 0 {
                break;
            }
            if IsDialogMessageW(window, &message) == 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
    Ok(())
}
