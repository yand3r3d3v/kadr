//! `~/.config/kadr/config.toml`: the interface language and the values a
//! form starts with.
//!
//! ```toml
//! lang = "ru"
//!
//! [compress]
//! crf = 26
//! preset = "slow"
//! resolution = "720p"
//!
//! [audio]
//! format = "copy"
//!
//! [gif]
//! fps = 15
//! width = 640
//! ```

use std::path::PathBuf;

use anyhow::{Context, Result};
use toml_edit::{DocumentMut, Item, value};

use crate::cli::{AudioFormat, Prefill};
use crate::text::Lang;

#[derive(Default, Debug, Clone)]
pub struct Config {
    pub lang: Lang,
    /// Applied to every file that is opened, before the command line options.
    pub defaults: Prefill,
}

pub fn path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("kadr").join("config.toml"))
}

/// Reads what it understands and ignores the rest: a config written for a
/// newer kadr, or with a typo, must not stop the program from starting.
pub fn parse(text: &str) -> Config {
    let Ok(doc) = text.parse::<DocumentMut>() else { return Config::default() };
    let get = |table: &str, key: &str| -> Option<&Item> { doc.get(table)?.get(key) };
    let int = |table: &str, key: &str| get(table, key).and_then(Item::as_integer);
    let string = |table: &str, key: &str| get(table, key).and_then(Item::as_str).map(str::to_string);

    Config {
        lang: match doc.get("lang").and_then(Item::as_str) {
            Some("ru") => Lang::Ru,
            _ => Lang::En,
        },
        defaults: Prefill {
            crf: int("compress", "crf"),
            preset: string("compress", "preset"),
            resolution: string("compress", "resolution"),
            no_audio: get("compress", "audio").and_then(Item::as_bool) == Some(false),
            exact: get("cut", "exact").and_then(Item::as_bool) == Some(true),
            format: match string("audio", "format").as_deref() {
                Some("copy") => Some(AudioFormat::Copy),
                Some("mp3") => Some(AudioFormat::Mp3),
                _ => None,
            },
            quality: int("audio", "quality"),
            fps: int("gif", "fps"),
            width: int("gif", "width"),
            length: int("gif", "length"),
            image: string("frame", "format"),
            ..Default::default()
        },
    }
}

pub fn load() -> Config {
    path().and_then(|p| std::fs::read_to_string(p).ok()).map(|t| parse(&t)).unwrap_or_default()
}

/// Sets `lang` and leaves everything else in the file, comments included, as
/// the user wrote it.
pub fn with_lang(text: &str, lang: Lang) -> String {
    let mut doc = text.parse::<DocumentMut>().unwrap_or_default();
    doc["lang"] = value(if lang == Lang::Ru { "ru" } else { "en" });
    doc.to_string()
}

pub fn save_lang(lang: Lang) -> Result<()> {
    let path = path().context("there is no home directory to keep the config in")?;
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&path, with_lang(&text, lang))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_language_and_the_defaults() {
        let c = parse("lang = \"ru\"\n[compress]\ncrf = 26\npreset = \"slow\"\n[gif]\nfps = 15\n");
        assert_eq!(c.lang, Lang::Ru);
        assert_eq!(c.defaults.crf, Some(26));
        assert_eq!(c.defaults.preset.as_deref(), Some("slow"));
        assert_eq!(c.defaults.fps, Some(15));
    }

    #[test]
    fn a_broken_file_is_an_empty_config() {
        let c = parse("this is = = not toml");
        assert_eq!(c.lang, Lang::En);
        assert_eq!(c.defaults.crf, None);
    }

    #[test]
    fn switching_the_language_keeps_the_rest_of_the_file() {
        let before = "# my settings\nlang = \"en\"\n\n[compress]\ncrf = 26 # smaller\n";
        let after = with_lang(before, Lang::Ru);
        assert_eq!(after, "# my settings\nlang = \"ru\"\n\n[compress]\ncrf = 26 # smaller\n");
        assert_eq!(with_lang("", Lang::Ru), "lang = \"ru\"\n");
    }
}
