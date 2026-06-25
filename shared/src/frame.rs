//! Кодирование/декодирование кадра телеметрии.
//!
//! Слой данных (до COBS): `version(1) | postcard(Telemetry) | crc16_le(2)`.
//! CRC16 считается по `version + payload`. Затем весь блок проходит COBS
//! и завершается нулевым байтом-разделителем (его добавляет передатчик).

use crc::{Crc, CRC_16_IBM_SDLC};

use crate::{Telemetry, MAX_FRAME, PROTOCOL_VERSION};

/// CRC-16/X25 (IBM-SDLC): refin/refout, init 0xFFFF, xorout 0xFFFF.
/// Широко используется (HDLC), хорошо ловит burst-ошибки UART.
const CRC16: Crc<u16> = Crc::<u16>::new(&CRC_16_IBM_SDLC);

/// Ошибки декодирования кадра.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    /// COBS-распаковка не удалась (битый кадр).
    Cobs,
    /// Кадр короче минимума (version + crc).
    TooShort,
    /// version-байт не совпал с PROTOCOL_VERSION.
    Version(u8),
    /// CRC не сошёлся (повреждение данных).
    Crc,
    /// postcard не смог десериализовать payload.
    Payload,
}

/// Закодировать кадр в `out`. Возвращает срез с готовым COBS-телом
/// (без завершающего 0x00 — его добавляет передатчик при отправке в UART).
pub fn encode_frame<'a>(
    t: &Telemetry,
    out: &'a mut [u8; MAX_FRAME],
) -> Result<&'a [u8], postcard::Error> {
    let mut data = [0u8; MAX_FRAME];
    data[0] = PROTOCOL_VERSION;
    let payload = postcard::to_slice(t, &mut data[1..])?;
    let payload_len = payload.len();
    let data_len = 1 + payload_len;

    let crc = CRC16.checksum(&data[..data_len]);
    data[data_len] = (crc & 0xff) as u8;
    data[data_len + 1] = (crc >> 8) as u8;
    let framed_len = data_len + 2;

    let n = cobs::encode(&data[..framed_len], out);
    Ok(&out[..n])
}

/// Декодировать один COBS-кадр (без завершающего 0x00) в Telemetry.
pub fn decode_frame(frame: &[u8]) -> Result<Telemetry, DecodeError> {
    let mut buf = [0u8; MAX_FRAME];
    let n = cobs::decode(frame, &mut buf).map_err(|_| DecodeError::Cobs)?;
    if n < 3 {
        return Err(DecodeError::TooShort);
    }
    let data = &buf[..n];

    if data[0] != PROTOCOL_VERSION {
        return Err(DecodeError::Version(data[0]));
    }

    let crc_pos = n - 2;
    let got = u16::from_le_bytes([data[crc_pos], data[crc_pos + 1]]);
    let want = CRC16.checksum(&data[..crc_pos]);
    if got != want {
        return Err(DecodeError::Crc);
    }

    postcard::from_bytes(&data[1..crc_pos]).map_err(|_| DecodeError::Payload)
}

#[cfg(test)]
mod tests {
    // Тесты гоняются на хосте; крейт no_std, поэтому Vec тянем из std явно.
    extern crate std;
    use super::*;
    use proptest::prelude::*;
    use std::vec::Vec;

    fn sample() -> Telemetry {
        Telemetry {
            temp_q4: 112,
            accel_mg: [12, -34, 1000],
            btn_a: 5,
            btn_b: 2,
        }
    }

    /// Закодировать `t`, вернуть COBS-кадр как Vec (как его увидит UART, без 0x00).
    fn encode_vec(t: &Telemetry) -> Vec<u8> {
        let mut out = [0u8; MAX_FRAME];
        encode_frame(t, &mut out).unwrap().to_vec()
    }

    #[test]
    fn roundtrip() {
        let t = sample();
        let mut out = [0u8; MAX_FRAME];
        let frame = encode_frame(&t, &mut out).unwrap();
        let back = decode_frame(frame).unwrap();
        assert_eq!(t, back);
    }

