#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod config;
mod desktop_switch;
mod error_dialog;
mod mouse_hook;
mod screen_edge;
mod startup;
mod tray;

const EDGE_WIDTH_PX: i32 = 4;

fn main() {
    if let Err(error) = run() {
        error_dialog::show(&format!("desktop-switcher 运行失败: {error}"));
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let settings = config::load_or_create()?;
    mouse_hook::apply_feature_settings(settings);

    app::run(EDGE_WIDTH_PX)?;

    Ok(())
}
