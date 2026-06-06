use std::mem::size_of;

use windows::Win32::{
    Foundation::{POINT, RECT},
    Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint},
};

pub fn is_at_left_edge(point: POINT, width_px: i32) -> bool {
    if width_px <= 0 {
        return false;
    }

    let Some(monitor) = monitor_bounds(point) else {
        return false;
    };

    point.x >= monitor.left
        && point.x < monitor.left.saturating_add(width_px)
        && point.y >= monitor.top
        && point.y < monitor.bottom
}

fn monitor_bounds(point: POINT) -> Option<RECT> {
    let monitor = unsafe { MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST) };
    if monitor.is_invalid() {
        return None;
    }

    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        rcMonitor: RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        },
        rcWork: RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        },
        dwFlags: 0,
    };

    let ok = unsafe { GetMonitorInfoW(monitor, &mut info) };
    if ok.as_bool() {
        Some(info.rcMonitor)
    } else {
        None
    }
}