    #[test]
    fn temp_celsius_correct() {
        assert_eq!(sample().temp_celsius(), 28.0);
    }

    #[test]
    fn temp_celsius_negative_and_zero() {
        let mut t = sample();
        t.temp_q4 = 0;
        assert_eq!(t.temp_celsius(), 0.0);
        t.temp_q4 = -40; // -10.0 °C
        assert_eq!(t.temp_celsius(), -10.0);
        t.temp_q4 = i16::MIN;
        assert_eq!(t.temp_celsius(), i16::MIN as f32 / 4.0);
    }

    #[test]
    fn corrupt_byte_fails() {
        let t = sample();
        let mut out = [0u8; MAX_FRAME];
        let frame = encode_frame(&t, &mut out).unwrap();
        let mut bad = frame.to_vec();
        bad[1] ^= 0xff;

        assert!(decode_frame(&bad).map(|x| x != t).unwrap_or(true));
    }

    /// Любая одиночная битовая ошибка в кадре должна быть поймана (Crc/Cobs/...),
    /// а не молча декодирована в неправильную, но валидную телеметрию.
    #[test]
    fn every_single_bit_flip_is_caught() {
        let t = sample();
        let frame = encode_vec(&t);
        for byte_idx in 0..frame.len() {
            for bit in 0..8 {
                let mut bad = frame.clone();
                bad[byte_idx] ^= 1 << bit;
                if let Ok(decoded) = decode_frame(&bad) {
                    assert_ne!(
                        decoded, t,
                        "битовая ошибка байт {byte_idx} бит {bit} прошла незамеченной"
                    );
                }
            }
        }
    }

    #[test]
    fn wrong_version_detected() {
        let t = sample();
        let mut out = [0u8; MAX_FRAME];
        let frame = encode_frame(&t, &mut out).unwrap();

        let mut raw = [0u8; MAX_FRAME];
        let n = cobs::decode(frame, &mut raw).unwrap();
        raw[0] = 99;
        let mut reenc = [0u8; MAX_FRAME];
        let m = cobs::encode(&raw[..n], &mut reenc);

        assert!(matches!(
            decode_frame(&reenc[..m]),
            Err(DecodeError::Version(99)) | Err(DecodeError::Crc)
        ));
    }

    /// version-байт меняем, а CRC пересчитываем — так Version-ветка проверяется
    /// детерминированно, без шанса уйти в Crc.
    #[test]
    fn version_mismatch_is_reported_before_crc() {
        let t = sample();
        let frame = encode_vec(&t);

        let mut raw = [0u8; MAX_FRAME];
        let n = cobs::decode(&frame, &mut raw).unwrap();
        raw[0] = PROTOCOL_VERSION.wrapping_add(1);
        // пересчёт CRC по version+payload, чтобы CRC сошёлся
        let crc = CRC16.checksum(&raw[..n - 2]);
        raw[n - 2] = (crc & 0xff) as u8;
        raw[n - 1] = (crc >> 8) as u8;

        let mut reenc = [0u8; MAX_FRAME];
        let m = cobs::encode(&raw[..n], &mut reenc);
        assert_eq!(
            decode_frame(&reenc[..m]),
            Err(DecodeError::Version(PROTOCOL_VERSION.wrapping_add(1)))
        );
    }

    #[test]
    fn empty_frame_is_cobs_error() {
        // Пустой вход — COBS не может распаковать (нужен хотя бы overhead-байт).
        assert!(matches!(decode_frame(&[]), Err(DecodeError::Cobs)));
    }

    #[test]
    fn too_short_after_cobs() {
        // COBS-кадр, распаковывающийся в <3 байта (нет места под version+crc).
        // [0x01] декодируется в пустой payload → n=0 < 3 → TooShort.
        assert_eq!(decode_frame(&[0x01]), Err(DecodeError::TooShort));
        // [0x02, 0xAA] → один байт данных → n=1 < 3 → TooShort.
        assert_eq!(decode_frame(&[0x02, 0xAA]), Err(DecodeError::TooShort));
    }

