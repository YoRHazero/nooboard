//! Minimal API harness. Pairing and synchronization rules remain in nooboard-core.
use nooboard_core::{App, Mode, Options};
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
            "Commands: status | discover | pairing | pair <address> | accept <session-id> | code <session-id> <8-digits> | cancel <session-id> | listen <address> | pair-listen <address> | name <device-name>"
        );
        println!(
            "auto | manual | pause | resume | receive on/off | history on/off | targets <noob-id>... | send [noob-id...] | peer-auto <noob-id> on/off | list [query] | copy <id> | delete <id> | clear | unpair <noob-id> | quit"
        );
        return Ok(());
    }
    let app = App::start(Options {
        database: PathBuf::from(&args[0]),
        profile: args[1].clone(),
        default_receive_directory: None,
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
        ["discover"] => {
            app.refresh_discovery().await?;
        }
        ["pairing"] => println!(
            "{}",
            serde_json::to_string_pretty(&app.snapshot().onboarding)?
        ),
        ["pair", address] => app.begin_pairing((*address).into(), None).await?,
        ["accept", id] => app.accept_pairing((*id).into()).await?,
        ["code", id, code] => {
            app.submit_pairing_code((*id).into(), (*code).into())
                .await?
        }
        ["cancel", id] => app.dismiss_pairing((*id).into()).await?,
        ["pair-listen", address] => {
            settings.pairing_listen_address = (*address).into();
            app.set_settings(settings).await?;
        }
        ["listen", address] => {
            settings.listen_address = (*address).into();
            app.set_settings(settings).await?;
        }
        ["name", ..] => {
            settings.device_name = words[1..].join(" ");
            app.set_settings(settings).await?;
        }
        ["targets", ..] => {
            app.select_targets(words[1..].iter().map(|s| (*s).into()).collect())
                .await?
        }
        ["peer-auto", id, value @ ("on" | "off")] => {
            let mut peer = app
                .status()
                .peers
                .into_iter()
                .find(|p| p.noob_id == *id)
                .ok_or("unknown peer")?;
            peer.settings.auto_send = *value == "on";
            app.configure_peer((*id).into(), peer.settings).await?;
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
        ["send"] => println!("Queued batch {:?}", app.send_current().await?),
        ["send", ..] => println!(
            "Queued batch {:?}",
            app.send_to(words[1..].iter().map(|s| (*s).into()).collect())
                .await?
        ),
        ["list", ..] => {
            for entry in app.history(words[1..].join(" "), 100, 0).await? {
                println!("{} [{}] {:?}", entry.id, entry.source, entry.text);
            }
        }
        ["copy", id] => app.copy_history(id.parse()?).await?,
        ["delete", id] => app.delete_history(id.parse()?).await?,
        ["clear"] => app.clear_history().await?,
        ["unpair", id] => app.unpair((*id).into()).await?,
        [] => {}
        _ => return Err("unknown command; run with --help for usage".into()),
    }
    Ok(())
}
