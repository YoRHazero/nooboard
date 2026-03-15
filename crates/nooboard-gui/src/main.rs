#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use clap::Parser;

mod app;
mod bootstrap;
mod ui;
mod workspace;

fn main() {
    app::run(app::GuiCli::parse());
}
