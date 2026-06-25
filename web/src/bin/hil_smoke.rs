//! HIL smoke-тест: читает CDC-порт несколько секунд, требует ≥3 валидных
//! кадра (decode через тот же `shared`, что и прод). Запускается на
//! self-hosted runner после flash+reset. Exit 0 = ok, 1 = fail.

use std::time::{Duration, Instant};

use shared::decode_frame;
use tokio::io::AsyncReadExt;
use tokio_serial::SerialPortBuilderExt;

const NEED_FRAMES: u32 = 1;
const TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let port = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/dev/cu.usbmodem312102".to_string());

    let mut serial = match tokio_serial::new(&port, 115_200).open_native_async() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("FAIL: cannot open {port}: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    eprintln!("port {port} opened, reading for {TIMEOUT:?}...");

    let mut acc: Vec<u8> = Vec::with_capacity(128);
    let mut byte = [0u8; 1];
    let start = Instant::now();
    let mut valid = 0u32;
    let mut total_bytes = 0u64;
    let mut separators = 0u64;
    let mut decode_fails = 0u64;

    while start.elapsed() < TIMEOUT {
        let n = match tokio::time::timeout(Duration::from_secs(2), serial.read(&mut byte)).await {
            Ok(Ok(n)) => n,
            _ => continue,
        };
        if n == 0 {
            continue;
        }
        total_bytes += 1;

        if byte[0] == 0x00 {
            separators += 1;
            if !acc.is_empty() {
                match decode_frame(&acc) {
                    Ok(frame) => {
                        valid += 1;
                        println!(
                            "frame #{valid}: temp={:.1}C accel={:?} btn_a={} btn_b={}",
                            frame.temp_celsius(),
                            frame.accel_mg,
                            frame.btn_a,
                            frame.btn_b
                        );
                        if valid >= NEED_FRAMES {
                            println!("PASS: {valid} valid frame(s) decoded");
                            return std::process::ExitCode::SUCCESS;
                        }
                    }
                    Err(_) => decode_fails += 1,
                }
                acc.clear();
            }
        } else {
            acc.push(byte[0]);
            if acc.len() > 128 {
                acc.clear();
            }
        }
    }

    eprintln!(
        "FAIL: 0 valid frames in {TIMEOUT:?}. \
         diag: total_bytes={total_bytes} separators={separators} decode_fails={decode_fails}"
    );
    std::process::ExitCode::FAILURE
}
