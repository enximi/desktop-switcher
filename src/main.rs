#![cfg_attr(windows, windows_subsystem = "windows")]

mod config;
mod desktop_switch;
mod mouse_hook;
mod screen_edge;
mod startup;
mod tray;

const EDGE_WIDTH_PX: i32 = 4;

fn main() {
    if let Err(error) = run() {
        tray::show_startup_error(&format!("desktop-switcher 运行失败: {error}"));
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let settings = config::load_or_create()?;
    mouse_hook::apply_feature_settings(settings);

    let _hook = mouse_hook::install(EDGE_WIDTH_PX)?;
    tray::run()?;

    Ok(())
}
