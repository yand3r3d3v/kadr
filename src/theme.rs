//! The palette. A colour's name says what it is for.

use ratatui::style::Color;

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    /// Frames, field labels, the part already done.
    pub ice: Color,
    /// Everything you can change.
    pub ochre: Color,
    /// Error and risk.
    pub carmine: Color,
    /// Flags in the command, hints.
    pub dim: Color,
    /// Thin lines.
    pub line: Color,
}

impl Theme {
    pub fn truecolor() -> Self {
        Theme {
            ice: Color::Rgb(0x7F, 0xB8, 0xE6),
            ochre: Color::Rgb(0xE3, 0xA7, 0x2F),
            carmine: Color::Rgb(0xF0, 0x62, 0x7A),
            dim: Color::Rgb(0x8A, 0x9B, 0xB0),
            line: Color::Rgb(0x4A, 0x7B, 0xA6),
        }
    }

    pub fn ansi() -> Self {
        Theme {
            ice: Color::Cyan,
            ochre: Color::Yellow,
            carmine: Color::Red,
            dim: Color::DarkGray,
            line: Color::DarkGray,
        }
    }

    pub fn detect() -> Self {
        match std::env::var("COLORTERM").as_deref() {
            Ok("truecolor") | Ok("24bit") => Theme::truecolor(),
            _ => Theme::ansi(),
        }
    }
}
