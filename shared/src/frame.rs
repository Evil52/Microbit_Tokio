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
    use super::*;

    fn sample() -> Telemetry {
        Telemetry {
            temp_q4: 112,
            accel_mg: [12, -34, 1000],
            btn_a: 5,
            btn_b: 2,
        }
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
    fn corrupt_byte_fails() {
        let t = sample();
        let mut out = [0u8; MAX_FRAME];
        let frame = encode_frame(&t, &mut out).unwrap();
        let mut bad = frame.to_vec();
        bad[1] ^= 0xff;

        assert!(decode_frame(&bad).map(|x| x != t).unwrap_or(true));
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
}
