use anyhow::{Context, Result};
use std::convert::Into;
use vecmath::Vector2;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::UI::WindowsAndMessaging::{DestroyWindow, ShowWindow, HHOOK, SW_SHOW};
use windows::{
    core::{w, PCWSTR},
    Win32::{
        Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
        Graphics::Gdi::{
            self, BeginPaint, ClientToScreen, DrawTextExW, EndPaint, GetStockObject, SelectObject,
            SetBkMode, SetTextColor, BACKGROUND_MODE, DT_CENTER, DT_SINGLELINE, DT_VCENTER, HBRUSH,
            PAINTSTRUCT, WHITE_BRUSH,
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{
            CallNextHookEx, CreateWindowExW, GetClientRect, GetCursorPos, LoadCursorW,
            RegisterClassW, SetLayeredWindowAttributes, SetWindowPos, SetWindowsHookExW,
            UnhookWindowsHookEx, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, HWND_TOPMOST, IDC_ARROW,
            LWA_COLORKEY, SWP_NOSIZE, WH_MOUSE_LL, WM_MOUSEMOVE, WNDCLASSW, WS_EX_LAYERED,
            WS_EX_TOPMOST, WS_POPUP, WS_VISIBLE,
        },
    },
};

use crate::ACTIVE_PIE_MENU;

pub enum Color {
    Black = 0x00000000,
    PolycountGray = 0x00202020,
}

impl From<Color> for HBRUSH {
    fn from(value: Color) -> Self {
        HBRUSH(value as isize)
    }
}

impl From<Color> for COLORREF {
    fn from(value: Color) -> Self {
        COLORREF(value as u32)
    }
}

pub struct Item {
    pub label: String,
    pub position: Vector2<f32>,
    pub action: fn(),
}

impl Item {
    pub fn new<S: Into<String>>(label: S, action: fn()) -> Self {
        Self {
            label: label.into(),
            position: (0.0, 0.0).into(),
            action,
        }
    }
}

pub struct Style {
    pub height: i32,
    pub width: i32,
    pub roundness_radius: i32,
    pub label_color: Color,
    pub color: Color,
}

pub struct PieMenu {
    pub hwnd: Option<HWND>,
    pub instance: HMODULE,
    pub position: Option<POINT>,
    pub max_width: i32,
    pub max_height: i32,
    pub items: Vec<Item>,
    pub item_style: Style,
    pub trigger_distance: f32,
    low_level_mouse_hook_handle: Option<HHOOK>,
}

const WINDOW_CLASS_NAME: PCWSTR = w!("PieMenuClass");
const WINDOW_NAME: PCWSTR = w!("PieMenu");

unsafe extern "system" fn low_level_mouse_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code < 0 {
        return CallNextHookEx(None, code, wparam, lparam);
    }

    if wparam.0 as u32 == WM_MOUSEMOVE {
        if let Some(ref active_pie_menu) = *ACTIVE_PIE_MENU.lock() {
            if let Ok(sorted_items_from_mouse) = active_pie_menu.sorted_items_from_mouse() {
                if !sorted_items_from_mouse.is_empty() {
                    (sorted_items_from_mouse[0].action)();
                    if let Err(err) = active_pie_menu.close() {
                        eprintln!("{err}");
                    }
                }
            }
        }
    }

    CallNextHookEx(None, code, wparam, lparam)
}

impl PieMenu {
    pub fn create(
        items: Vec<Item>,
        item_style: Style,
        trigger_distance: f32,
        max_width: i32,
        max_height: i32,
        wnd_proc: Option<unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT>,
    ) -> Result<Self> {
        unsafe {
            let instance = GetModuleHandleW(None)?;
            debug_assert!(instance.0 != 0);

            let wc = WNDCLASSW {
                hCursor: LoadCursorW(None, IDC_ARROW)?,
                hInstance: instance.into(),
                lpszClassName: WINDOW_CLASS_NAME,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: wnd_proc,
                hbrBackground: Color::Black.into(),
                ..Default::default()
            };

            let atom = RegisterClassW(&wc);
            debug_assert!(atom != 0);

            let pie_menu = Self {
                low_level_mouse_hook_handle: None,
                hwnd: None,
                instance,
                position: None,
                max_width,
                max_height,
                items,
                item_style,
                trigger_distance,
            };

            Ok(pie_menu)
        }
    }

