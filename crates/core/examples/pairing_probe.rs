//! Interactive, isolated macOS pairing peer for native UI verification.
//! The receiving peer displays its one-time code here, just as the app does locally.
#[cfg(all(feature = "diagnostics", target_os = "macos"))]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use nooboard_core::{PairingStage, Settings, diagnostics::Session};
    use std::time::Duration;
    let session = Session::with_settings(Settings {
        device_name: "配对验证肥啾".into(),
        discoverable: true,
        listen_address: "0.0.0.0:0".into(),
        pairing_listen_address: "0.0.0.0:0".into(),
        ..Settings::default()
    })
    .await?;
    session.app.refresh_discovery().await?;
    println!(
        "配对地址 {}",
        session.app.snapshot().onboarding.pairing_address
    );
    let target = std::env::args().nth(1);
    if let Some(target) = &target {
        session.app.begin_pairing(target.clone(), None).await?;
    }
    let mut previous = None;
    let mut complete_at = None;
    let started = std::time::Instant::now();
    while started.elapsed() < Duration::from_secs(300) {
        if let Some(pairing) = session.app.snapshot().onboarding.session
            && previous != Some(pairing.stage)
        {
            previous = Some(pairing.stage);
            println!("{}", serde_json::to_string(&pairing.stage)?);
            match pairing.stage {
                PairingStage::AwaitingApproval if pairing.incoming => {
                    session.app.accept_pairing(pairing.id).await?
                }
                PairingStage::ShowingCode => {
                    println!("本次配对码 {}", pairing.code.unwrap_or_default())
                }
                PairingStage::EnteringCode => {
                    println!("请输入 app 中显示的配对码：");
                    let code = tokio::task::spawn_blocking(|| {
                        let mut line = String::new();
                        std::io::stdin()
                            .read_line(&mut line)
                            .map(|_| line.trim().to_owned())
                    })
                    .await??;
                    session.app.submit_pairing_code(pairing.id, code).await?;
                }
                PairingStage::Completed => {
                    complete_at = Some(std::time::Instant::now());
                }
                PairingStage::Failed => {
                    println!("{:?}", pairing.error);
                    break;
                }
                _ => {}
            }
        }
        if complete_at.is_some_and(|at| at.elapsed() > Duration::from_secs(60)) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    session.shutdown().await?;
    Ok(())
}
#[cfg(not(all(feature = "diagnostics", target_os = "macos")))]
fn main() {
    eprintln!("Run on macOS with --features diagnostics; uses a private test pasteboard.");
}
