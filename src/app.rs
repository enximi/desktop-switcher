use windows::core::Result;

use crate::{mouse_hook, tray};

pub fn run(edge_width_px: i32) -> Result<()> {
    let tray = tray::Tray::create()?;
    let hook = mouse_hook::install(edge_width_px, tray.command_target())?;

    let result = tray.run_message_loop();

    drop(hook);
    drop(tray);

    result
}
