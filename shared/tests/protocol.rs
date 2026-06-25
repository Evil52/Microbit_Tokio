//! Интеграционные тесты публичного контракта `shared` — через тот же API,
//! что используют firmware (encode) и web (decode). Проверяем wire-формат
//! и устойчивость декодера к реалистичным сбоям UART.

use shared::{decode_frame, encode_frame, DecodeError, Telemetry, MAX_FRAME, PROTOCOL_VERSION};

fn sample() -> Telemetry {
    Telemetry {
        temp_q4: 100,
        accel_mg: [10, -20, 980],
        btn_a: 3,
        btn_b: 4,
    }
}

fn encode_vec(t: &Telemetry) -> Vec<u8> {
    let mut out = [0u8; MAX_FRAME];
    encode_frame(t, &mut out).unwrap().to_vec()
}

#[test]
fn public_roundtrip() {
    let t = sample();
    assert_eq!(decode_frame(&encode_vec(&t)).unwrap(), t);
}

#[test]
fn protocol_version_is_one() {
    // Контракт версии: пока схема не менялась — версия 1. Этот тест умышленно
    // падает при изменении PROTOCOL_VERSION, заставляя задуматься о совместимости.
    assert_eq!(PROTOCOL_VERSION, 1);
}

#[test]
fn wire_format_starts_with_version_after_cobs() {
    // Распаковав COBS вручную, первый байт тела должен быть версией протокола.
    let frame = encode_vec(&sample());
    let mut raw = [0u8; MAX_FRAME];
    let n = cobs::decode(&frame, &mut raw).unwrap();
    assert!(n >= 3, "тело должно содержать version + payload + crc16");
    assert_eq!(raw[0], PROTOCOL_VERSION, "первый байт тела — версия");
}

#[test]
fn truncated_frame_is_rejected() {
    let frame = encode_vec(&sample());
    // Обрезаем последний байт — CRC/COBS должны не пропустить.
    let truncated = &frame[..frame.len() - 1];
    assert!(decode_frame(truncated).is_err());
}

#[test]
fn extra_trailing_byte_is_rejected() {
    let mut frame = encode_vec(&sample());
    frame.push(0x42);
    // Лишний байт ломает либо COBS-структуру, либо CRC.
    assert!(decode_frame(&frame).is_err());
}

#[test]
fn boundary_values_roundtrip() {
    for t in [
        Telemetry {
            temp_q4: 0,
            accel_mg: [0, 0, 0],
            btn_a: 0,
            btn_b: 0,
        },
        Telemetry {
            temp_q4: i16::MAX,
            accel_mg: [i16::MAX, i16::MAX, i16::MAX],
            btn_a: u16::MAX,
            btn_b: u16::MAX,
        },
        Telemetry {
            temp_q4: i16::MIN,
            accel_mg: [i16::MIN, i16::MIN, i16::MIN],
            btn_a: 1,
            btn_b: 1,
        },
    ] {
        let decoded = decode_frame(&encode_vec(&t)).unwrap();
        assert_eq!(decoded, t);
    }
}

#[test]
fn decode_error_variants_are_distinguishable() {
    // Эти варианты — публичный API; гарантируем, что они различимы для логирования.
    assert_ne!(DecodeError::Cobs, DecodeError::TooShort);
    assert_ne!(DecodeError::Crc, DecodeError::Payload);
    assert_eq!(DecodeError::Version(1), DecodeError::Version(1));
    assert_ne!(DecodeError::Version(1), DecodeError::Version(2));
}

#[test]
fn temp_celsius_quarter_degree_steps() {
    // 0.25 °C на шаг: q4=1 → 0.25, q4=4 → 1.0.
    let mut t = sample();
    t.temp_q4 = 1;
    assert_eq!(t.temp_celsius(), 0.25);
    t.temp_q4 = 4;
    assert_eq!(t.temp_celsius(), 1.0);
    t.temp_q4 = -2;
    assert_eq!(t.temp_celsius(), -0.5);
}
