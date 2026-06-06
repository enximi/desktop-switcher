use std::mem::size_of;

use windows::{
    Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput,
        VIRTUAL_KEY, VK_CONTROL, VK_LEFT, VK_LWIN, VK_RIGHT, VK_TAB,
    },
    core::{Error, Result},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    Left,
    Right,
}

pub fn switch_desktop(direction: Direction) -> Result<()> {
    let arrow_key = match direction {
        Direction::Left => VK_LEFT,
        Direction::Right => VK_RIGHT,
    };

    send_key_sequence(&[
        key_down(VK_CONTROL),
        key_down(VK_LWIN),
        key_down(arrow_key),
        key_up(arrow_key),
        key_up(VK_LWIN),
        key_up(VK_CONTROL),
    ])
}

pub fn show_task_view() -> Result<()> {
    send_key_sequence(&[
        key_down(VK_LWIN),
        key_down(VK_TAB),
        key_up(VK_TAB),
        key_up(VK_LWIN),
    ])
}

fn send_key_sequence(inputs: &[INPUT]) -> Result<()> {
    let sent = unsafe { SendInput(inputs, size_of::<INPUT>() as i32) };

    if sent == inputs.len() as u32 {
        Ok(())
    } else {
        Err(Error::from_thread())
    }
}

fn key_down(key: VIRTUAL_KEY) -> INPUT {
    keyboard_input(key, KEYBD_EVENT_FLAGS(0))
}

fn key_up(key: VIRTUAL_KEY) -> INPUT {
    keyboard_input(key, KEYEVENTF_KEYUP)
}

fn keyboard_input(key: VIRTUAL_KEY, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}
