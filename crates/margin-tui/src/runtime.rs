//! The effectful shell around the pure core: terminal setup/teardown,
//! the event loop, and the panic guard (ADR-0009: a crash must never
//! leave the user's terminal in raw mode).

use std::io;
use std::sync::Once;

use crossterm::event::{Event, KeyEventKind};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::app::{update, AppState, Msg};
use crate::keymap::msg_for_key;
use crate::view::view;

/// Run the review session to completion (user quit) or error.
pub fn run(state: &mut AppState) -> io::Result<()> {
    install_panic_hook();
    enable_raw_mode()?;
    crossterm::execute!(io::stdout(), EnterAlternateScreen)?;

    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let size = terminal.size()?;
    update(state, Msg::Resize(size.width, size.height));

    let result = event_loop(&mut terminal, state);
    restore_terminal()?;
    result
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut AppState,
) -> io::Result<()> {
    loop {
        terminal.draw(|frame| view(state, frame))?;
        if state.should_quit {
            return Ok(());
        }
        // While highlighting work is pending (budget ran out mid-frame),
        // poll with a short timeout so fill-in frames happen without input;
        // otherwise block on read at zero CPU.
        let event = if state.highlight.has_pending() {
            if crossterm::event::poll(std::time::Duration::from_millis(25))? {
                Some(crossterm::event::read()?)
            } else {
                None // timeout: redraw to let the cache make progress
            }
        } else {
            Some(crossterm::event::read()?)
        };
        match event {
            // Windows terminals also deliver Release/Repeat events;
            // acting on those would double every keystroke.
            Some(Event::Key(key)) if key.kind == KeyEventKind::Press => {
                if let Some(msg) = msg_for_key(key) {
                    update(state, msg);
                }
            }
            Some(Event::Resize(width, height)) => update(state, Msg::Resize(width, height)),
            _ => {}
        }
    }
}

fn restore_terminal() -> io::Result<()> {
    disable_raw_mode()?;
    crossterm::execute!(io::stdout(), LeaveAlternateScreen)?;
    Ok(())
}

/// Restore the terminal before the default panic output, then point at the
/// issue tracker. Installed once; the RAII-less design is deliberate —
/// a hook fires even on panics that unwind past `run`.
fn install_panic_hook() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = restore_terminal();
            default_hook(info);
            eprintln!(
                "margin crashed \u{2014} this is a bug. Please report it:\n\
                 https://github.com/imrajyavardhan12/Margin/issues/new?template=bug_report.yml"
            );
        }));
    });
}
