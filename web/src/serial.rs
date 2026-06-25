//! Чтение CDC-порта micro:bit: байты → COBS-кадры (раздел 0x00) →
//! shared::decode_frame → broadcast подписчикам (SSE).
//! Устойчив к flash/обрыву: внешний цикл переоткрывает порт.

use std::sync::atomic::Ordering;
use std::time::Duration;

use shared::{decode_frame, Telemetry};
use tokio::io::AsyncReadExt;
use tokio::sync::broadcast;
use tokio_serial::SerialPortBuilderExt;

use crate::flash::FlashGate;

/// Максимальный размер накопителя кадра до принудительного ресинка.
/// COBS-кадр всегда < этого (см. shared::MAX_FRAME), так что переполнение =
/// рассинхрон потока, а не легитимный длинный кадр.
const MAX_ACC: usize = 128;

/// Результат скармливания одного байта в накопитель кадров.
#[derive(Debug, PartialEq, Eq)]
pub enum Feed {
    /// Байт накоплен, кадр ещё не завершён.
    NeedMore,
    /// Встретился разделитель 0x00, кадр готов к декодированию (вызови `take`).
    FrameReady,
    /// Накопитель переполнился без разделителя — поток сброшен (ресинк).
    Overflow,
}

/// Сборщик COBS-кадров из потока байтов: разделитель — 0x00.
///
/// Вынесен из async-цикла чтобы логику фрейминга/ресинка можно было
/// тестировать без железа. `serial::run` и `hil_smoke` используют ту же машину.
#[derive(Default)]
pub struct FrameAccumulator {
    acc: Vec<u8>,
}

impl FrameAccumulator {
    pub fn new() -> Self {
        Self {
            acc: Vec::with_capacity(MAX_ACC),
        }
    }

    /// Скормить один байт. См. [`Feed`].
    pub fn push(&mut self, b: u8) -> Feed {
        if b == 0x00 {
            if self.acc.is_empty() {
                // Пустой кадр между разделителями (idle/двойной 0x00) — пропускаем.
                Feed::NeedMore
            } else {
                Feed::FrameReady
            }
        } else {
            self.acc.push(b);
            if self.acc.len() > MAX_ACC {
                self.acc.clear();
                Feed::Overflow
            } else {
                Feed::NeedMore
            }
        }
    }

    /// Забрать накопленный кадр и очистить накопитель (после `Feed::FrameReady`).
    pub fn take(&mut self) -> Vec<u8> {
        let frame = self.acc.clone();
        self.acc.clear();
        frame
    }

    /// Сколько байт сейчас в накопителе (диагностика; используется в тестах).
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.acc.len()
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.acc.is_empty()
    }
}

