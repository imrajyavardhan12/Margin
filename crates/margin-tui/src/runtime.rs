//! The effectful shell around the pure core: terminal setup/teardown,
//! the event loop, and the panic guard (ADR-0009: a crash must never
//! leave the user's terminal in raw mode).

use std::io;
use std::sync::{Mutex, Once};
use std::time::{Duration, Instant};

use crossterm::event::{DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::app::{update, AppState, CommandExecutor, Msg};
use crate::keymap::{msg_for_key, msg_for_mouse};
use crate::view::view;

/// Debounced "the world changed" signal for watch mode (issue #12).
///
/// The binary's file-system watcher calls [`WatchHandle::notify`] from its
/// event thread; the event loop polls [`WatchHandle::take_due`] and issues
/// one `Msg::Reload` — the same message as `r`, so auto-reload is the
/// existing `DiffSource` capability, not a TUI special case — once a quiet
/// window has passed since the *last* event. Rapid agent writes collapse
/// into a single reload: no storms.
///
/// std-only by design: margin-tui knows nothing about how events are
/// produced (same inversion as `CommandExecutor`).
pub struct WatchHandle {
    /// (first event of the pending burst, most recent event).
    pending: Mutex<Option<(Instant, Instant)>>,
    window: Duration,
}

/// A sustained write storm (every event inside the quiet window) re-arms
/// the debounce forever; after this many windows of continuous activity
/// the reload fires anyway, so a long-running agent never starves the
/// review (post-M2 review finding).
const MAX_WAIT_WINDOWS: u32 = 8;

impl WatchHandle {
    pub fn new(window: Duration) -> Self {
        Self {
            pending: Mutex::new(None),
            window,
        }
    }

    /// Record a file-system event (called from the watcher thread).
    pub fn notify(&self) {
        self.notify_at(Instant::now());
    }

    /// Consume the pending signal once the quiet window has elapsed —
    /// or the max wait, whichever comes first.
    pub fn take_due(&self) -> bool {
        self.take_due_at(Instant::now())
    }

    fn notify_at(&self, at: Instant) {
        if let Ok(mut pending) = self.pending.lock() {
            let first = pending.map_or(at, |(first, _)| first);
            *pending = Some((first, at));
        }
    }

    fn take_due_at(&self, now: Instant) -> bool {
        let Ok(mut pending) = self.pending.lock() else {
            return false;
        };
        match *pending {
            Some((first, last))
                if now.duration_since(last) >= self.window
                    || now.duration_since(first) >= self.window * MAX_WAIT_WINDOWS =>
            {
                *pending = None;
                true
            }
            _ => false,
        }
    }
}

/// A terminal mode transition owned by [`TerminalSession`]. One private
/// operation Interface is enough to inject failures without abstracting
/// ratatui or proliferating setup/cleanup methods.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TerminalAction {
    EnableRaw,
    EnterAlternate,
    EnableMouse,
    DisableMouse,
    LeaveAlternate,
    DisableRaw,
}

trait TerminalControl {
    fn perform(&mut self, action: TerminalAction) -> io::Result<()>;
}

struct CrosstermControl;

impl TerminalControl for CrosstermControl {
    fn perform(&mut self, action: TerminalAction) -> io::Result<()> {
        match action {
            TerminalAction::EnableRaw => enable_raw_mode(),
            TerminalAction::EnterAlternate => {
                crossterm::execute!(io::stdout(), EnterAlternateScreen)
            }
            TerminalAction::EnableMouse => {
                crossterm::execute!(io::stdout(), EnableMouseCapture)
            }
            TerminalAction::DisableMouse => {
                crossterm::execute!(io::stdout(), DisableMouseCapture)
            }
            TerminalAction::LeaveAlternate => {
                crossterm::execute!(io::stdout(), LeaveAlternateScreen)
            }
            TerminalAction::DisableRaw => disable_raw_mode(),
        }
    }
}

/// Scope-bound ownership of every terminal mode Margin enables.
///
/// A flag is set *before* its setup operation. Terminal writes can fail after
/// partially taking effect, so an error still triggers the matching best-effort
/// cleanup. Explicit finish reports cleanup errors; unwinding uses `Drop`.
struct TerminalSession<'a> {
    control: &'a mut dyn TerminalControl,
    raw: bool,
    alternate: bool,
    mouse: bool,
}

