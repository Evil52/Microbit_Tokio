//! Прошивка платы через probe-rs (вызов CLI как subprocess).
//! Координация с serial-ридером: на время flash тот закрывает порт (FlashGate).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::process::Command;
use tokio::sync::Notify;

/// Путь к ELF firmware (собирается отдельно `cargo build` из firmware/).
const FIRMWARE_ELF: &str = "target/thumbv7em-none-eabihf/debug/firmware";
const CHIP: &str = "nRF52833_xxAA";

/// Координатор flash↔serial. serial-ридер уважает `flashing`, flash дёргает `resume`.
#[derive(Clone)]
pub struct FlashGate {
    pub flashing: Arc<AtomicBool>,
    pub resume: Arc<Notify>,
}

impl FlashGate {
    pub fn new() -> Self {
        Self {
            flashing: Arc::new(AtomicBool::new(false)),
            resume: Arc::new(Notify::new()),
        }
    }
}

/// Прошить плату. Ставит флаг (serial закроет порт), запускает probe-rs, снимает флаг.
pub async fn flash(gate: &FlashGate) -> Result<String, String> {
    gate.flashing.store(true, Ordering::SeqCst);

    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let result = run_probe_rs().await;

    gate.flashing.store(false, Ordering::SeqCst);
    gate.resume.notify_waiters();

    result
}

async fn run_probe_rs() -> Result<String, String> {
    let dl = Command::new("probe-rs")
        .args(["download", "--chip", CHIP, FIRMWARE_ELF])
        .output()
        .await
        .map_err(|e| format!("spawn probe-rs failed: {e}"))?;
    if !dl.status.success() {
        return Err(format!(
            "download failed: {}",
            String::from_utf8_lossy(&dl.stderr)
        ));
    }

    let rs = Command::new("probe-rs")
        .args(["reset", "--chip", CHIP])
        .output()
        .await
        .map_err(|e| format!("spawn reset failed: {e}"))?;
    if !rs.status.success() {
        return Err(format!(
            "reset failed: {}",
            String::from_utf8_lossy(&rs.stderr)
        ));
    }

    Ok("flashed + reset OK".to_string())
}
