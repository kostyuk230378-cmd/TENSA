//! Разбор MAC-адреса и сборка magic packet (ТЗ tensa-tools, инструмент WoL).

use std::fmt;

use super::error::WolError;

/// Длина MAC-адреса в байтах.
pub const MAC_LEN: usize = 6;

/// Сколько раз MAC повторяется в magic packet.
pub const MAC_REPEATS: usize = 16;

/// Длина префикса magic packet: 6 байт `0xFF`.
pub const MAGIC_PREFIX_LEN: usize = 6;

/// Полная длина magic packet: 6 × `0xFF` + MAC × 16 = 102 байта.
pub const MAGIC_PACKET_LEN: usize = MAGIC_PREFIX_LEN + MAC_LEN * MAC_REPEATS;

/// MAC-адрес пробуждаемого устройства (6 байт).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MacAddress([u8; MAC_LEN]);

impl MacAddress {
    /// Собрать MAC из готовых байт.
    pub const fn from_bytes(bytes: [u8; MAC_LEN]) -> Self {
        Self(bytes)
    }

    /// Разобрать MAC из строки: `AA:BB:CC:DD:EE:FF` или без разделителей —
    /// `AABBCCDDEEFF`. Регистр символов не важен, пробелы по краям отбрасываются.
    ///
    /// Разделители `-` (форма `ipconfig`) не принимаются: ТЗ задаёт только
    /// двоеточие и форму без разделителей.
    pub fn parse(input: &str) -> Result<Self, WolError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(invalid(input, "empty string"));
        }

        let bytes = if trimmed.contains(':') {
            parse_octets(trimmed).map_err(|reason| invalid(input, reason))?
        } else {
            parse_without_separators(trimmed).map_err(|reason| invalid(input, reason))?
        };

        Ok(Self(bytes))
    }

    /// Байты MAC-адреса.
    pub const fn as_bytes(&self) -> &[u8; MAC_LEN] {
        &self.0
    }
}

impl fmt::Display for MacAddress {
    /// Форма `AA:BB:CC:DD:EE:FF` в верхнем регистре.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [a, b, c, d, e, g] = self.0;
        write!(f, "{a:02X}:{b:02X}:{c:02X}:{d:02X}:{e:02X}:{g:02X}")
    }
}

/// Собрать magic packet: 6 байт `0xFF` + MAC, повторённый 16 раз.
pub fn build_magic_packet(mac: &MacAddress) -> [u8; MAGIC_PACKET_LEN] {
    let mut packet = [0xFFu8; MAGIC_PACKET_LEN];
    // Префикс уже заполнен 0xFF; дальше 16 копий MAC по 6 байт.
    let (copies, _rest) = packet[MAGIC_PREFIX_LEN..].as_chunks_mut::<MAC_LEN>();
    for chunk in copies {
        chunk.copy_from_slice(mac.as_bytes());
    }
    packet
}

/// Ошибка разбора MAC с сохранением исходной строки.
fn invalid(input: &str, reason: &'static str) -> WolError {
    WolError::InvalidMac {
        input: input.to_string(),
        reason,
    }
}

/// Шестнадцатеричное значение символа (регистр не важен).
fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Разобрать один октет: ровно 2 hex-символа.
fn parse_pair(pair: &[u8]) -> Result<u8, &'static str> {
    if pair.len() != 2 {
        return Err("each octet must be exactly 2 hex digits");
    }
    let [hi, lo] = [pair[0], pair[1]];
    let hi = hex_digit(hi).ok_or("octet contains a non-hex character")?;
    let lo = hex_digit(lo).ok_or("octet contains a non-hex character")?;
    Ok((hi << 4) | lo)
}

/// Разобрать форму с разделителями: 6 октетов через `:`.
fn parse_octets(trimmed: &str) -> Result<[u8; MAC_LEN], &'static str> {
    let parts: Vec<&str> = trimmed.split(':').collect();
    if parts.len() != MAC_LEN {
        return Err("expected 6 octets separated by ':'");
    }

    let mut mac = [0u8; MAC_LEN];
    for (slot, part) in mac.iter_mut().zip(parts) {
        *slot = parse_pair(part.as_bytes())?;
    }
    Ok(mac)
}