impl<'a> TerminalSession<'a> {
    fn start(control: &'a mut dyn TerminalControl, mouse: bool) -> io::Result<Self> {
        let mut session = Self {
            control,
            raw: false,
            alternate: false,
            mouse: false,
        };

        session.raw = true;
        session.control.perform(TerminalAction::EnableRaw)?;
        session.alternate = true;
        session.control.perform(TerminalAction::EnterAlternate)?;
        if mouse {
            // Wheel + click (issue #26); `mouse = false` in config opts out
            // and keeps the terminal's own selection behavior everywhere.
            session.mouse = true;
            session.control.perform(TerminalAction::EnableMouse)?;
        }
        Ok(session)
    }

    /// Attempt every applicable cleanup operation, retaining the first error.
    fn restore(&mut self) -> io::Result<()> {
        let mut first_error = None;
        if self.mouse {
            self.mouse = false;
            remember_error(
                &mut first_error,
                self.control.perform(TerminalAction::DisableMouse),
            );
        }
        if self.alternate {
            self.alternate = false;
            remember_error(
                &mut first_error,
                self.control.perform(TerminalAction::LeaveAlternate),
            );
        }
        if self.raw {
            self.raw = false;
            remember_error(
                &mut first_error,
                self.control.perform(TerminalAction::DisableRaw),
            );
        }
        first_error.map_or(Ok(()), Err)
    }

    /// Preserve the runtime error when both work and restoration fail.
    fn finish<T>(mut self, result: io::Result<T>) -> io::Result<T> {
        let cleanup = self.restore();
        match result {
            Err(primary) => Err(primary),
            Ok(value) => cleanup.map(|()| value),
        }
    }
}

impl Drop for TerminalSession<'_> {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

fn remember_error(first: &mut Option<io::Error>, result: io::Result<()>) {
    if let Err(error) = result {
        if first.is_none() {
            *first = Some(error);
        }
    }
}

/// Execute work while a fully or partially configured terminal is owned by a
/// guard. Every fallible ratatui/event-loop operation belongs inside `body`.
fn with_terminal_session<T>(
    control: &mut dyn TerminalControl,
    mouse: bool,
    body: impl FnOnce() -> io::Result<T>,
) -> io::Result<T> {
    let session = TerminalSession::start(control, mouse)?;
    let result = body();
    session.finish(result)
}

/// Run the review session to completion (user quit) or error. The
/// executor performs any side effects `update` requests (ADR-0013);
/// `watch`, when present, feeds debounced reloads (issue #12).
pub fn run(
    state: &mut AppState,
    executor: &mut dyn CommandExecutor,
    watch: Option<&WatchHandle>,
    mouse: bool,
) -> io::Result<()> {
    install_panic_hook();
    let mut control = CrosstermControl;
    with_terminal_session(&mut control, mouse, || {
        let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
        let size = terminal.size()?;
        update(state, Msg::Resize(size.width, size.height));
        event_loop(&mut terminal, state, executor, watch)
    })
}

/// One message through the core; any requested effect executes and its
/// outcome feeds straight back in as a message (the command loop).
fn dispatch(state: &mut AppState, msg: Msg, executor: &mut dyn CommandExecutor) {
    if let Some(command) = update(state, msg) {
        let result = executor.execute(command);
        update(state, Msg::CommandFinished(result));
    }
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut AppState,
    executor: &mut dyn CommandExecutor,
    watch: Option<&WatchHandle>,
) -> io::Result<()> {
    // Draw only when something changed (or highlight fill-in is owed):
    // watch mode wakes every 100ms to check the debounce, and re-rendering
    // an unchanged frame at 10Hz would waste idle CPU for nothing.
    let mut needs_draw = true;
    loop {
        if needs_draw || state.highlight.has_pending() {
            terminal.draw(|frame| view(state, frame))?;
            needs_draw = false;
        }
        if state.should_quit {
            return Ok(());
        }
        // Pick how long to wait for input. Pending highlight work wants
        // fast fill-in frames; watch mode needs periodic wake-ups to check
        // the debounce; otherwise block on read at zero CPU.
        let timeout = if state.highlight.has_pending() {
            Some(Duration::from_millis(25))
        } else if watch.is_some() {
            Some(Duration::from_millis(100))
        } else {
            None
        };
        let event = match timeout {
            Some(timeout) => {
                if crossterm::event::poll(timeout)? {
                    Some(crossterm::event::read()?)
                } else {
                    None // timeout: fall through to the watch check
                }
            }
            None => Some(crossterm::event::read()?),
        };
        match event {
            // Windows terminals also deliver Release/Repeat events;
            // acting on those would double every keystroke.
            Some(Event::Key(key)) if key.kind == KeyEventKind::Press => {
                if let Some(msg) = msg_for_key(key, state.input_mode()) {
                    dispatch(state, msg, executor);
                    needs_draw = true;
                }
            }
            Some(Event::Resize(width, height)) => {
                dispatch(state, Msg::Resize(width, height), executor);
                needs_draw = true;
            }
            Some(Event::Mouse(mouse)) => {
                if let Some(msg) = msg_for_mouse(mouse) {
                    dispatch(state, msg, executor);
                    needs_draw = true;
                }
            }
            _ => {}
        }
        // After input (or a tick): a debounced watch signal becomes the
        // same reload `r` performs. Skipped while a modal overlay is open —
        // a reload must never yank the world out from under a typed
        // confirmation or a picker mid-choice (the signal keeps
        // accumulating and fires once the overlay closes).
        if let Some(handle) = watch {
            if state.confirm.is_none() && state.picker.is_none() && handle.take_due() {
                dispatch(state, Msg::Reload, executor);
                needs_draw = true;
            }
        }
    }
}

