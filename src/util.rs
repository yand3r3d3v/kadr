//! Time, size and shell formatting shared by the form and the plain mode.

/// Short clock for reading: `02:10`, or `01:47:03` past an hour.
pub fn fmt_clock(t: f64) -> String {
    let s = t.max(0.0).round() as u64;
    let (h, m, s) = (s / 3600, s / 60 % 60, s % 60);
    if h > 0 {
        format!("{h:02}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

/// Timestamp as it goes into the ffmpeg command: `00:00:38` or `00:00:38.500`.
pub fn fmt_ts(t: f64) -> String {
    let ms = (t.max(0.0) * 1000.0).round() as u64;
    let (h, m, s, ms) = (ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000);
    if ms == 0 {
        format!("{h:02}:{m:02}:{s:02}")
    } else {
        format!("{h:02}:{m:02}:{s:02}.{ms:03}")
    }
}

/// Accepts `38`, `38.5`, `0:38`, `00:00:38`.
pub fn parse_time(s: &str) -> Option<f64> {
    let parts: Vec<&str> = s.trim().split(':').collect();
    if parts.is_empty() || parts.len() > 3 || parts.iter().any(|p| p.is_empty()) {
        return None;
    }
    let (last, head) = parts.split_last()?;
    let mut total = 0.0;
    for p in head {
        total = total * 60.0 + p.parse::<u32>().ok()? as f64;
    }
    let sec: f64 = last.parse().ok()?;
    if !sec.is_finite() || sec < 0.0 {
        return None;
    }
    Some(total * 60.0 + sec)
}

pub fn fmt_size(bytes: u64) -> String {
    let b = bytes as f64;
    if b < 1e6 {
        format!("{} KB", (b / 1e3).round().max(1.0))
    } else if b < 1e7 {
        format!("{:.1} MB", b / 1e6)
    } else if b < 1e9 {
        format!("{:.0} MB", b / 1e6)
    } else {
        format!("{:.1} GB", b / 1e9)
    }
}

pub fn needs_quote(s: &str) -> bool {
    s.is_empty()
        || !s
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_@%+=:,./-".contains(c))
}

pub fn shell_quote(s: &str) -> String {
    if needs_quote(s) {
        format!("'{}'", s.replace('\'', r"'\''"))
    } else {
        s.to_string()
    }
}

/// Letter keys keep working in a Cyrillic layout: the key that types `ш` is `i`.
pub fn latin_key(c: char) -> char {
    const RU: &str = "йцукенгшщзфывапролдячсмитьЙЦУКЕНГШЩЗФЫВАПРОЛДЯЧСМИТЬбю";
    const EN: &str = "qwertyuiopasdfghjklzxcvbnmQWERTYUIOPASDFGHJKLZXCVBNM,.";
    match RU.chars().position(|r| r == c) {
        Some(i) => EN.chars().nth(i).unwrap_or(c),
        None => c,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_and_timestamp() {
        assert_eq!(fmt_clock(130.0), "02:10");
        assert_eq!(fmt_clock(6423.0), "01:47:03");
        assert_eq!(fmt_ts(38.0), "00:00:38");
        assert_eq!(fmt_ts(38.5), "00:00:38.500");
    }

    #[test]
    fn time_input() {
        assert_eq!(parse_time("38"), Some(38.0));
        assert_eq!(parse_time("0:38"), Some(38.0));
        assert_eq!(parse_time("00:01:12"), Some(72.0));
        assert_eq!(parse_time("38.5"), Some(38.5));
        assert_eq!(parse_time("1::2"), None);
        assert_eq!(parse_time("abc"), None);
    }

    #[test]
    fn sizes() {
        assert_eq!(fmt_size(48_000_000), "48 MB");
        assert_eq!(fmt_size(9_800_000), "9.8 MB");
        assert_eq!(fmt_size(1_900_000_000), "1.9 GB");
    }

    #[test]
    fn quoting() {
        assert_eq!(shell_quote("lecture_04.mov"), "lecture_04.mov");
        assert_eq!(shell_quote("my file.mov"), "'my file.mov'");
        assert_eq!(shell_quote("it's.mov"), r"'it'\''s.mov'");
    }

    #[test]
    fn cyrillic_layout() {
        assert_eq!(latin_key('ш'), 'i');
        assert_eq!(latin_key('щ'), 'o');
        assert_eq!(latin_key('с'), 'c');
        assert_eq!(latin_key('x'), 'x');
    }
}
