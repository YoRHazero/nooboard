use std::io::{self, Write};
use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use nooboard_config::{ConfigTemplate, resolve_init_output_path, write_config_template};

#[derive(Debug, Parser)]
#[command(name = "nooboard-config")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Init(InitCommand),
}

#[derive(Debug, Parser)]
struct InitCommand {
    #[arg(long, value_enum)]
    profile: InitProfile,
    #[arg(long)]
    output: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum InitProfile {
    Production,
    Development,
}

impl From<InitProfile> for ConfigTemplate {
    fn from(value: InitProfile) -> Self {
        match value {
            InitProfile::Production => Self::Production,
            InitProfile::Development => Self::Development,
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Command::Init(command) => run_init(command)?,
    }

    Ok(())
}

fn run_init(command: InitCommand) -> Result<(), Box<dyn std::error::Error>> {
    let cwd = std::env::current_dir()?;
    let output_path = resolve_init_output_path(command.output.as_deref(), &cwd);
    let target_exists = output_path.exists();

    if command.output.is_none() {
        let prompt = if target_exists {
            format!(
                "No output path provided. Will overwrite config at {}. Continue? [y/N]: ",
                output_path.display()
            )
        } else {
            format!(
                "No output path provided. Will create config at {}. Continue? [y/N]: ",
                output_path.display()
            )
        };

        if !prompt_yes_no(&prompt)? {
            println!("Aborted.");
            return Ok(());
        }
    } else if target_exists {
        let prompt = format!(
            "Config already exists at {}. Overwrite? [y/N]: ",
            output_path.display()
        );
        if !prompt_yes_no(&prompt)? {
            println!("Aborted.");
            return Ok(());
        }
    }

    write_config_template(&output_path, command.profile.into())?;
    println!("Wrote config to {}", output_path.display());
    Ok(())
}

fn prompt_yes_no(prompt: &str) -> io::Result<bool> {
    print!("{prompt}");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let normalized = input.trim().to_ascii_lowercase();
    Ok(matches!(normalized.as_str(), "y" | "yes"))
}
