//! Декодирование base64/hex и поиск XOR-скрытых строк (правило 8).
//!
//! Найденные закодированные данные декодируются и прогоняются через
//! сигнатуры правил 1–5; само по себе кодирование почти не штрафуется.


/// Энтропия Шеннона по байтам (0..8 бит).
pub(crate) fn shannon_entropy(bytes: &[u8]) -> f64 {
    if bytes.is_empty() {
        return 0.0;
    }
    let mut counts = [0usize; 256];
    for &b in bytes {
        counts[b as usize] += 1;
    }
    let len = bytes.len() as f64;
    let mut h = 0.0;
    for &c in &counts {
        if c == 0 {
            continue;
        }
        let p = c as f64 / len;
        h -= p * p.log2();
    }
    h
}

/// Доля печатаемых ASCII-символов (для проверки, что декодированное — текст).
pub(crate) fn printable_ratio(bytes: &[u8]) -> f64 {
    if bytes.is_empty() {
        return 0.0;
    }
    let n = bytes
        .iter()
        .filter(|&&b| (0x20..0x7F).contains(&b) || b == b'\n' || b == b'\r' || b == b'\t')
        .count();
    n as f64 / bytes.len() as f64
}

/// Похоже ли на base64: длина от 16, только алфавит base64, выглядит случайно.
pub(crate) fn looks_base64(s: &str) -> bool {
    if s.len() < 16 || s.len() % 4 != 0 {
        return false;
    }
    let mut non_pad = 0usize;
    for (i, c) in s.bytes().enumerate() {
        let ok = c.is_ascii_alphanumeric() || c == b'+' || c == b'/' || (c == b'=' && i >= s.len() - 2);
        if !ok {
            return false;
        }
        if c != b'=' {
            non_pad += 1;
        }
    }
    // Словарные слова и хеши не считаем: нужно смешение регистра и цифр.
    let has_digit = s.bytes().any(|c| c.is_ascii_digit());
    let has_lower = s.bytes().any(|c| c.is_ascii_lowercase());
    let has_upper = s.bytes().any(|c| c.is_ascii_uppercase());
    non_pad >= 16 && has_digit && (has_lower || has_upper)
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn b64_val(c: u8) -> Option<u8> {
    B64.iter().position(|&x| x == c).map(|p| p as u8)
}

/// Декодирует base64. `None` при неверном паддинге или символах вне алфавита.
pub(crate) fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let clean: Vec<u8> = s
        .bytes()
        .filter(|&c| c != b'\n' && c != b'\r' && c != b' ' && c != b'\t')
        .collect();
    if clean.len() % 4 != 0 || clean.is_empty() {
        return None;
    }
    let pad = clean.iter().rev().take_while(|&&c| c == b'=').count();
    if pad > 2 {
        return None;
    }
    let mut out = Vec::with_capacity(clean.len() / 4 * 3);
    for chunk in clean.chunks_exact(4) {
        let v: [u8; 4] = [
            b64_val(chunk[0])?,
            b64_val(chunk[1])?,
            if chunk[2] == b'=' { 0 } else { b64_val(chunk[2])? },
            if chunk[3] == b'=' { 0 } else { b64_val(chunk[3])? },
        ];
        let n = ((v[0] as u32) << 18) | ((v[1] as u32) << 12) | ((v[2] as u32) << 6) | v[3] as u32;
        out.push((n >> 16) as u8);
        if chunk[2] != b'=' {
            out.push((n >> 8) as u8);
        }
        if chunk[3] != b'=' {
            out.push(n as u8);
        }
    }
    Some(out)
}

/// Похоже ли на hex-строку: чётная длина от 32, только hex-цифры.
pub(crate) fn looks_hex(s: &str) -> bool {
    if s.len() < 32 || s.len() % 2 != 0 {
        return false;
    }
    s.bytes().all(|c| c.is_ascii_hexdigit())
}

/// Декодирует hex. `None` при нечётной длине или чужих символах.
pub(crate) fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(s.len() / 2);
    for pair in b.chunks_exact(2) {
        let hi = (pair[0] as char).to_digit(16)?;
        let lo = (pair[1] as char).to_digit(16)?;
        out.push(((hi << 4) | lo) as u8);
    }
    Some(out)
}

/// Ищет однобайтовый XOR-ключ, при котором в строке появляется один из
/// маркеров. Возвращает (ключ, расшифрованную строку).
pub(crate) fn xor_find_marker(s: &str, markers: &[&str]) -> Option<(u8, String)> {
    let bytes = s.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    for key in 1u16..256 {
        let k = key as u8;
        let decoded: Vec<u8> = bytes.iter().map(|&b| b ^ k).collect();
        if printable_ratio(&decoded) < 0.9 {
            continue;
        }
        let text = String::from_utf8(decoded.clone()).ok()?;
        let hit = markers.iter().any(|m| text.contains(m));
        if hit {
            return Some((k, text));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trip() {
        let enc = "aHR0cHM6Ly9hcGkudGVsZWdyYW0ub3JnL2JvdDEyMw==";
        assert!(looks_base64(enc));
        let dec = base64_decode(enc).expect("decode");
        assert_eq!(String::from_utf8(dec).unwrap(), "https://api.telegram.org/bot123");
    }

    #[test]
    fn base64_rejects_words_and_bad_padding() {
        assert!(!looks_base64("modmenu-description-text"));
        assert!(!looks_base64("aGk=")); // короткая
        assert!(base64_decode("a=Gk").is_none());
    }

    #[test]
    fn hex_round_trip() {
        let s = "687474703a2f2f3132372e302e302e31";
        assert!(looks_hex(s));
        let dec = hex_decode(s).expect("hex");
        assert_eq!(String::from_utf8(dec).unwrap(), "http://127.0.0.1");
        assert!(!looks_hex("zz"));
    }

    #[test]
    fn xor_single_byte_is_found() {
        // "https://ngrok.io" ^ 0x35
        let enc: String = "https://ngrok.io"
            .bytes()
            .map(|b| (b ^ 0x35) as char)
            .collect();
        let (key, text) = xor_find_marker(&enc, &["ngrok.io"]).expect("xor");
        assert_eq!(key, 0x35);
        assert!(text.contains("ngrok.io"));
    }

    #[test]
    fn entropy_orders_noise_above_text() {
        let text = b"hello world hello world hello world";
        let noise: Vec<u8> = (0..64u8).collect();
        assert!(shannon_entropy(&noise) > shannon_entropy(text));
        assert!(printable_ratio(b"plain text\n") > 0.99);
        assert!(printable_ratio(&[0, 1, 2, 3]) < 0.1);
    }
}