/// Best-effort unconditional restoration for panic paths. The scope guard owns
/// normal errors and unwinding; this fallback also runs with `panic = abort`,
/// when destructors do not run. Every operation is attempted independently.
fn restore_terminal() -> io::Result<()> {
    let mut control = CrosstermControl;
    let mut first_error = None;
    remember_error(
        &mut first_error,
        control.perform(TerminalAction::DisableMouse),
    );
    remember_error(
        &mut first_error,
        control.perform(TerminalAction::LeaveAlternate),
    );
    remember_error(
        &mut first_error,
        control.perform(TerminalAction::DisableRaw),
    );
    first_error.map_or(Ok(()), Err)
}

/// Restore the terminal before the default panic output, then point at the
/// issue tracker. Installed once; the hook complements the scope guard because
/// it still runs when panic handling aborts instead of unwinding.
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

#[cfg(test)]
mod tests {
    use super::*;

    const WINDOW: Duration = Duration::from_millis(200);

    #[derive(Default)]
    struct FakeTerminal {
        calls: Vec<TerminalAction>,
        fail: Vec<TerminalAction>,
    }

    impl FakeTerminal {
        fn failing(actions: &[TerminalAction]) -> Self {
            Self {
                calls: Vec::new(),
                fail: actions.to_vec(),
            }
        }

        fn perform(&mut self, action: TerminalAction) -> io::Result<()> {
            self.calls.push(action);
            if self.fail.contains(&action) {
                Err(io::Error::other(format!("{action:?} failed")))
            } else {
                Ok(())
            }
        }
    }

    impl TerminalControl for FakeTerminal {
        fn perform(&mut self, action: TerminalAction) -> io::Result<()> {
            FakeTerminal::perform(self, action)
        }
    }

    #[test]
    fn setup_failures_restore_every_mode_that_may_have_changed() {
        let cases = [
            (
                TerminalAction::EnableRaw,
                vec![TerminalAction::EnableRaw, TerminalAction::DisableRaw],
            ),
            (
                TerminalAction::EnterAlternate,
                vec![
                    TerminalAction::EnableRaw,
                    TerminalAction::EnterAlternate,
                    TerminalAction::LeaveAlternate,
                    TerminalAction::DisableRaw,
                ],
            ),
            (
                TerminalAction::EnableMouse,
                vec![
                    TerminalAction::EnableRaw,
                    TerminalAction::EnterAlternate,
                    TerminalAction::EnableMouse,
                    TerminalAction::DisableMouse,
                    TerminalAction::LeaveAlternate,
                    TerminalAction::DisableRaw,
                ],
            ),
        ];

        for (failure, expected) in cases {
            let mut terminal = FakeTerminal::failing(&[failure]);
            {
                let result = TerminalSession::start(&mut terminal, true);
                assert!(result.is_err(), "{failure:?} must fail setup");
            }
            assert_eq!(terminal.calls, expected, "cleanup after {failure:?}");
        }
    }

