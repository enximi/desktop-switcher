use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

use windows::{
    Win32::{
        Foundation::{LPARAM, LRESULT, WPARAM},
        UI::WindowsAndMessaging::{
            CallNextHookEx, HC_ACTION, HHOOK, MSLLHOOKSTRUCT, SetWindowsHookExW,
            UnhookWindowsHookEx, WH_MOUSE_LL, WM_MBUTTONDOWN, WM_MOUSEWHEEL, WM_RBUTTONDOWN,
            WM_RBUTTONUP,
        },
    },
    core::Result,
};

use crate::{
    config::FeatureSettings,
    desktop_switch::{self, Direction},
    screen_edge,
};

static EDGE_WIDTH_PX: AtomicI32 = AtomicI32::new(4);
static EDGE_WHEEL_SWITCHING_ENABLED: AtomicBool = AtomicBool::new(true);
static RIGHT_BUTTON_GESTURES_ENABLED: AtomicBool = AtomicBool::new(true);
static RIGHT_BUTTON_DOWN: AtomicBool = AtomicBool::new(false);
static RIGHT_BUTTON_GESTURE_USED: AtomicBool = AtomicBool::new(false);

pub fn install(edge_width_px: i32) -> Result<HookGuard> {
    EDGE_WIDTH_PX.store(edge_width_px, Ordering::Relaxed);

    let hook = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), None, 0)? };

    Ok(HookGuard(hook))
}

pub fn is_edge_wheel_switching_enabled() -> bool {
    EDGE_WHEEL_SWITCHING_ENABLED.load(Ordering::Relaxed)
}

pub fn set_edge_wheel_switching_enabled(enabled: bool) {
    EDGE_WHEEL_SWITCHING_ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn is_right_button_gestures_enabled() -> bool {
    RIGHT_BUTTON_GESTURES_ENABLED.load(Ordering::Relaxed)
}

pub fn set_right_button_gestures_enabled(enabled: bool) {
    RIGHT_BUTTON_GESTURES_ENABLED.store(enabled, Ordering::Relaxed);

    if !enabled {
        RIGHT_BUTTON_DOWN.store(false, Ordering::Relaxed);
        RIGHT_BUTTON_GESTURE_USED.store(false, Ordering::Relaxed);
    }
}

pub fn feature_settings() -> FeatureSettings {
    FeatureSettings {
        edge_wheel_switching_enabled: is_edge_wheel_switching_enabled(),
        right_button_gestures_enabled: is_right_button_gestures_enabled(),
    }
}

pub fn apply_feature_settings(settings: FeatureSettings) {
    set_edge_wheel_switching_enabled(settings.edge_wheel_switching_enabled);
    set_right_button_gestures_enabled(settings.right_button_gestures_enabled);
}

unsafe extern "system" fn mouse_proc(code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let event = unsafe { &*(l_param.0 as *const MSLLHOOKSTRUCT) };

        if handle_right_button_state(w_param.0 as u32) {
            return LRESULT(1);
        }

        if should_handle_right_button_gesture() && handle_trigger_event(w_param.0 as u32, event) {
            RIGHT_BUTTON_GESTURE_USED.store(true, Ordering::Relaxed);
            return LRESULT(1);
        }

        if is_edge_wheel_switching_enabled() {
            let edge_width_px = EDGE_WIDTH_PX.load(Ordering::Relaxed);

            if screen_edge::is_at_left_edge(event.pt, edge_width_px)
                && handle_trigger_event(w_param.0 as u32, event)
            {
                return LRESULT(1);
            }
        }
    }

    unsafe { CallNextHookEx(None, code, w_param, l_param) }
}

fn handle_right_button_state(message: u32) -> bool {
    if !is_right_button_gestures_enabled() {
        return false;
    }

    match message {
        WM_RBUTTONDOWN => {
            RIGHT_BUTTON_DOWN.store(true, Ordering::Relaxed);
            RIGHT_BUTTON_GESTURE_USED.store(false, Ordering::Relaxed);
            false
        }
        WM_RBUTTONUP => {
            RIGHT_BUTTON_DOWN.store(false, Ordering::Relaxed);

            if RIGHT_BUTTON_GESTURE_USED.swap(false, Ordering::Relaxed) {
                return true;
            }

            false
        }
        _ => false,
    }
}

fn should_handle_right_button_gesture() -> bool {
    is_right_button_gestures_enabled() && RIGHT_BUTTON_DOWN.load(Ordering::Relaxed)
}

fn handle_trigger_event(message: u32, event: &MSLLHOOKSTRUCT) -> bool {
    match message {
        WM_MOUSEWHEEL => {
            let wheel_delta = high_word(event.mouseData) as i16;
            if wheel_delta == 0 {
                return true;
            }

            let direction = if wheel_delta > 0 {
                Direction::Left
            } else {
                Direction::Right
            };

            if let Err(error) = desktop_switch::switch_desktop(direction) {
                eprintln!("切换虚拟桌面失败: {error}");
            }

            true
        }
        WM_MBUTTONDOWN => {
            if let Err(error) = desktop_switch::show_task_view() {
                eprintln!("打开多任务视图失败: {error}");
            }

            true
        }
        _ => false,
    }
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
