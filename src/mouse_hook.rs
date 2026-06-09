use std::{cell::RefCell, ffi::c_void, num::NonZeroUsize};

use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        UI::WindowsAndMessaging::{
            CallNextHookEx, HC_ACTION, HHOOK, MSLLHOOKSTRUCT, PostMessageW, SetWindowsHookExW,
            UnhookWindowsHookEx, WH_MOUSE_LL, WM_APP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEWHEEL,
        },
    },
    core::Result,
};

use crate::{config::FeatureSettings, screen_edge};

pub const COMMAND_MESSAGE: u32 = WM_APP + 1;

thread_local! {
    // The low-level mouse hook, tray menu commands, and message loop all run on
    // the same thread. Keeping the runtime thread-local avoids cross-thread
    // locking while still letting the hook callback decide synchronously whether
    // to pass or consume each mouse event.
    static ACTOR: RefCell<HookRuntime> = RefCell::new(HookRuntime::default());
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(usize)]
pub enum MouseCommand {
    SwitchLeft = 1,
    SwitchRight = 2,
    ShowTaskView = 3,
}

impl MouseCommand {
    pub fn from_message_value(value: usize) -> Option<Self> {
        match value {
            value if value == Self::SwitchLeft as usize => Some(Self::SwitchLeft),
            value if value == Self::SwitchRight as usize => Some(Self::SwitchRight),
            value if value == Self::ShowTaskView as usize => Some(Self::ShowTaskView),
            _ => None,
        }
    }

    fn message_value(self) -> usize {
        self as usize
    }
}

pub fn install(edge_width_px: i32, command_target: HWND) -> Result<HookGuard> {
    let hook = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), None, 0)? };

    ACTOR.with(|actor| {
        actor
            .borrow_mut()
            .install(edge_width_px, CommandTarget::from_hwnd(command_target));
    });

    Ok(HookGuard(hook))
}

pub fn is_edge_wheel_switching_enabled() -> bool {
    ACTOR.with(|actor| actor.borrow().edge_wheel_switching_enabled)
}

pub fn feature_settings() -> FeatureSettings {
    ACTOR.with(|actor| actor.borrow().feature_settings())
}

pub fn apply_feature_settings(settings: FeatureSettings) {
    ACTOR.with(|actor| actor.borrow_mut().apply_feature_settings(settings));
}

unsafe extern "system" fn mouse_proc(code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let raw_event = unsafe { &*(l_param.0 as *const MSLLHOOKSTRUCT) };

        if let Some(event) = MouseEvent::from_win32(w_param.0 as u32, raw_event) {
            let (decision, target) = ACTOR.with(|actor| actor.borrow_mut().handle_event(event));

            if let HookDecision::Command(command) = decision {
                post_command(target, command);
            }

            if decision.is_consumed() {
                return LRESULT(1);
            }
        }
    }

    unsafe { CallNextHookEx(None, code, w_param, l_param) }
}

fn post_command(target: Option<CommandTarget>, command: MouseCommand) {
    let Some(target) = target else {
        return;
    };

    unsafe {
        let _ = PostMessageW(
            Some(target.as_hwnd()),
            COMMAND_MESSAGE,
            WPARAM(command.message_value()),
            LPARAM(0),
        );
    }
}

#[derive(Debug)]
struct HookRuntime {
    edge_width_px: i32,
    command_target: Option<CommandTarget>,
    edge_wheel_switching_enabled: bool,
    gestures: GestureState,
}

impl Default for HookRuntime {
    fn default() -> Self {
        Self {
            edge_width_px: 4,
            command_target: None,
            edge_wheel_switching_enabled: FeatureSettings::default().edge_wheel_switching_enabled,
            gestures: GestureState::default(),
        }
    }
}

impl HookRuntime {
    fn install(&mut self, edge_width_px: i32, command_target: Option<CommandTarget>) {
        self.edge_width_px = edge_width_px;
        self.command_target = command_target;
        self.gestures = GestureState::default();
    }

    fn apply_feature_settings(&mut self, settings: FeatureSettings) {
        self.edge_wheel_switching_enabled = settings.edge_wheel_switching_enabled;
    }

    fn feature_settings(&self) -> FeatureSettings {
        FeatureSettings {
            edge_wheel_switching_enabled: self.edge_wheel_switching_enabled,
        }
    }

