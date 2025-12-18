//! Color themes.

use ratatui::style::Color;

pub struct Theme {
    pub background: Color,
    pub foreground: Color,
    pub highlight: Color,
    pub success: Color,
    pub error: Color,
    pub warning: Color,
}

impl Theme {
    pub fn dark() -> Self {
        Self {
            background: Color::Black,
            foreground: Color::White,
            highlight: Color::Cyan,
            success: Color::Green,
            error: Color::Red,
            warning: Color::Yellow,
        }
    }

    pub fn light() -> Self {
        Self {
            background: Color::White,
            foreground: Color::Black,
            highlight: Color::Blue,
            success: Color::Green,
            error: Color::Red,
            warning: Color::Yellow,
        }
    }
}
