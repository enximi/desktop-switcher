use std::mem::size_of;

use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Shell::{
                NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_SETVERSION,
                NOTIFYICON_VERSION_4, NOTIFYICONDATAW, Shell_NotifyIconW,
            },
            WindowsAndMessaging::{
                CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DispatchMessageW,
                GetMessageW, HICON, IDI_APPLICATION, LoadIconW, MSG, MessageBoxW, PostQuitMessage,
                RegisterClassW, TranslateMessage, WINDOW_EX_STYLE, WM_DESTROY, WM_USER, WNDCLASSW,
                WS_OVERLAPPEDWINDOW,
            },
        },
    },
    core::{Error, HSTRING, PCWSTR, Result, w},
};

const WINDOW_CLASS_NAME: PCWSTR = w!("DesktopSwitcherTrayWindow");
const TRAY_TOOLTIP: &str = "desktop-switcher";
const TRAY_ICON_ID: u32 = 1;
const WM_TRAY_ICON: u32 = WM_USER + 1;

pub fn run() -> Result<()> {
    let window = HiddenWindow::create()?;
    let tray_icon = TrayIcon::add(window.hwnd)?;

    let result = run_message_loop();

    drop(tray_icon);
    drop(window);

    result
}

pub fn show_startup_error(message: &str) {
    let message = HSTRING::from(message);

    unsafe {
        MessageBoxW(
            None,
            &message,
            w!("desktop-switcher"),
            windows::Win32::UI::WindowsAndMessaging::MB_ICONERROR,
        );
    }
}

fn run_message_loop() -> Result<()> {
    let mut message = MSG::default();

    loop {
        let result = unsafe { GetMessageW(&mut message, None, 0, 0) };
        if result.0 == -1 {
            return Err(Error::from_thread());
        }

        if result.0 == 0 {
            return Ok(());
        }

        unsafe {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

struct HiddenWindow {
    hwnd: HWND,
}

impl HiddenWindow {
    fn create() -> Result<Self> {
        let instance = unsafe { GetModuleHandleW(None)? };
        let window_class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            hInstance: instance.into(),
            lpszClassName: WINDOW_CLASS_NAME,
            ..Default::default()
        };

        let class_atom = unsafe { RegisterClassW(&window_class) };
        if class_atom == 0 {
            return Err(Error::from_thread());
        }

        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                WINDOW_CLASS_NAME,
                w!("desktop-switcher"),
                WS_OVERLAPPEDWINDOW,
                0,
                0,
                0,
                0,
                None,
                None,
                Some(instance.into()),
                None,
            )?
        };

        Ok(Self { hwnd })
    }
}

impl Drop for HiddenWindow {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::DestroyWindow(self.hwnd);
        }
    }
}

struct TrayIcon {
    hwnd: HWND,
    icon: HICON,
}

impl TrayIcon {
    fn add(hwnd: HWND) -> Result<Self> {
        let icon = unsafe { LoadIconW(None, IDI_APPLICATION)? };

        let mut data = tray_icon_data(hwnd, icon);
        set_tip(&mut data, TRAY_TOOLTIP);

        let added = unsafe { Shell_NotifyIconW(NIM_ADD, &data) };
        if !added.as_bool() {
            return Err(Error::from_thread());
        }

        data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
        let version_set = unsafe { Shell_NotifyIconW(NIM_SETVERSION, &data) };
        if !version_set.as_bool() {
            unsafe {
                let _ = Shell_NotifyIconW(NIM_DELETE, &data);
            }

            return Err(Error::from_thread());
        }

        Ok(Self { hwnd, icon })
    }
}

impl Drop for TrayIcon {
    fn drop(&mut self) {
        let data = tray_icon_data(self.hwnd, self.icon);

        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &data);
        }
    }
}

fn tray_icon_data(hwnd: HWND, icon: HICON) -> NOTIFYICONDATAW {
    NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_ICON_ID,
        uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
        uCallbackMessage: WM_TRAY_ICON,
        hIcon: icon,
        ..Default::default()
    }
}

fn set_tip(data: &mut NOTIFYICONDATAW, tip: &str) {
    let encoded = tip.encode_utf16();
    for (target, source) in data.szTip.iter_mut().zip(encoded) {
        *target = source;
    }
}

extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    match message {
        WM_DESTROY => {
            unsafe {
                PostQuitMessage(0);
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, message, w_param, l_param) },
    }
}
