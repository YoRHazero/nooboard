//! JSON-lines control for isolated native integration tests. No clipboard bodies in output.
use nooboard_core::{
    Event, PeerSettings, Settings,
    diagnostics::{PeerFixture, Session, trust_peer},
};
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
    TrustPeer {
        noob_id: String,
        certificate: Vec<u8>,
        fingerprint: String,
        device_name: String,
        address: Option<String>,
    },
    Listen {
        address: String,
    },
    ConfigurePeer {
        noob_id: String,
        settings: PeerSettings,
    },
    Targets {
        targets: Vec<String>,
    },
    Settings {
        settings: Settings,
    },
    Copy {
        text: String,
    },
    Read,
    Send {
        targets: Option<Vec<String>>,
    },
    Status,
    History,
    Events,
    Unpair {
        noob_id: String,
    },
    Quit,
}
fn digest_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}
fn digest(text: &str) -> Value {
    json!({"bytes": text.len(), "sha256": digest_hex(text.as_bytes())})
}
async fn execute(
    session: &Session,
    events: &mut tokio::sync::broadcast::Receiver<Event>,
    command: Command,
) -> Result<Value, Box<dyn std::error::Error>> {
    let app = &session.app;
    Ok(match command {
        Command::TrustPeer {
            noob_id,
            certificate,
            fingerprint,
            device_name,
            address,
        } => json!(
            trust_peer(
                app,
                PeerFixture {
                    noob_id,
                    certificate,
                    confirmed_fingerprint: fingerprint,
                    device_name,
                    address
                }
            )
            .await?
        ),
        Command::Listen { address } => {
            let _ = address;
            return Err("pass the listen address when starting link_probe".into());
        }
        Command::ConfigurePeer { noob_id, settings } => {
            app.configure_peer(noob_id, settings).await?;
            Value::Null
        }
        Command::Targets { targets } => {
            app.select_targets(targets).await?;
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
        Command::Send { targets } => json!(match targets {
            Some(targets) => app.send_to(targets).await?,
            None => app.send_current().await?,
        }),
        Command::Status => json!(app.status()),
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
                    Ok(e) => collected.push(e),
                    Err(tokio::sync::broadcast::error::TryRecvError::Empty) => break,
                    Err(e) => return Err(e.into()),
                }
            }
            json!(collected)
        }
        Command::Unpair { noob_id } => {
            app.unpair(noob_id).await?;
            Value::Null
        }
        Command::Quit => Value::Null,
    })
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let session = Session::with_settings(Settings {
        listen_address: std::env::args()
            .nth(1)
            .unwrap_or_else(|| "127.0.0.1:0".into()),
        pairing_listen_address: "127.0.0.1:0".into(),
        discoverable: false,
        ..Settings::default()
    })
    .await?;
    let mut events = session.app.subscribe_events();
    println!(
        "{}",
        json!({"ready":true, "certificate":session.app.certificate(), "fingerprint":session.app.status().fingerprint, "noob_id":session.app.status().noob_id, "device_name":session.app.status().settings.device_name, "listen_address":session.app.status().listen_address})
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
