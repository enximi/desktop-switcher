use std::mem::size_of;

use crate::{config, desktop_switch, error_dialog, mouse_hook, startup};

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
                MF_UNCHECKED, MSG, PostMessageW, PostQuitMessage, RegisterClassW,
                SetForegroundWindow, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu,
                TranslateMessage, WINDOW_EX_STYLE, WM_CONTEXTMENU, WM_DESTROY, WM_NULL,
                WM_RBUTTONUP, WM_USER, WNDCLASSW, WS_OVERLAPPEDWINDOW,
            },
        },
    },
    core::{Error, PCWSTR, Result, w},
};

const WINDOW_CLASS_NAME: PCWSTR = w!("DesktopSwitcherTrayWindow");
const TRAY_TOOLTIP: &str = "desktop-switcher";
const APP_ICON_ID: u16 = 1;
const TRAY_ICON_ID: u32 = 1;
const MENU_EDGE_WHEEL_SWITCHING_ID: usize = 98;
const MENU_STARTUP_ID: usize = 99;
const MENU_EXIT_ID: usize = 100;
const WM_TRAY_ICON: u32 = WM_USER + 1;

pub struct Tray {
    _icon: TrayIcon,
    window: HiddenWindow,
}

impl Tray {
    pub fn create() -> Result<Self> {
        let window = HiddenWindow::create()?;
        let icon = TrayIcon::add(window.hwnd)?;

        Ok(Self {
            _icon: icon,
            window,
        })
    }

    pub fn command_target(&self) -> HWND {
        self.window.hwnd
    }

    pub fn run_message_loop(&self) -> Result<()> {
        run_message_loop()
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
    if let Some(command) = menu.track_at_cursor(hwnd)? {
        handle_menu_command(command, hwnd)?;
    }

    Ok(())
}

fn handle_menu_command(command: MenuCommand, hwnd: HWND) -> Result<()> {
    match command {
        MenuCommand::ToggleEdgeWheelSwitching => toggle_edge_wheel_switching(),
        MenuCommand::ToggleStartup => toggle_startup()?,
        MenuCommand::Exit => unsafe {
            DestroyWindow(hwnd)?;
        },
    }

    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MenuCommand {
    ToggleEdgeWheelSwitching,
    ToggleStartup,
    Exit,
}

impl MenuCommand {
    fn from_id(id: usize) -> Option<Self> {
        match id {
            MENU_EDGE_WHEEL_SWITCHING_ID => Some(Self::ToggleEdgeWheelSwitching),
            MENU_STARTUP_ID => Some(Self::ToggleStartup),
            MENU_EXIT_ID => Some(Self::Exit),
            _ => None,
        }
    }
}

fn current_cursor_position() -> Result<POINT> {
    let mut cursor = POINT::default();

    unsafe {
        GetCursorPos(&mut cursor)?;
    }

    Ok(cursor)
}

fn track_menu(handle: HMENU, hwnd: HWND, cursor: POINT) -> Option<MenuCommand> {
    unsafe {
        let selected = TrackPopupMenu(
            handle,
            TPM_RIGHTBUTTON | TPM_RETURNCMD,
            cursor.x,
            cursor.y,
            None,
            hwnd,
            None,
        );
        let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));

        MenuCommand::from_id(selected.0 as usize)
    }
}

fn toggle_edge_wheel_switching() {
    let mut settings = mouse_hook::feature_settings();
    settings.edge_wheel_switching_enabled = !settings.edge_wheel_switching_enabled;
    save_feature_settings(settings);
}

fn save_feature_settings(settings: config::FeatureSettings) {
    if let Err(error) = config::save(&settings) {
        error_dialog::show(&format!("配置保存失败: {error}"));
        return;
    }

    mouse_hook::apply_feature_settings(settings);
}

fn toggle_startup() -> Result<()> {
    let result = startup::is_enabled()
        .and_then(|currently_enabled| startup::set_enabled(!currently_enabled));

    if let Err(error) = result {
        error_dialog::show(&format!("开机自启设置失败: {error}"));
        return Err(error);
    }

    Ok(())
}

fn handle_mouse_command(w_param: WPARAM) {
    let Some(command) = mouse_hook::MouseCommand::from_message_value(w_param.0) else {
        return;
    };

    let result = match command {
        mouse_hook::MouseCommand::SwitchLeft => {
            desktop_switch::switch_desktop(desktop_switch::Direction::Left)
        }
        mouse_hook::MouseCommand::SwitchRight => {
            desktop_switch::switch_desktop(desktop_switch::Direction::Right)
        }
        mouse_hook::MouseCommand::ShowTaskView => desktop_switch::show_task_view(),
    };

    if let Err(error) = result {
        eprintln!("鼠标命令执行失败: {error}");
    }
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
            AppendMenuW(handle, startup_flags, MENU_STARTUP_ID, w!("开机自启"))?;
            AppendMenuW(handle, MF_SEPARATOR, 0, PCWSTR::null())?;
            AppendMenuW(handle, MF_STRING, MENU_EXIT_ID, w!("退出"))?;
        }

        Ok(Self { handle })
    }

    fn track_at_cursor(&self, hwnd: HWND) -> Result<Option<MenuCommand>> {
        let cursor = current_cursor_position()?;

        unsafe {
            let _ = SetForegroundWindow(hwnd);
        }

        Ok(track_menu(self.handle, hwnd, cursor))
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
        mouse_hook::COMMAND_MESSAGE => {
            handle_mouse_command(w_param);
            LRESULT(0)
        }
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
