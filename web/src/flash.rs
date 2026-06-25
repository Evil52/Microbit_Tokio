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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn new_gate_is_not_flashing() {
        let gate = FlashGate::new();
        assert!(!gate.flashing.load(Ordering::SeqCst));
    }

    #[test]
    fn clone_shares_state() {
        // Клон должен видеть тот же флаг — serial-ридер и flash держат клоны.
        let gate = FlashGate::new();
        let clone = gate.clone();
        gate.flashing.store(true, Ordering::SeqCst);
        assert!(
            clone.flashing.load(Ordering::SeqCst),
            "клон видит изменение флага через общий Arc"
        );
    }

    #[tokio::test]
    async fn resume_notify_wakes_waiter() {
        // Контракт serial::run: ждёт resume.notified(), flash дёргает notify_waiters().
        let gate = FlashGate::new();
        gate.flashing.store(true, Ordering::SeqCst);

        let waiter = gate.clone();
        let handle = tokio::spawn(async move {
            waiter.resume.notified().await;
            waiter.flashing.load(Ordering::SeqCst)
        });

        // Дать таску добраться до notified() перед нотификацией.
        tokio::task::yield_now().await;

        gate.flashing.store(false, Ordering::SeqCst);
        gate.resume.notify_waiters();

        let still_flashing = tokio::time::timeout(Duration::from_secs(1), handle)
            .await
            .expect("waiter должен проснуться, а не зависнуть")
            .unwrap();
        assert!(!still_flashing, "после resume флаг снят");
    }

    // Инвариант flash(): по завершении флаг снят и waiters разбужены —
    // независимо от исхода probe-rs (нет платы → Err, есть → Ok). Тест не
    // зависит от наличия probe-rs в окружении, только от gate-контракта.
    // `#[ignore]`: дёргает реальный probe-rs subprocess (медленно, трогает HW-слой);
    // запускается явно через `cargo test -- --ignored`, не в обычном CI-прогоне.
    #[tokio::test]
    #[ignore = "spawns real probe-rs subprocess; run with --ignored"]
    async fn flash_clears_flag_and_notifies_regardless_of_outcome() {
        let gate = FlashGate::new();

        let waiter = gate.clone();
        let woke = tokio::spawn(async move {
            waiter.resume.notified().await;
        });
        tokio::task::yield_now().await;

        // Исход (Ok/Err) зависит от наличия платы — нам важен только инвариант.
        let _ = flash(&gate).await;

        assert!(
            !gate.flashing.load(Ordering::SeqCst),
            "flash обязан снять флаг в конце, при любом исходе"
        );
        tokio::time::timeout(Duration::from_secs(2), woke)
            .await
            .expect("flash должен разбудить serial-ридер через notify_waiters")
            .unwrap();
    }
}