/// Разобрать форму без разделителей: ровно 12 hex-символов.
fn parse_without_separators(trimmed: &str) -> Result<[u8; MAC_LEN], &'static str> {
    let bytes = trimmed.as_bytes();
    if bytes.len() != MAC_LEN * 2 {
        return Err("expected 12 hex digits without separators");
    }

    let mut mac = [0u8; MAC_LEN];
    let (pairs, _rest) = bytes.as_chunks::<2>();
    for (slot, pair) in mac.iter_mut().zip(pairs) {
        *slot = parse_pair(pair)?;
    }
    Ok(mac)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Байты MAC `AA:BB:CC:DD:EE:FF`.
    const AA_BB: [u8; MAC_LEN] = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];

    /// Валидные формы: двоеточия (любой регистр) и без разделителей.
    #[test]
    fn parse_accepts_valid_forms() {
        for input in [
            "AA:BB:CC:DD:EE:FF",
            "aa:bb:cc:dd:ee:ff",
            "Aa:Bb:Cc:Dd:Ee:Ff",
            "AABBCCDDEEFF",
            "aabbccddeeff",
            " AaBbCcDdEeFf ",
        ] {
            let mac = MacAddress::parse(input).expect("валидный MAC");
            assert_eq!(mac.as_bytes(), &AA_BB, "input: {input}");
            assert_eq!(mac.to_string(), "AA:BB:CC:DD:EE:FF", "input: {input}");
        }
    }

    /// Граничные октеты и круговой разбор (parse → Display → parse).
    #[test]
    fn parse_round_trips_through_display() {
        let mac = MacAddress::from_bytes([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
        assert_eq!(mac.to_string(), "00:11:22:33:44:55");
        assert_eq!(
            MacAddress::parse("00:11:22:33:44:55").expect("валидный MAC"),
            mac
        );

        let mac = MacAddress::from_bytes([0x0F, 0xF0, 0x09, 0x90, 0xAB, 0xBA]);
        assert_eq!(mac.to_string(), "0F:F0:09:90:AB:BA");
        assert_eq!(
            MacAddress::parse("0F:F0:09:90:AB:BA").expect("валидный MAC"),
            mac
        );
    }

    /// Невалидные формы MAC.
    #[test]
    fn parse_rejects_invalid_forms() {
        for input in [
            "",
            "   ",
            "AA:BB:CC:DD:EE",
            "AA:BB:CC:DD:EE:FF:00",
            "AA:BB:CC:DD:EE:F",
            "AA::BB:CC:DD:EE",
            "AA-BB-CC-DD-EE-FF",
            "GG:BB:CC:DD:EE:FF",
            "aa bb cc dd ee ff",
            "AABBCCDDEEF",
            "AABBCCDDEEFF00",
            "АА:ВВ:СС:DD:EE:FF",
        ] {
            assert!(
                MacAddress::parse(input).is_err(),
                "ожидалась ошибка для: {input}"
            );
        }
    }

    /// Ошибка сохраняет исходную строку и причину.
    #[test]
    fn parse_error_keeps_input_and_reason() {
        let err = MacAddress::parse("AA:BB:CC:DD:EE").expect_err("невалидный MAC");
        match err {
            WolError::InvalidMac { input, reason } => {
                assert_eq!(input, "AA:BB:CC:DD:EE");
                assert_eq!(reason, "expected 6 octets separated by ':'");
            }
            other => panic!("не та ошибка: {other:?}"),
        }

        let err = MacAddress::parse("AABBCC").expect_err("невалидный MAC");
        assert!(matches!(err, WolError::InvalidMac { .. }));
    }

    /// Пакет: 6 байт 0xFF + MAC 16 раз, побайтовое сравнение.
    #[test]
    fn magic_packet_has_exact_bytes() {
        assert_eq!(MAC_REPEATS, 16);
        assert_eq!(MAGIC_PREFIX_LEN, 6);
        assert_eq!(MAGIC_PACKET_LEN, 102);

        let packet = build_magic_packet(&MacAddress::from_bytes(AA_BB));

        let mut expected = vec![0xFFu8; MAGIC_PREFIX_LEN];
        for _ in 0..MAC_REPEATS {
            expected.extend_from_slice(&AA_BB);
        }

        assert_eq!(packet.len(), expected.len());
        assert_eq!(packet.as_slice(), expected.as_slice());
        assert_eq!(&packet[..MAGIC_PREFIX_LEN], &[0xFFu8; MAGIC_PREFIX_LEN]);
        let (copies, _rest) = packet[MAGIC_PREFIX_LEN..].as_chunks::<MAC_LEN>();
        for (index, chunk) in copies.iter().enumerate() {
            assert_eq!(chunk, &AA_BB, "копия MAC #{index}");
        }
    }

    /// Нулевой MAC не смешивается с префиксом 0xFF.
    #[test]
    fn magic_packet_with_zero_mac() {
        let packet = build_magic_packet(&MacAddress::from_bytes([0x00; MAC_LEN]));
        assert_eq!(&packet[..MAGIC_PREFIX_LEN], &[0xFFu8; MAGIC_PREFIX_LEN]);
        assert!(packet[MAGIC_PREFIX_LEN..].iter().all(|byte| *byte == 0x00));
        assert_eq!(packet.iter().filter(|byte| **byte == 0xFF).count(), 6);
    }
}
