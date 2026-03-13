#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use clap::Parser;

mod app;
mod state;
mod ui;

fn main() {
    app::run(app::DesktopCli::parse());
}
