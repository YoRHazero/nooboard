use clap::Parser;

mod app;
mod state;
mod ui;

fn main() {
    app::run(app::DesktopCli::parse());
}
