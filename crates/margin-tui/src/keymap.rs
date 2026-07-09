//! Key events -> [`Msg`], one table per input mode, no logic — so
//! user-customizable keymaps later are a data change (ADR-0003).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{InputMode, Msg};

pub fn msg_for_key(key: KeyEvent, mode: InputMode) -> Option<Msg> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    // Ctrl-C quits from anywhere: the escape hatch must never be modal.
    if ctrl && key.code == KeyCode::Char('c') {
        return Some(Msg::Quit);
    }
    match mode {
        InputMode::Normal => normal(key, ctrl),
        InputMode::Search => search(key),
        InputMode::Picker => picker(key, ctrl),
    }
}

fn normal(key: KeyEvent, ctrl: bool) -> Option<Msg> {
    match key.code {
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
        KeyCode::Char('/') => Some(Msg::SearchStart),
        KeyCode::Char('n') => Some(Msg::NextMatch),
        KeyCode::Char('N') => Some(Msg::PrevMatch),
        KeyCode::Char('f') => Some(Msg::PickerStart),
        KeyCode::Char('s') => Some(Msg::StageHunk),
        KeyCode::Char('u') => Some(Msg::UnstageHunk),
        KeyCode::Char('r') => Some(Msg::Reload),
        KeyCode::Char('v') => Some(Msg::ToggleLayout),
        KeyCode::Char('w') => Some(Msg::ToggleWrap),
        KeyCode::Char('b') => Some(Msg::ToggleSidebar),
        KeyCode::Char('?') => Some(Msg::ToggleHelp),
        KeyCode::Esc => Some(Msg::Escape),
        _ => None,
    }
}

fn search(key: KeyEvent) -> Option<Msg> {
    match key.code {
        KeyCode::Esc => Some(Msg::SearchCancel),
        KeyCode::Enter => Some(Msg::SearchConfirm),
        KeyCode::Backspace => Some(Msg::SearchBackspace),
        KeyCode::Char(c) => Some(Msg::SearchInput(c)),
        _ => None,
    }
}

fn picker(key: KeyEvent, ctrl: bool) -> Option<Msg> {
    match key.code {
        KeyCode::Char('n') if ctrl => Some(Msg::PickerNext),
        KeyCode::Char('p') if ctrl => Some(Msg::PickerPrev),
        KeyCode::Esc => Some(Msg::PickerCancel),
        KeyCode::Enter => Some(Msg::PickerConfirm),
        KeyCode::Backspace => Some(Msg::PickerBackspace),
        KeyCode::Down => Some(Msg::PickerNext),
        KeyCode::Up => Some(Msg::PickerPrev),
        KeyCode::Char(c) => Some(Msg::PickerInput(c)),
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
        let normal = |code, mods| msg_for_key(key(code, mods), InputMode::Normal);
        assert_eq!(
            normal(KeyCode::Char('j'), KeyModifiers::NONE),
            Some(Msg::CursorDown)
        );
        assert_eq!(
            normal(KeyCode::Char('J'), KeyModifiers::SHIFT),
            Some(Msg::NextHunk),
            "shifted chars arrive as uppercase Char with SHIFT set"
        );
        assert_eq!(
            normal(KeyCode::Char('d'), KeyModifiers::CONTROL),
            Some(Msg::HalfPageDown)
        );
        assert_eq!(
            normal(KeyCode::Char('d'), KeyModifiers::NONE),
            None,
            "plain d is reserved (discard arrives in v0.2 behind confirm)"
        );
        assert_eq!(
            normal(KeyCode::Char('r'), KeyModifiers::NONE),
            Some(Msg::Reload)
        );
        assert_eq!(
            normal(KeyCode::Char('u'), KeyModifiers::CONTROL),
            Some(Msg::HalfPageUp),
            "Ctrl-u pages, plain u unstages"
        );
        assert_eq!(
            normal(KeyCode::Char('/'), KeyModifiers::NONE),
            Some(Msg::SearchStart)
        );
        assert_eq!(
            normal(KeyCode::Char('f'), KeyModifiers::NONE),
            Some(Msg::PickerStart)
        );
    }

    #[test]
    fn modal_routing() {
        // 'q' types into the search box instead of quitting...
        assert_eq!(
            msg_for_key(
                key(KeyCode::Char('q'), KeyModifiers::NONE),
                InputMode::Search
            ),
            Some(Msg::SearchInput('q'))
        );
        // ...but Ctrl-C quits from every mode.
        for mode in [InputMode::Normal, InputMode::Search, InputMode::Picker] {
            assert_eq!(
                msg_for_key(key(KeyCode::Char('c'), KeyModifiers::CONTROL), mode),
                Some(Msg::Quit)
            );
        }
        assert_eq!(
            msg_for_key(
                key(KeyCode::Char('n'), KeyModifiers::CONTROL),
                InputMode::Picker
            ),
            Some(Msg::PickerNext)
        );
        assert_eq!(
            msg_for_key(
                key(KeyCode::Char('n'), KeyModifiers::NONE),
                InputMode::Picker
            ),
            Some(Msg::PickerInput('n'))
        );
    }
}
