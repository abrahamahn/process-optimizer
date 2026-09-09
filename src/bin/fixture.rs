//! Benign test-owned process. Never distributed in the application artifact.
fn main() {
    if std::env::args().any(|v| v == "--window") {
        #[cfg(windows)]
        {
            window();
            return;
        }
    }
    std::thread::sleep(std::time::Duration::from_secs(45));
}

#[cfg(windows)]
fn window() {
    use process_optimizer::windows::wide;
    use std::{
        mem::zeroed,
        ptr::{null, null_mut},
    };
    use windows_sys::Win32::{
        Foundation::*, System::LibraryLoader::GetModuleHandleW, UI::WindowsAndMessaging::*,
    };
    unsafe extern "system" fn proc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        if m == WM_DESTROY {
            PostQuitMessage(0);
            return 0;
        }
        DefWindowProcW(h, m, w, l)
    }
    unsafe {
        let instance = GetModuleHandleW(null());
        let name = wide("ProcessOptimizerTestFixture");
        let mut class: WNDCLASSW = zeroed();
        class.hInstance = instance;
        class.lpszClassName = name.as_ptr();
        class.lpfnWndProc = Some(proc);
        RegisterClassW(&class);
        let h = CreateWindowExW(
            0,
            name.as_ptr(),
            wide("Optimizer test fixture").as_ptr(),
            WS_OVERLAPPEDWINDOW,
            0,
            0,
            160,
            120,
            null_mut(),
            null_mut(),
            instance,
            null(),
        );
        if h.is_null() {
            std::process::exit(1);
        }
        let mut msg = zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
