// #![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod pie_menu;
use parking_lot::Mutex;
use pie_menu::{Color, Item, PieMenu, Style};
use std::convert::Into;
use windows::Win32::UI::Input::KeyboardAndMouse::{self, VIRTUAL_KEY};
use windows::Win32::UI::WindowsAndMessaging::{
    KBDLLHOOKSTRUCT, LLKHF_ALTDOWN, WH_KEYBOARD_LL, WM_KEYDOWN, WM_SYSKEYDOWN,
};
use windows::{
    core::Result,
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        System::Com::{CoInitialize, CoUninitialize},
        UI::WindowsAndMessaging::{
            self, CallNextHookEx, DefWindowProcW, DispatchMessageW, GetMessageW, SetWindowsHookExW,
            UnhookWindowsHookEx, MSG,
        },
    },
};

pub static ACTIVE_PIE_MENU: once_cell::sync::Lazy<Mutex<Option<PieMenu>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(None));

extern "system" fn wndproc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message {
            WindowsAndMessaging::WM_PAINT => {
                if let Some(ref mut active_pie_menu) = *ACTIVE_PIE_MENU.lock() {
                    active_pie_menu.paint(hwnd);
                }
                LRESULT(0)
            }
            WindowsAndMessaging::WM_DESTROY => LRESULT(0),
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}

unsafe extern "system" fn low_level_keyboard_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code < 0 {
        return CallNextHookEx(None, code, wparam, lparam);
    }

    let hook_lparam = *(lparam.0 as *const KBDLLHOOKSTRUCT);

    let mut handled = false;

    if wparam.0 as u32 == WM_KEYDOWN || wparam.0 as u32 == WM_SYSKEYDOWN {
        let alt_down = (hook_lparam.flags & LLKHF_ALTDOWN).0 != 0;
        let keycode = VIRTUAL_KEY(hook_lparam.vkCode as u16);

        if alt_down {
            if let Some(ref mut active_pie_menu) = *ACTIVE_PIE_MENU.lock() {
                handled = match keycode {
                    KeyboardAndMouse::VK_P => {
                        if let Err(err) = active_pie_menu.show() {
                            eprintln!("couldn't show pie menu `{err}`");
                        }
                        true
                    }
                    _ => false,
                };
            }
        }
    }

    if handled {
        LRESULT(-1)
    } else {
        CallNextHookEx(None, code, wparam, lparam)
    }
}

fn main() -> Result<()> {
    unsafe {
        CoInitialize(None)?;

        let low_level_keyboard_hook =
            SetWindowsHookExW(WH_KEYBOARD_LL, Some(low_level_keyboard_proc), None, 0).unwrap();

        let pie_menu = PieMenu::create(
            vec![
                Item::new("Item1", || println!("Firsty")),
                Item::new("Item2", || println!("Secondy")),
                Item::new("Item3", || println!("Thirdy")),
                Item::new("Item4", || println!("Fourthy")),
            ],
            Style {
                height: 20,
                width: 100,
                roundness_radius: 8,
                label_color: Color::Black,
                color: Color::PolycountGray,
            },
            100f32,
            600,
            600,
            Some(wndproc),
        );

        *ACTIVE_PIE_MENU.lock() = Some(pie_menu.unwrap());

        let mut message = MSG::default();

        while GetMessageW(&mut message, None, 0, 0).into() {
            DispatchMessageW(&message);
        }

        if UnhookWindowsHookEx(low_level_keyboard_hook).is_err() {
            eprintln!("couldn't unhook the low level keyboard hook");
        }

        CoUninitialize();

        Ok(())
    }
}
