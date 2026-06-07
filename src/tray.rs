use std::mem::size_of;

use crate::{config, mouse_hook, startup};

use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM},
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Shell::{
                NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_SETVERSION,
                NOTIFYICON_VERSION_4, NOTIFYICONDATAW, Shell_NotifyIconW,
            },
            WindowsAndMessaging::{
                AppendMenuW, CS_HREDRAW, CS_VREDRAW, CreatePopupMenu, CreateWindowExW,
                DefWindowProcW, DestroyMenu, DestroyWindow, DispatchMessageW, GetCursorPos,
                GetMessageW, HICON, HMENU, LoadIconW, MF_CHECKED, MF_SEPARATOR, MF_STRING,
                MF_UNCHECKED, MSG, MessageBoxW, PostMessageW, PostQuitMessage, RegisterClassW,
                SetForegroundWindow, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu,
                TranslateMessage, WINDOW_EX_STYLE, WM_CONTEXTMENU, WM_DESTROY, WM_NULL,
                WM_RBUTTONUP, WM_USER, WNDCLASSW, WS_OVERLAPPEDWINDOW,
            },
        },
    },
    core::{Error, HSTRING, PCWSTR, Result, w},
};

const WINDOW_CLASS_NAME: PCWSTR = w!("DesktopSwitcherTrayWindow");
const TRAY_TOOLTIP: &str = "desktop-switcher";
const APP_ICON_ID: u16 = 1;
const TRAY_ICON_ID: u32 = 1;
const MENU_EDGE_WHEEL_SWITCHING_ID: usize = 98;
const MENU_RIGHT_BUTTON_GESTURES_ID: usize = 97;
const MENU_STARTUP_ID: usize = 99;
const MENU_EXIT_ID: usize = 100;
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
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

struct TrayIcon {
    hwnd: HWND,
    icon: HICON,
}

impl TrayIcon {
    fn add(hwnd: HWND) -> Result<Self> {
        let instance = unsafe { GetModuleHandleW(None)? };
        let icon = unsafe { LoadIconW(Some(instance.into()), int_resource(APP_ICON_ID))? };

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

fn int_resource(id: u16) -> PCWSTR {
    PCWSTR(id as usize as *const u16)
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

fn show_context_menu(hwnd: HWND) -> Result<()> {
    let menu = PopupMenu::create()?;
    let mut cursor = POINT::default();

    unsafe {
        GetCursorPos(&mut cursor)?;
        let _ = SetForegroundWindow(hwnd);

        let selected = TrackPopupMenu(
            menu.handle,
            TPM_RIGHTBUTTON | TPM_RETURNCMD,
            cursor.x,
            cursor.y,
            None,
            hwnd,
            None,
        );
        let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));

        match selected.0 as usize {
            MENU_EDGE_WHEEL_SWITCHING_ID => toggle_edge_wheel_switching(),
            MENU_RIGHT_BUTTON_GESTURES_ID => toggle_right_button_gestures(),
            MENU_STARTUP_ID => toggle_startup()?,
            MENU_EXIT_ID => DestroyWindow(hwnd)?,
            _ => {}
        }
    }

    Ok(())
}

fn toggle_edge_wheel_switching() {
    let mut settings = mouse_hook::feature_settings();
    settings.edge_wheel_switching_enabled = !settings.edge_wheel_switching_enabled;
    save_feature_settings(settings);
}

fn toggle_right_button_gestures() {
    let mut settings = mouse_hook::feature_settings();
    settings.right_button_gestures_enabled = !settings.right_button_gestures_enabled;
    save_feature_settings(settings);
}

fn save_feature_settings(settings: config::FeatureSettings) {
    if let Err(error) = config::save(&settings) {
        show_startup_error(&format!("配置保存失败: {error}"));
        return;
    }

    mouse_hook::apply_feature_settings(settings);
}

fn toggle_startup() -> Result<()> {
    let result = startup::is_enabled()
        .and_then(|currently_enabled| startup::set_enabled(!currently_enabled));

    if let Err(error) = result {
        show_startup_error(&format!("开机自启设置失败: {error}"));
        return Err(error);
    }

    Ok(())
}

struct PopupMenu {
    handle: HMENU,
}

impl PopupMenu {
    fn create() -> Result<Self> {
        let handle = unsafe { CreatePopupMenu()? };
        let switching_enabled = mouse_hook::is_edge_wheel_switching_enabled();
        let switching_flags = if switching_enabled {
            MF_STRING | MF_CHECKED
        } else {
            MF_STRING | MF_UNCHECKED
        };
        let right_button_gestures_enabled = mouse_hook::is_right_button_gestures_enabled();
        let right_button_gestures_flags = if right_button_gestures_enabled {
            MF_STRING | MF_CHECKED
        } else {
            MF_STRING | MF_UNCHECKED
        };
        let startup_enabled = startup::is_enabled().unwrap_or(false);
        let startup_flags = if startup_enabled {
            MF_STRING | MF_CHECKED
        } else {
            MF_STRING | MF_UNCHECKED
        };

        unsafe {
            AppendMenuW(
                handle,
                switching_flags,
                MENU_EDGE_WHEEL_SWITCHING_ID,
                w!("边缘滚轮切换"),
            )?;
            AppendMenuW(
                handle,
                right_button_gestures_flags,
                MENU_RIGHT_BUTTON_GESTURES_ID,
                w!("按住右键触发"),
            )?;
            AppendMenuW(handle, startup_flags, MENU_STARTUP_ID, w!("开机自启"))?;
            AppendMenuW(handle, MF_SEPARATOR, 0, PCWSTR::null())?;
            AppendMenuW(handle, MF_STRING, MENU_EXIT_ID, w!("退出"))?;
        }

        Ok(Self { handle })
    }
}

impl Drop for PopupMenu {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyMenu(self.handle);
        }
    }
}

fn low_word(value: isize) -> u32 {
    (value as u32) & 0xffff
}

extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    match message {
        WM_TRAY_ICON
            if low_word(l_param.0) == WM_CONTEXTMENU || low_word(l_param.0) == WM_RBUTTONUP =>
        {
            let _ = show_context_menu(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            unsafe {
                PostQuitMessage(0);
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, message, w_param, l_param) },
    }
}
