//! Key events -> [`Msg`]. One table, no logic: user-customizable keymaps
//! later (post-v0.3) are a data change here, not a refactor (ADR-0003).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::Msg;

pub fn msg_for_key(key: KeyEvent) -> Option<Msg> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char('c') if ctrl => Some(Msg::Quit),
        KeyCode::Char('d') if ctrl => Some(Msg::HalfPageDown),
        KeyCode::Char('u') if ctrl => Some(Msg::HalfPageUp),
        KeyCode::Char('q') => Some(Msg::Quit),
        KeyCode::Char('j') | KeyCode::Down => Some(Msg::CursorDown),
        KeyCode::Char('k') | KeyCode::Up => Some(Msg::CursorUp),
        KeyCode::Char('J') => Some(Msg::NextHunk),
        KeyCode::Char('K') => Some(Msg::PrevHunk),
        KeyCode::Char(']') | KeyCode::Tab => Some(Msg::NextFile),
        KeyCode::Char('[') | KeyCode::BackTab => Some(Msg::PrevFile),
        KeyCode::Char('g') => Some(Msg::GKey),
        KeyCode::Char('G') | KeyCode::End => Some(Msg::Bottom),
        KeyCode::Home => Some(Msg::GKey), // two Homes == gg; fine
        KeyCode::PageDown => Some(Msg::HalfPageDown),
        KeyCode::PageUp => Some(Msg::HalfPageUp),
        KeyCode::Char('b') => Some(Msg::ToggleSidebar),
        KeyCode::Char('?') => Some(Msg::ToggleHelp),
        KeyCode::Esc => Some(Msg::Escape),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEventKind;

    fn key(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        let mut event = KeyEvent::new(code, mods);
        event.kind = KeyEventKind::Press;
        event
    }

    #[test]
    fn core_bindings() {
        assert_eq!(
            msg_for_key(key(KeyCode::Char('j'), KeyModifiers::NONE)),
            Some(Msg::CursorDown)
        );
        assert_eq!(
            msg_for_key(key(KeyCode::Char('J'), KeyModifiers::SHIFT)),
            Some(Msg::NextHunk),
            "shifted chars arrive as uppercase Char with SHIFT set"
        );
        assert_eq!(
            msg_for_key(key(KeyCode::Char('d'), KeyModifiers::CONTROL)),
            Some(Msg::HalfPageDown)
        );
        assert_eq!(
            msg_for_key(key(KeyCode::Char('d'), KeyModifiers::NONE)),
            None,
            "plain d is reserved (discard arrives in v0.2 behind confirm)"
        );
        assert_eq!(
            msg_for_key(key(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Some(Msg::Quit)
        );
    }
}
