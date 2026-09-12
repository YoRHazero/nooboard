//! JSON-lines control for reproducible, native two-host integration tests.
//! Clipboard text is never printed; only hashes, byte counts and public certificates.
use nooboard_core::{Endpoint, Event, PairRequest, Settings, diagnostics::Session};
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};

#[derive(Deserialize)]
struct Request {
    id: u64,
    #[serde(flatten)]
    command: Command,
}
#[derive(Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
enum Command {
    Pair {
        certificate: Vec<u8>,
        fingerprint: String,
        endpoint: Endpoint,
    },
    Settings {
        settings: Settings,
    },
    Copy {
        text: String,
    },
    Read,
    Send,
    Status,
    History,
    Events,
    Unpair,
    Quit,
}
fn digest(text: &str) -> Value {
    json!({"bytes": text.len(), "sha256": nooboard_network::fingerprint(text.as_bytes())})
}
async fn execute(
    session: &Session,
    events: &mut tokio::sync::broadcast::Receiver<Event>,
    command: Command,
) -> Result<Value, Box<dyn std::error::Error>> {
    let app = &session.app;
    Ok(match command {
        Command::Pair {
            certificate,
            fingerprint,
            endpoint,
        } => {
            app.pair(PairRequest {
                certificate,
                confirmed_fingerprint: fingerprint,
                endpoint,
            })
            .await?;
            Value::Null
        }
        Command::Settings { settings } => {
            app.set_settings(settings).await?;
            Value::Null
        }
        Command::Copy { text } => {
            session.copy(text).await?;
            Value::Null
        }
        Command::Read => session
            .read()
            .await?
            .as_deref()
            .map(digest)
            .unwrap_or(Value::Null),
        Command::Send => json!(app.send_current().await?),
        Command::Status => {
            let s = app.status();
            json!({"online": s.online, "peer_accepting": s.peer_accepting, "settings": s.settings})
        }
        Command::History => json!(
            app.history(String::new(), 100, 0)
                .await?
                .iter()
                .map(|e| json!({"text": digest(&e.text), "source": e.source}))
                .collect::<Vec<_>>()
        ),
        Command::Events => {
            let mut collected = Vec::new();
            loop {
                match events.try_recv() {
                    Ok(e) => collected.push(format!("{e:?}")),
                    Err(tokio::sync::broadcast::error::TryRecvError::Empty) => break,
                    Err(e) => return Err(e.into()),
                }
            }
            json!(collected)
        }
        Command::Unpair => {
            app.unpair().await?;
            Value::Null
        }
        Command::Quit => Value::Null,
    })
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let session = Session::start().await?;
    let mut events = session.app.subscribe();
    println!(
        "{}",
        json!({"ready":true, "certificate":session.app.certificate(), "fingerprint":session.app.status().fingerprint})
    );
    io::stdout().flush()?;
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
    while let Some(line) = lines.recv().await {
        let request: Request = serde_json::from_str(&line)?;
        let quit = matches!(request.command, Command::Quit);
        let response = match execute(&session, &mut events, request.command).await {
            Ok(value) => json!({"id": request.id, "ok":true,"result":value}),
            Err(error) => json!({"id":request.id,"ok":false,"error":error.to_string()}),
        };
        println!("{response}");
        io::stdout().flush()?;
        if quit {
            break;
        }
    }
    session.shutdown().await?;
    Ok(())
}