/// Внешний цикл: держит соединение, переоткрывает при flash/обрыве.
pub async fn run(port: String, tx: broadcast::Sender<Telemetry>, gate: FlashGate) {
    loop {
        if gate.flashing.load(Ordering::SeqCst) {
            gate.resume.notified().await;
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        if let Err(e) = read_loop(&port, &tx, &gate).await {
            tracing::warn!(?e, "serial disconnected, retrying in 1s");
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
}

/// Внутренний цикл чтения. Возвращается при ошибке или начале flash.
async fn read_loop(
    port: &str,
    tx: &broadcast::Sender<Telemetry>,
    gate: &FlashGate,
) -> anyhow::Result<()> {
    let mut serial = tokio_serial::new(port, 115_200).open_native_async()?;
    tracing::info!(port = %port, "serial opened");

    let mut framer = FrameAccumulator::new();
    let mut byte = [0u8; 1];

    loop {
        if gate.flashing.load(Ordering::SeqCst) {
            tracing::info!("pausing serial for flash");
            return Ok(());
        }

        let n = serial.read(&mut byte).await?;
        if n == 0 {
            continue;
        }

        match framer.push(byte[0]) {
            Feed::NeedMore => {}
            Feed::Overflow => tracing::warn!("frame overflow, resync"),
            Feed::FrameReady => match decode_frame(&framer.take()) {
                Ok(frame) => {
                    let _ = tx.send(frame);
                }
                Err(e) => tracing::warn!(?e, "bad frame"),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::{encode_frame, MAX_FRAME};

    fn sample() -> Telemetry {
        Telemetry {
            temp_q4: 84,
            accel_mg: [5, -5, 1000],
            btn_a: 1,
            btn_b: 2,
        }
    }

    /// Закодировать кадр на провод (COBS-тело + завершающий 0x00), как делает firmware.
    fn wire(t: &Telemetry) -> Vec<u8> {
        let mut out = [0u8; MAX_FRAME];
        let frame = encode_frame(t, &mut out).unwrap();
        let mut bytes = frame.to_vec();
        bytes.push(0x00);
        bytes
    }

    /// Прогнать байты через накопитель, вернуть все декодированные кадры.
    fn drain(framer: &mut FrameAccumulator, bytes: &[u8]) -> Vec<Telemetry> {
        let mut frames = Vec::new();
        for &b in bytes {
            if framer.push(b) == Feed::FrameReady {
                if let Ok(t) = decode_frame(&framer.take()) {
                    frames.push(t);
                }
            }
        }
        frames
    }

    #[test]
    fn single_frame_decodes() {
        let t = sample();
        let mut f = FrameAccumulator::new();
        assert_eq!(drain(&mut f, &wire(&t)), vec![t]);
        assert!(f.is_empty(), "после кадра накопитель пуст");
    }

    #[test]
    fn back_to_back_frames() {
        let a = sample();
        let b = Telemetry {
            btn_a: 99,
            ..sample()
        };
        let mut stream = wire(&a);
        stream.extend(wire(&b));

        let mut f = FrameAccumulator::new();
        assert_eq!(drain(&mut f, &stream), vec![a, b]);
    }

    #[test]
    fn leading_garbage_then_resync_on_separator() {
        // Подключились в середине потока: мусор, затем 0x00, затем чистый кадр.
        let t = sample();
        let mut stream = vec![0x11, 0x22, 0x33, 0x00]; // мусорный «кадр» — отсеется CRC
        stream.extend(wire(&t));

        let mut f = FrameAccumulator::new();
        let frames = drain(&mut f, &stream);
        assert_eq!(frames, vec![t], "после ресинка получаем валидный кадр");
    }

    #[test]
    fn double_separator_is_not_empty_frame() {
        // 0x00 0x00 не должен порождать пустой кадр или панику.
        let t = sample();
        let mut stream = vec![0x00, 0x00];
        stream.extend(wire(&t));

        let mut f = FrameAccumulator::new();
        assert_eq!(drain(&mut f, &stream), vec![t]);
    }

    #[test]
    fn empty_accumulator_on_separator_needs_more() {
        let mut f = FrameAccumulator::new();
        assert_eq!(f.push(0x00), Feed::NeedMore);
        assert!(f.is_empty());
    }

    #[test]
    fn overflow_resets_and_recovers() {
        let mut f = FrameAccumulator::new();
        // Заваливаем >MAX_ACC ненулевых байт без разделителя.
        let mut saw_overflow = false;
        for _ in 0..(MAX_ACC + 5) {
            if f.push(0xAB) == Feed::Overflow {
                saw_overflow = true;
                break;
            }
        }
        assert!(saw_overflow, "должно случиться переполнение");
        assert!(f.is_empty(), "после overflow накопитель сброшен");

        // После сброса нормальный кадр всё ещё декодируется.
        let t = sample();
        assert_eq!(drain(&mut f, &wire(&t)), vec![t]);
    }

    #[test]
    fn exactly_max_acc_bytes_no_overflow() {
        // Ровно MAX_ACC байт — это ещё не переполнение (граница).
        let mut f = FrameAccumulator::new();
        for _ in 0..MAX_ACC {
            assert_eq!(f.push(0x01), Feed::NeedMore);
        }
        assert_eq!(f.len(), MAX_ACC);
        // следующий ненулевой байт переполняет
        assert_eq!(f.push(0x01), Feed::Overflow);
    }

    #[test]
    fn frame_split_across_reads_still_decodes() {
        // Эмулируем побайтовое поступление (как serial.read по 1 байту) —
        // drain и так подаёт по байту, так что просто проверяем разбиение
        // потока на произвольных границах.
        let t = sample();
        let stream = wire(&t);
        let mut f = FrameAccumulator::new();
        let mut frames = Vec::new();
        // подаём первую половину, потом вторую
        let (head, tail) = stream.split_at(stream.len() / 2);
        frames.extend(drain(&mut f, head));
        frames.extend(drain(&mut f, tail));
        assert_eq!(frames, vec![t]);
    }

    #[test]
    fn corrupted_frame_dropped_but_stream_continues() {
        let good = sample();
        // битый кадр: валидное тело с перевёрнутым байтом + следующий хороший
        let mut bad = wire(&good);
        bad[1] ^= 0xff; // ломаем CRC
        let mut stream = bad;
        stream.extend(wire(&good));

        let mut f = FrameAccumulator::new();
        let frames = drain(&mut f, &stream);
        // битый отсеялся, хороший прошёл
        assert_eq!(frames, vec![good]);
    }
}
