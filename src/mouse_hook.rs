use std::{
    sync::OnceLock,
    sync::atomic::{AtomicI32, AtomicU64, Ordering},
    time::Instant,
};

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

const SWITCH_THROTTLE_MS: u64 = 250;

static EDGE_WIDTH_PX: AtomicI32 = AtomicI32::new(4);
static START_TIME: OnceLock<Instant> = OnceLock::new();
static LAST_SWITCH_MS: AtomicU64 = AtomicU64::new(0);

pub fn install(edge_width_px: i32) -> Result<HookGuard> {
    EDGE_WIDTH_PX.store(edge_width_px, Ordering::Relaxed);
    START_TIME.get_or_init(Instant::now);

    let hook = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), None, 0)? };

    Ok(HookGuard(hook))
}

unsafe extern "system" fn mouse_proc(code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && w_param.0 as u32 == WM_MOUSEWHEEL {
        let event = unsafe { &*(l_param.0 as *const MSLLHOOKSTRUCT) };
        let edge_width_px = EDGE_WIDTH_PX.load(Ordering::Relaxed);

        if screen_edge::is_at_left_edge(event.pt, edge_width_px) {
            let wheel_delta = high_word(event.mouseData) as i16;
            if wheel_delta != 0 && throttle_allows_switch() {
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

fn throttle_allows_switch() -> bool {
    let Some(start_time) = START_TIME.get() else {
        return false;
    };

    let elapsed_ms = start_time.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;

    loop {
        let last_switch_ms = LAST_SWITCH_MS.load(Ordering::Relaxed);
        if last_switch_ms != 0 && elapsed_ms.saturating_sub(last_switch_ms) < SWITCH_THROTTLE_MS {
            return false;
        }

        if LAST_SWITCH_MS
            .compare_exchange(
                last_switch_ms,
                elapsed_ms,
                Ordering::AcqRel,
                Ordering::Relaxed,
            )
            .is_ok()
        {
            return true;
        }
    }
}

pub struct HookGuard(HHOOK);

impl Drop for HookGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = UnhookWindowsHookEx(self.0);
        }
    }
}