    pub unsafe fn paint(&mut self, hwnd: HWND) {
        let mut ps = PAINTSTRUCT::default();
        let hdc = BeginPaint(hwnd, &mut ps);
        let mut rect = RECT::default();
        GetClientRect(hwnd, &mut rect).unwrap();

        let hbr_old = SelectObject(hdc, GetStockObject(WHITE_BRUSH));
        SetTextColor(hdc, Into::<COLORREF>::into(Color::PolycountGray));
        SetBkMode(hdc, BACKGROUND_MODE(0));

        let mut theta = 0f32;
        let center_x = ((rect.right - rect.left) / 2) as f32;
        let center_y = ((rect.bottom - rect.top) / 2) as f32;
        {
            {
                let mut screen_pos = POINT {
                    x: center_x as i32,
                    y: center_y as i32,
                };
                ClientToScreen(hwnd, &mut screen_pos);
            }
        }
        let max_theta = 2f32 * std::f32::consts::PI;
        let step = max_theta / (self.items.len() as f32);
        let padding = 4 * self.items.len() as i32;
        let radius = (self.item_style.width + padding) as f32;

        let mut item_index = 0;
        while theta < max_theta && item_index < self.items.len() {
            let x = center_x + radius * theta.cos();
            let y = center_y + radius * theta.sin();
            theta += step;
            let position = (x as i32, y as i32);
            let mut item_rect = RECT {
                left: position.0 - (self.item_style.width / 2),
                top: position.1 - (self.item_style.height / 2),
                right: position.0 + (self.item_style.width / 2),
                bottom: position.1 + (self.item_style.height / 2),
            };

            let mut item_screen_pos = POINT {
                x: position.0,
                y: position.1,
            };

            ClientToScreen(hwnd, &mut item_screen_pos);
            self.items[item_index].position =
                (item_screen_pos.x as f32, item_screen_pos.y as f32).into();

            Gdi::RoundRect(
                hdc,
                item_rect.left,
                item_rect.top,
                item_rect.right,
                item_rect.bottom,
                self.item_style.roundness_radius,
                self.item_style.roundness_radius,
            );

            let mut lpchtext = self.items[item_index]
                .label
                .encode_utf16()
                .collect::<Vec<_>>();
            DrawTextExW(
                hdc,
                &mut lpchtext,
                &mut item_rect,
                DT_SINGLELINE | DT_CENTER | DT_VCENTER,
                None,
            );

            item_index += 1;
        }

        SelectObject(hdc, hbr_old);
        EndPaint(hwnd, &ps);
    }

    fn sorted_items_from_mouse(&self) -> Result<Vec<&Item>> {
        let mut cursor_pos = POINT::default();
        unsafe { GetCursorPos(&mut cursor_pos)? };
        let pie_center: Vector2<f32> = [
            self.position.unwrap().x as f32,
            self.position.unwrap().y as f32,
        ];

        let cursor_pos: Vector2<f32> = [cursor_pos.x as f32, cursor_pos.y as f32];
        let center_to_cursor_dir = vecmath::vec2_sub(pie_center, cursor_pos);
        let distance_from_pie_center = vecmath::vec2_len(center_to_cursor_dir);
        let center_to_cursor_dir_normalized = vecmath::vec2_normalized(center_to_cursor_dir);

        let mut dotted_items: Vec<(&Item, f32)> = Vec::new();
        self.items.iter().for_each(|item| {
            let dot = vecmath::vec2_dot(
                center_to_cursor_dir_normalized,
                vecmath::vec2_normalized(vecmath::vec2_sub(pie_center, item.position)),
            );
            if dot > 0.95 && distance_from_pie_center >= self.trigger_distance {
                dotted_items.push((item, dot));
            }
        });
        dotted_items.sort_by(|a, b| b.1.total_cmp(&a.1));

        Ok(dotted_items.iter().map(|i| i.0).collect())
    }

    pub fn show(&mut self) -> Result<()> {
        unsafe {
            self.low_level_mouse_hook_handle = Some(SetWindowsHookExW(
                WH_MOUSE_LL,
                Some(low_level_mouse_proc),
                None,
                0,
            )?);

            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_LAYERED,
                WINDOW_CLASS_NAME,
                WINDOW_NAME,
                WS_POPUP | WS_VISIBLE,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                self.max_width,
                self.max_height,
                None,
                None,
                self.instance,
                None,
            );

            self.hwnd = Some(hwnd);

            SetLayeredWindowAttributes(
                hwnd,
                Into::<COLORREF>::into(Color::Black),
                0,
                LWA_COLORKEY,
            )?;

            let mut cursor_pos = POINT::default();
            GetCursorPos(&mut cursor_pos)?;
            self.position = Some(cursor_pos);
            SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                cursor_pos.x - (self.max_width / 2),
                cursor_pos.y - (self.max_height / 2),
                0,
                0,
                SWP_NOSIZE,
            )?;

            ShowWindow(hwnd, SW_SHOW);
        }

        Ok(())
    }

    pub fn close(&self) -> Result<()> {
        unsafe {
            if let Some(hwnd) = self.hwnd {
                if let Some(hook) = self.low_level_mouse_hook_handle {
                    if UnhookWindowsHookEx(hook).is_err() {
                        eprintln!("couldn't unhook on close");
                    }
                }
                DestroyWindow(hwnd).context("couldn't destroy window")?;
            }

            Ok(())
        }
    }
}
