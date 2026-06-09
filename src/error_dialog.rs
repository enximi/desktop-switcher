use windows::{
    Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MessageBoxW},
    core::{HSTRING, w},
};

pub fn show(message: &str) {
    let message = HSTRING::from(message);

    unsafe {
        MessageBoxW(None, &message, w!("desktop-switcher"), MB_ICONERROR);
    }
}