    fn handle_event(&mut self, event: MouseEvent) -> (HookDecision, Option<CommandTarget>) {
        let context = TriggerContext {
            is_at_left_edge: self.edge_wheel_switching_enabled
                && screen_edge::is_at_left_edge(event.point, self.edge_width_px),
        };

        let decision = self.gestures.handle(event.kind, context);
        (decision, self.command_target)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CommandTarget(NonZeroUsize);

impl CommandTarget {
    fn from_hwnd(hwnd: HWND) -> Option<Self> {
        NonZeroUsize::new(hwnd.0 as usize).map(Self)
    }

    fn as_hwnd(self) -> HWND {
        HWND(self.0.get() as *mut c_void)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MouseEvent {
    kind: MouseEventKind,
    point: screen_edge::Point,
}

impl MouseEvent {
    fn from_win32(message: u32, event: &MSLLHOOKSTRUCT) -> Option<Self> {
        let kind = match message {
            WM_MOUSEWHEEL => {
                let delta = high_word(event.mouseData) as i16;
                MouseEventKind::Wheel { delta }
            }
            WM_MBUTTONDOWN => MouseEventKind::MiddleButtonDown,
            WM_MBUTTONUP => MouseEventKind::MiddleButtonUp,
            _ => return None,
        };

        Some(Self {
            kind,
            point: event.pt.into(),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MouseEventKind {
    Wheel { delta: i16 },
    MiddleButtonDown,
    MiddleButtonUp,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct GestureState {
    middle_button_gesture_used: bool,
}

impl GestureState {
    fn handle(&mut self, event: MouseEventKind, context: TriggerContext) -> HookDecision {
        if event == MouseEventKind::MiddleButtonUp && self.middle_button_gesture_used {
            self.middle_button_gesture_used = false;
            return HookDecision::Consume;
        }

        if !context.is_at_left_edge {
            return HookDecision::Pass;
        }

        self.handle_trigger(event)
    }

    fn handle_trigger(&mut self, event: MouseEventKind) -> HookDecision {
        match event {
            MouseEventKind::Wheel { delta } => match delta.cmp(&0) {
                std::cmp::Ordering::Greater => HookDecision::Command(MouseCommand::SwitchLeft),
                std::cmp::Ordering::Less => HookDecision::Command(MouseCommand::SwitchRight),
                std::cmp::Ordering::Equal => HookDecision::Consume,
            },
            MouseEventKind::MiddleButtonDown => {
                self.middle_button_gesture_used = true;
                HookDecision::Command(MouseCommand::ShowTaskView)
            }
            MouseEventKind::MiddleButtonUp => {
                if self.middle_button_gesture_used {
                    self.middle_button_gesture_used = false;
                }

                HookDecision::Consume
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TriggerContext {
    is_at_left_edge: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HookDecision {
    Pass,
    Consume,
    Command(MouseCommand),
}

impl HookDecision {
    fn is_consumed(self) -> bool {
        matches!(self, Self::Consume | Self::Command(_))
    }
}

fn high_word(value: u32) -> u16 {
    ((value >> 16) & 0xffff) as u16
}

pub struct HookGuard(HHOOK);

impl Drop for HookGuard {
    fn drop(&mut self) {
        ACTOR.with(|actor| {
            actor.borrow_mut().command_target = None;
        });

        unsafe {
            let _ = UnhookWindowsHookEx(self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EDGE_CONTEXT: TriggerContext = TriggerContext {
        is_at_left_edge: true,
    };

    const OUTSIDE_CONTEXT: TriggerContext = TriggerContext {
        is_at_left_edge: false,
    };

    #[test]
    fn edge_wheel_maps_delta_to_desktop_commands() {
        let mut state = GestureState::default();

        assert_eq!(
            state.handle(MouseEventKind::Wheel { delta: 120 }, EDGE_CONTEXT),
            HookDecision::Command(MouseCommand::SwitchLeft)
        );
        assert_eq!(
            state.handle(MouseEventKind::Wheel { delta: -120 }, EDGE_CONTEXT),
            HookDecision::Command(MouseCommand::SwitchRight)
        );
    }

    #[test]
    fn middle_button_up_is_consumed_after_handled_middle_button_down() {
        let mut state = GestureState::default();

        assert_eq!(
            state.handle(MouseEventKind::MiddleButtonDown, EDGE_CONTEXT),
            HookDecision::Command(MouseCommand::ShowTaskView)
        );
        assert_eq!(
            state.handle(MouseEventKind::MiddleButtonUp, OUTSIDE_CONTEXT),
            HookDecision::Consume
        );
    }

    #[test]
    fn outside_trigger_area_passes_mouse_events_through() {
        let mut state = GestureState::default();

        assert_eq!(
            state.handle(MouseEventKind::Wheel { delta: 120 }, OUTSIDE_CONTEXT),
            HookDecision::Pass
        );
        assert_eq!(
            state.handle(MouseEventKind::MiddleButtonDown, OUTSIDE_CONTEXT),
            HookDecision::Pass
        );
    }
}