    #[test]
    fn any_error_after_setup_restores_the_terminal_and_keeps_the_error() {
        // Terminal creation, size queries, drawing, and event reads all execute
        // inside this body in `run`; one injected body failure covers their
        // shared early-return seam.
        let mut terminal = FakeTerminal::failing(&[TerminalAction::DisableMouse]);
        let result: io::Result<()> = with_terminal_session(&mut terminal, true, || {
            Err(io::Error::other("event read failed"))
        });

        let Err(error) = result else {
            panic!("the injected runtime failure must be returned");
        };
        assert_eq!(
            error.to_string(),
            "event read failed",
            "cleanup must not hide the runtime error"
        );
        assert_eq!(
            terminal.calls,
            vec![
                TerminalAction::EnableRaw,
                TerminalAction::EnterAlternate,
                TerminalAction::EnableMouse,
                TerminalAction::DisableMouse,
                TerminalAction::LeaveAlternate,
                TerminalAction::DisableRaw,
            ]
        );
    }

    #[test]
    fn cleanup_attempts_every_step_and_reports_its_first_error() {
        let failures = [
            TerminalAction::DisableMouse,
            TerminalAction::LeaveAlternate,
            TerminalAction::DisableRaw,
        ];
        let mut terminal = FakeTerminal::failing(&failures);
        let result = with_terminal_session(&mut terminal, true, || Ok(()));

        let Err(error) = result else {
            panic!("cleanup failures must be returned");
        };
        assert_eq!(
            error.to_string(),
            "DisableMouse failed",
            "the first cleanup failure is the actionable one"
        );
        assert!(terminal.calls.ends_with(&failures));
    }

    #[test]
    fn unwinding_restores_terminal_modes() {
        let mut terminal = FakeTerminal::default();
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _: io::Result<()> = with_terminal_session(&mut terminal, true, || {
                panic!("injected draw panic");
            });
        }));

        assert!(panic.is_err());
        assert!(terminal.calls.ends_with(&[
            TerminalAction::DisableMouse,
            TerminalAction::LeaveAlternate,
            TerminalAction::DisableRaw,
        ]));
    }

    #[test]
    fn debounce_waits_for_quiet_then_fires_once() {
        let handle = WatchHandle::new(WINDOW);
        let t0 = Instant::now();
        assert!(!handle.take_due_at(t0), "no events yet");

        handle.notify_at(t0);
        assert!(
            !handle.take_due_at(t0 + Duration::from_millis(100)),
            "inside the quiet window"
        );
        assert!(
            handle.take_due_at(t0 + Duration::from_millis(200)),
            "window elapsed"
        );
        assert!(
            !handle.take_due_at(t0 + Duration::from_millis(400)),
            "signal is consumed: one reload per burst"
        );
    }

    #[test]
    fn rapid_writes_collapse_into_one_reload() {
        let handle = WatchHandle::new(WINDOW);
        let t0 = Instant::now();
        // An agent writing every 50ms for a second: each event re-arms
        // the window, so nothing fires mid-storm...
        let mut fired = 0;
        for i in 0..20 {
            let at = t0 + Duration::from_millis(50 * i);
            handle.notify_at(at);
            if handle.take_due_at(at + Duration::from_millis(50)) {
                fired += 1;
            }
        }
        assert_eq!(fired, 0, "no reload storms during rapid writes");
        // ...and quiescence yields exactly one reload.
        let quiet = t0 + Duration::from_millis(50 * 19 + 200);
        assert!(handle.take_due_at(quiet));
        assert!(!handle.take_due_at(quiet + WINDOW));
    }

    #[test]
    fn a_sustained_storm_cannot_starve_the_reload() {
        // An agent writing every 50ms for minutes: the quiet window never
        // elapses, but the max wait (8 windows = 1.6s here) fires anyway.
        let handle = WatchHandle::new(WINDOW);
        let t0 = Instant::now();
        let mut fired_at = None;
        for i in 0..100 {
            let at = t0 + Duration::from_millis(50 * i);
            handle.notify_at(at);
            if handle.take_due_at(at) {
                fired_at = Some(at);
                break;
            }
        }
        let Some(at) = fired_at else {
            panic!("the cap must fire mid-storm");
        };
        let waited = at.duration_since(t0);
        assert!(
            waited >= WINDOW * 8 && waited < WINDOW * 9,
            "fired after {waited:?}, expected ~8 windows"
        );
        // The burst tracker reset: the next event starts a fresh window.
        handle.notify_at(at + Duration::from_millis(10));
        assert!(!handle.take_due_at(at + Duration::from_millis(20)));
    }
}
