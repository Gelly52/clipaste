use crate::common;
use std::ptr;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::DataExchange::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Memory::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const WM_CLIPBOARDUPDATE: u32 = 0x031D;

static mut LAST_SEQ: u32 = 0;

/// Check if clipboard has image data but no text or file drop
fn is_image_only_clipboard() -> bool {
    unsafe {
        let has_dib = IsClipboardFormatAvailable(CF_DIB as u32) != 0;
        let has_bitmap = IsClipboardFormatAvailable(CF_BITMAP as u32) != 0;
        let has_text = IsClipboardFormatAvailable(CF_UNICODETEXT as u32) != 0;
        let has_hdrop = IsClipboardFormatAvailable(CF_HDROP as u32) != 0;

        (has_dib || has_bitmap) && !has_text && !has_hdrop
    }
}

/// Read CF_DIB data from clipboard and convert to PNG
fn read_clipboard_as_png() -> Option<Vec<u8>> {
    unsafe {
        if OpenClipboard(0) == 0 {
            return None;
        }

        let result = (|| {
            let handle = GetClipboardData(CF_DIB as u32);
            if handle == 0 {
                return None;
            }

            let ptr = GlobalLock(handle as *mut _);
            if ptr.is_null() {
                return None;
            }

            let size = GlobalSize(handle as *mut _);
            let data = std::slice::from_raw_parts(ptr as *const u8, size);
            let dib_data = data.to_vec();

            GlobalUnlock(handle as *mut _);
            Some(dib_data)
        })();

        CloseClipboard();

        let dib_data = result?;
        common::dib_to_png(&dib_data)
    }
}

/// Write a file path as text to the clipboard, preserving image data
fn write_path_to_clipboard(path: &str, png_data: &[u8]) {
    unsafe {
        if OpenClipboard(0) == 0 {
            return;
        }

        EmptyClipboard();

        // Write file path as CF_UNICODETEXT
        let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        let byte_len = wide.len() * 2;
        let hmem = GlobalAlloc(GMEM_MOVEABLE, byte_len);
        if hmem.is_null() {
            CloseClipboard();
            return;
        }
        let dest = GlobalLock(hmem);
        if !dest.is_null() {
            ptr::copy_nonoverlapping(wide.as_ptr() as *const u8, dest as *mut u8, byte_len);
            GlobalUnlock(hmem);
            SetClipboardData(CF_UNICODETEXT as u32, hmem as isize);
        }

        // Also register a custom "PNG" format with the PNG data
        // Some tools check for this
        let png_format_name: Vec<u16> = "PNG\0".encode_utf16().collect();
        let png_format = RegisterClipboardFormatW(png_format_name.as_ptr());
        if png_format != 0 {
            let hmem_png = GlobalAlloc(GMEM_MOVEABLE, png_data.len());
            if !hmem_png.is_null() {
                let dest_png = GlobalLock(hmem_png);
                if !dest_png.is_null() {
                    ptr::copy_nonoverlapping(
                        png_data.as_ptr(),
                        dest_png as *mut u8,
                        png_data.len(),
                    );
                    GlobalUnlock(hmem_png);
                    SetClipboardData(png_format, hmem_png as isize);
                }
            }
        }

        CloseClipboard();
    }
}

fn normalize() {
    if !is_image_only_clipboard() {
        return;
    }

    let png_data = match read_clipboard_as_png() {
        Some(d) => d,
        None => {
            common::log("failed to read clipboard image as PNG");
            return;
        }
    };

    let file_path = match common::save_png_to_temp(&png_data) {
        Some(p) => p,
        None => return,
    };

    let path_str = file_path.to_string_lossy().to_string();
    write_path_to_clipboard(&path_str, &png_data);

    let filename = file_path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();
    common::log(&format!("normalized {filename} ({} bytes)", png_data.len()));

    common::clean_old_temp_files();
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_CLIPBOARDUPDATE => {
            // Deduplicate: check clipboard sequence number
            let seq = GetClipboardSequenceNumber();
            if seq != LAST_SEQ {
                LAST_SEQ = seq;
                normalize();
            }
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

pub fn run() {
    common::ensure_temp_dir();
    common::log(&format!(
        "v{} started (pid {})",
        common::VERSION,
        std::process::id()
    ));

    unsafe {
        let class_name: Vec<u16> = "clipaste_hidden\0".encode_utf16().collect();
        let hinstance = GetModuleHandleW(ptr::null());

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance,
            lpszClassName: class_name.as_ptr(),
            style: 0,
            cbClsExtra: 0,
            cbWndExtra: 0,
            hIcon: 0,
            hCursor: 0,
            hbrBackground: 0,
            lpszMenuName: ptr::null(),
        };

        RegisterClassW(&wc);

        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            class_name.as_ptr(),
            0,               // no style (hidden)
            0, 0, 0, 0,      // position/size don't matter
            HWND_MESSAGE,     // message-only window
            0,
            hinstance,
            ptr::null(),
        );

        if hwnd == 0 {
            common::log("failed to create hidden window");
            std::process::exit(1);
        }

        if AddClipboardFormatListener(hwnd) == 0 {
            common::log("failed to register clipboard listener");
            std::process::exit(1);
        }

        common::log("clipboard listener registered (event-driven, no polling)");

        // Message loop
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, 0, 0, 0) > 0 {
            DispatchMessageW(&msg);
        }

        RemoveClipboardFormatListener(hwnd);
    }
}
