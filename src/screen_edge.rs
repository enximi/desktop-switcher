use std::mem::size_of;

use windows::Win32::{
    Foundation::{POINT, RECT},
    Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl From<POINT> for Point {
    fn from(point: POINT) -> Self {
        Self {
            x: point.x,
            y: point.y,
        }
    }
}

impl From<Point> for POINT {
    fn from(point: Point) -> Self {
        Self {
            x: point.x,
            y: point.y,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

impl Rect {
    fn contains_left_edge(self, point: Point, width_px: i32) -> bool {
        width_px > 0
            && point.x >= self.left
            && point.x < self.left.saturating_add(width_px)
            && point.y >= self.top
            && point.y < self.bottom
    }
}

impl From<RECT> for Rect {
    fn from(rect: RECT) -> Self {
        Self {
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: rect.bottom,
        }
    }
}

pub fn is_at_left_edge(point: Point, width_px: i32) -> bool {
    let Some(monitor) = monitor_bounds(point) else {
        return false;
    };

    monitor.contains_left_edge(point, width_px)
}

fn monitor_bounds(point: Point) -> Option<Rect> {
    let monitor = unsafe { MonitorFromPoint(point.into(), MONITOR_DEFAULTTONEAREST) };
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
        Some(info.rcMonitor.into())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MONITOR: Rect = Rect {
        left: 100,
        top: 50,
        right: 500,
        bottom: 350,
    };

    #[test]
    fn left_edge_contains_points_inside_configured_width() {
        assert!(MONITOR.contains_left_edge(Point { x: 100, y: 50 }, 4));
        assert!(MONITOR.contains_left_edge(Point { x: 103, y: 349 }, 4));
    }

    #[test]
    fn left_edge_rejects_points_outside_width_or_vertical_bounds() {
        assert!(!MONITOR.contains_left_edge(Point { x: 104, y: 100 }, 4));
        assert!(!MONITOR.contains_left_edge(Point { x: 99, y: 100 }, 4));
        assert!(!MONITOR.contains_left_edge(Point { x: 100, y: 350 }, 4));
        assert!(!MONITOR.contains_left_edge(Point { x: 100, y: 49 }, 4));
    }

    #[test]
    fn left_edge_rejects_non_positive_width() {
        assert!(!MONITOR.contains_left_edge(Point { x: 100, y: 100 }, 0));
        assert!(!MONITOR.contains_left_edge(Point { x: 100, y: 100 }, -1));
    }
}
