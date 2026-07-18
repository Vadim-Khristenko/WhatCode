//! Тема оформления. Все цвета — явные `Color::Rgb`, никаких магических ANSI-строк.
//! Палитра адаптируется под активную персону: белый по умолчанию, фиолетовый для
//! Герты, жёлтый для Anis. Остальные цвета — глубокий космос, мягкие границы,
//! читаемый текст.

use ratatui::style::{Color, Modifier, Style};
use whatcode_core::persona::PersonaColor;

/// Неизменяемая палитра. Передаётся в виджеты по ссылке, не клонируется на кадр.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub bg: Color,
    pub surface: Color,
    pub border: Color,
    pub border_focused: Color,
    pub text: Color,
    pub text_dim: Color,
    pub persona: Color,
    pub user: Color,
    pub accent: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self::from_persona_color(PersonaColor::WHITE)
    }
}

impl Theme {
    /// Создать тему из персонализированного цвета. Остальная палитра остаётся
    /// нейтральной, а `persona` и `accent` подстраиваются под цвет персоны.
    pub fn from_persona_color(color: PersonaColor) -> Self {
        let persona = Color::Rgb(color.r, color.g, color.b);
        // accent чуть теплее/ярче persona для заголовков и акцентов.
        let accent = if color == PersonaColor::WHITE {
            Color::Rgb(137, 221, 255) // ледяной циан для нейтрального режима
        } else {
            persona
        };
        Self {
            bg: Color::Rgb(13, 17, 28),
            surface: Color::Rgb(20, 26, 40),
            border: Color::Rgb(48, 58, 84),
            border_focused: Color::Rgb(122, 162, 247),
            text: Color::Rgb(205, 214, 244),
            text_dim: Color::Rgb(110, 122, 158),
            persona,
            user: Color::Rgb(158, 206, 106),
            accent,
            success: Color::Rgb(158, 206, 106),
            warning: Color::Rgb(224, 175, 104),
            error: Color::Rgb(247, 118, 142),
        }
    }

    pub fn base(&self) -> Style {
        Style::default().fg(self.text).bg(self.bg)
    }

    pub fn border(&self, focused: bool) -> Style {
        Style::default().fg(if focused {
            self.border_focused
        } else {
            self.border
        })
    }

    pub fn title(&self) -> Style {
        Style::default()
            .fg(self.accent)
            .add_modifier(Modifier::BOLD)
    }

    pub fn header(&self) -> Style {
        Style::default()
            .fg(self.persona)
            .add_modifier(Modifier::BOLD)
    }

    pub fn dim(&self) -> Style {
        Style::default().fg(self.text_dim)
    }

    pub fn persona_label(&self) -> Style {
        Style::default()
            .fg(self.persona)
            .add_modifier(Modifier::BOLD)
    }

    pub fn user_label(&self) -> Style {
        Style::default().fg(self.user).add_modifier(Modifier::BOLD)
    }

    pub fn active_item(&self) -> Style {
        Style::default().bg(self.surface).fg(self.persona)
    }

    pub fn subtle_surface(&self) -> Style {
        Style::default().bg(self.surface).fg(self.text_dim)
    }
}
