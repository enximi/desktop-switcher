use std::sync::atomic::{AtomicI32, Ordering};

use windows::{
    Win32::{
        Foundation::{LPARAM, LRESULT, WPARAM},
        UI::WindowsAndMessaging::{
            CallNextHookEx, HC_ACTION, HHOOK, MSLLHOOKSTRUCT, SetWindowsHookExW,
            UnhookWindowsHookEx, WH_MOUSE_LL, WM_MOUSEWHEEL,
        },
    },
    core::Result,
};

use crate::{
    desktop_switch::{self, Direction},
    screen_edge,
};

static EDGE_WIDTH_PX: AtomicI32 = AtomicI32::new(4);

pub fn install(edge_width_px: i32) -> Result<HookGuard> {
    EDGE_WIDTH_PX.store(edge_width_px, Ordering::Relaxed);

    let hook = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), None, 0)? };

    Ok(HookGuard(hook))
}

unsafe extern "system" fn mouse_proc(code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && w_param.0 as u32 == WM_MOUSEWHEEL {
        let event = unsafe { &*(l_param.0 as *const MSLLHOOKSTRUCT) };
        let edge_width_px = EDGE_WIDTH_PX.load(Ordering::Relaxed);

        if screen_edge::is_at_left_edge(event.pt, edge_width_px) {
            let wheel_delta = high_word(event.mouseData) as i16;
            if wheel_delta != 0 {
                let direction = if wheel_delta > 0 {
                    Direction::Left
                } else {
                    Direction::Right
                };

                if let Err(error) = desktop_switch::switch_desktop(direction) {
                    eprintln!("切换虚拟桌面失败: {error}");
                }
            }

            return LRESULT(1);
        }
    }

    unsafe { CallNextHookEx(None, code, w_param, l_param) }
}

fn high_word(value: u32) -> u16 {
    ((value >> 16) & 0xffff) as u16
}

pub struct HookGuard(HHOOK);

impl Drop for HookGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = UnhookWindowsHookEx(self.0);
        }
    }
}
