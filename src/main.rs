#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use windows::{
    core::{w, Result, PCWSTR},
    Win32::{
        Foundation::{COLORREF, HMODULE, HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Gdi::{
            BeginPaint, Ellipse, EndPaint, GetStockObject, SelectObject, UpdateWindow, HBRUSH,
            PAINTSTRUCT, WHITE_BRUSH,
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{
            self, CreateWindowExW, DefWindowProcW, DispatchMessageW, GetClientRect, GetMessageW,
            LoadCursorW, PostQuitMessage, RegisterClassW, SetLayeredWindowAttributes, ShowWindow,
            CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, IDC_ARROW, LWA_COLORKEY, MSG, SW_SHOW,
            WNDCLASSW, WS_EX_LAYERED, WS_EX_TOPMOST, WS_POPUP, WS_VISIBLE,
        },
    },
};

pub fn module_handle_w() -> Result<HMODULE> {
    unsafe { GetModuleHandleW(None) }
}

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message {
            WindowsAndMessaging::WM_PAINT => {
                println!("WM_PAINT");
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rect = RECT::default();
                GetClientRect(hwnd, &mut rect).unwrap();
                let hbr_old = SelectObject(hdc, GetStockObject(WHITE_BRUSH));
                Ellipse(hdc, rect.left, rect.top, rect.right, rect.bottom);
                SelectObject(hdc, hbr_old);
                EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            WindowsAndMessaging::WM_DESTROY => {
                println!("WM_DESTROY");
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}

const WINDOW_CLASS_NAME: PCWSTR = w!("Classy");
const WINDOW_NAME: PCWSTR = w!("Windy");

fn main() -> Result<()> {
    unsafe {
        let instance = GetModuleHandleW(None)?;
        debug_assert!(instance.0 != 0);

        let wc = WNDCLASSW {
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hInstance: instance.into(),
            lpszClassName: WINDOW_CLASS_NAME,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            hbrBackground: HBRUSH(0x00000000),
            ..Default::default()
        };

        let atom = RegisterClassW(&wc);
        debug_assert!(atom != 0);

        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_LAYERED,
            WINDOW_CLASS_NAME,
            WINDOW_NAME,
            WS_POPUP | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            800,
            600,
            None,
            None,
            instance,
            None,
        );

        SetLayeredWindowAttributes(hwnd, COLORREF(0x00000000), 0, LWA_COLORKEY).unwrap();

        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);

        let mut message = MSG::default();

        while GetMessageW(&mut message, None, 0, 0).into() {
            DispatchMessageW(&message);
        }

        Ok(())
    }
}