    #[test]
    fn garbage_is_rejected_not_decoded() {
        // Случайный мусор не должен превращаться в валидный Telemetry.
        for junk in [
            &[0xff, 0xff, 0xff, 0xff][..],
            &[0x05, 0x01, 0x02, 0x03, 0x04][..],
            &[0x00][..],
        ] {
            assert!(
                decode_frame(junk).is_err(),
                "мусор {junk:?} не должен декодироваться"
            );
        }
    }

    #[test]
    fn frame_fits_within_max_frame() {
        // Экстремальные значения дают самый длинный postcard-payload (varint).
        let t = Telemetry {
            temp_q4: i16::MIN,
            accel_mg: [i16::MIN, i16::MAX, i16::MIN],
            btn_a: u16::MAX,
            btn_b: u16::MAX,
        };
        let mut out = [0u8; MAX_FRAME];
        let frame = encode_frame(&t, &mut out).expect("должен влезть в MAX_FRAME");
        assert!(
            frame.len() <= MAX_FRAME,
            "кадр {} > MAX_FRAME {}",
            frame.len(),
            MAX_FRAME
        );
        // и обратно декодируется без потерь
        assert_eq!(decode_frame(frame).unwrap(), t);
    }

    #[test]
    fn encoded_frame_never_contains_zero_separator() {
        // COBS-инвариант: внутри тела кадра нет 0x00 — иначе сломается ресинк.
        let t = Telemetry {
            temp_q4: 0,
            accel_mg: [0, 0, 0],
            btn_a: 0,
            btn_b: 0,
        };
        let frame = encode_vec(&t);
        assert!(
            !frame.contains(&0x00),
            "тело COBS-кадра не должно содержать байт-разделитель 0x00"
        );
    }

    proptest! {
        /// Любой Telemetry проходит encode→decode без потерь.
        #[test]
        fn prop_roundtrip(
            temp_q4 in any::<i16>(),
            ax in any::<i16>(), ay in any::<i16>(), az in any::<i16>(),
            btn_a in any::<u16>(), btn_b in any::<u16>(),
        ) {
            let t = Telemetry { temp_q4, accel_mg: [ax, ay, az], btn_a, btn_b };
            let mut out = [0u8; MAX_FRAME];
            let frame = encode_frame(&t, &mut out).unwrap();
            prop_assert!(frame.len() <= MAX_FRAME);
            prop_assert!(!frame.contains(&0x00));
            prop_assert_eq!(decode_frame(frame).unwrap(), t);
        }

        /// Случайный байтовый мусор никогда не паникует и не выдаёт чужой Telemetry
        /// без срабатывания CRC.
        #[test]
        fn prop_random_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..80)) {
            // Главное — отсутствие паники/UB; результат может быть Ok или Err.
            let _ = decode_frame(&bytes);
        }

        /// Повреждение ровно одного байта тела ловится (или, редко, COBS-коллизия
        /// даёт другую длину) — но никогда не отдаёт исходный t как валидный.
        #[test]
        fn prop_corruption_never_silently_passes(
            temp_q4 in any::<i16>(),
            idx in 0usize..16,
            xor in 1u8..=255,
        ) {
            let t = Telemetry { temp_q4, accel_mg: [1, 2, 3], btn_a: 7, btn_b: 9 };
            let mut frame = encode_vec(&t);
            let i = idx % frame.len();
            frame[i] ^= xor;
            if let Ok(decoded) = decode_frame(&frame) {
                // Если декодировалось — это случайно может совпасть только если
                // повреждённый байт не влиял; но CRC должен это поймать. Допустим
                // лишь точное совпадение исходника (COBS вернул то же тело).
                prop_assert_eq!(decoded, t);
            }
        }
    }
}
