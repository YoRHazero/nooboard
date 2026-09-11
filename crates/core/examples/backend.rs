//! Minimal API harness. Pairing and synchronization rules remain in nooboard-core.
use nooboard_core::{App, Endpoint, Mode, Options, PairRequest};
use std::{
    error::Error,
    io::{self, BufRead},
    path::PathBuf,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 2 || args[0] == "--help" {
        println!("Usage: backend <database-path> <profile-name>");
        println!(
            "Commands: status | export <certificate-path> | pair <certificate-path> <fingerprint> <listen|connect> <address>"
        );
        println!(
            "auto | manual | pause | resume | receive on/off | history on/off | send | list [query] | copy <id> | delete <id> | clear | unpair | quit"
        );
        return Ok(());
    }
    let app = App::start(Options {
        database: PathBuf::from(&args[0]),
        profile: args[1].clone(),
    })
    .await?;
    println!("Identity fingerprint: {}", app.status().fingerprint);
    println!(
        "Ready. Type status or quit. Clipboard bodies are not logged; list explicitly displays history."
    );
    let mut events = app.subscribe();
    let (input, mut lines) = tokio::sync::mpsc::channel(16);
    std::thread::spawn(move || {
        for line in io::stdin().lock().lines() {
            match line {
                Ok(line) => {
                    if input.blocking_send(line).is_err() {
                        break;
                    }
                }
                _ => break,
            }
        }
    });
    loop {
        tokio::select! {
            line = lines.recv() => {
                let Some(line) = line else { break };
                if line.trim() == "quit" { break; }
                if let Err(error) = execute(&app, &line).await { eprintln!("{error}"); }
            }
            event = events.recv() => match event {
                Ok(event) => println!("{event:?}"),
                Err(tokio::sync::broadcast::error::RecvError::Lagged(count)) => eprintln!("Skipped {count} events; use status to refresh."),
                Err(_) => break,
            }
        }
    }
    app.shutdown().await?;
    Ok(())
}
async fn execute(app: &App, line: &str) -> Result<(), Box<dyn Error>> {
    let words: Vec<&str> = line.split_whitespace().collect();
    let mut settings = app.status().settings;
    match words.as_slice() {
        ["status"] => println!("{:?}", app.status()),
        ["export", path] => std::fs::write(path, app.certificate())?,
        ["pair", path, fingerprint, role, address] => {
            let endpoint = match *role {
                "listen" => Endpoint::Listen((*address).into()),
                "connect" => Endpoint::Connect((*address).into()),
                _ => return Err("role must be listen or connect".into()),
            };
            app.pair(PairRequest {
                certificate: std::fs::read(path)?,
                confirmed_fingerprint: (*fingerprint).into(),
                endpoint,
            })
            .await?;
        }
        ["auto"] => {
            settings.mode = Mode::Automatic;
            app.set_settings(settings).await?;
        }
        ["manual"] => {
            settings.mode = Mode::Manual;
            app.set_settings(settings).await?;
        }
        ["pause"] => {
            settings.paused = true;
            app.set_settings(settings).await?;
        }
        ["resume"] => {
            settings.paused = false;
            app.set_settings(settings).await?;
        }
        ["receive", value @ ("on" | "off")] => {
            settings.receive = *value == "on";
            app.set_settings(settings).await?;
        }
        ["history", value @ ("on" | "off")] => {
            settings.history = *value == "on";
            app.set_settings(settings).await?;
        }
        ["send"] => println!("Transport wrote sequence {}", app.send_current().await?),
        ["list", ..] => {
            for entry in app.history(words[1..].join(" "), 100, 0).await? {
                println!("{} [{}] {:?}", entry.id, entry.source, entry.text);
            }
        }
        ["copy", id] => app.copy_history(id.parse()?).await?,
        ["delete", id] => app.delete_history(id.parse()?).await?,
        ["clear"] => app.clear_history().await?,
        ["unpair"] => app.unpair().await?,
        [] => {}
        _ => return Err("unknown command; run with --help for usage".into()),
    }
    Ok(())
}
