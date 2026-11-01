//! Terminals one column wide, and a zero size.
//!
//! One column is a valid terminal size (the host accepts dimensions from 1),
//! and the emulator stays the same size as the host's pty, so it cannot
//! widen itself to make room. alacritty assumes room for a wide character's
//! second cell: printing one at one column indexed past the row and
//! panicked, and resizing a screen holding wide characters to one column
//! reflowed forever, allocating rows as it went. Vendored patches drop the
//! character and blank it on such a reflow (vendor/README.md); these tests
//! hold the emulator to that through its own API, where ordinary program
//! output reaches it.
//!
//! Every test runs its work through [`bounded`], so that a regression of the
//! reflow fix fails the test after 10 seconds instead of running until
//! memory runs out. The timer bounds time, not memory: the endless reflow
//! allocates as it goes, about 1 GB in those 10 seconds in a debug build
//! (measured), and a machine with less free memory than that can still run
//! out before the test fails. A real memory bound would need the test in a
//! process of its own with a limit set on it.

mod common;

use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use common::output::stream;
use common::{emulator, feed, resize};
use farhelm_terminal_emulator::{Emulator, Size};
use proptest::prelude::*;

/// How long one bounded piece of work may take. The work itself takes
/// milliseconds; this only has to be far above that on a loaded machine.
const LIMIT: Duration = Duration::from_secs(10);

/// Run `work` on a thread of its own and return its result, or fail if it
/// takes longer than [`LIMIT`].
///
/// On a timeout the thread is left running and the test panics; nextest runs
/// each test in a process of its own, so the process and the runaway thread
/// end with the failure. A panic in `work` is passed on as is.
fn bounded<T: Send + 'static>(what: &str, work: impl FnOnce() -> T + Send + 'static) -> T {
    let (sender, receiver) = mpsc::channel();
    let worker = thread::spawn(move || {
        let _ = sender.send(work());
    });
    match receiver.recv_timeout(LIMIT) {
        Ok(result) => result,
        Err(RecvTimeoutError::Timeout) => panic!(
            "{what} did not finish within {LIMIT:?}: the one-column reflow fix \
             (vendor/README.md, \"Bug fixes\") may have regressed into its \
             endless, allocating loop"
        ),
        Err(RecvTimeoutError::Disconnected) => match worker.join() {
            Err(panic) => std::panic::resume_unwind(panic),
            Ok(()) => unreachable!("the worker sends before it returns"),
        },
    }
}

/// A one-column terminal prints narrow text around a wide character and
/// drops the wide character, with wrapping on and off.
///
/// Why: this panicked before the vendored fix, on output any program can
/// print. Spec: `Size`, vendor/README.md.
#[test]
fn a_one_column_terminal_drops_wide_characters() {
    let (wrapping, not_wrapping) = bounded("printing on one column", || {
        let mut wrapping = emulator(1, 3);
        feed(&mut wrapping, "a\u{4e2d}b\u{1f600}".as_bytes());
        let mut not_wrapping = emulator(1, 3);
        feed(&mut not_wrapping, "\x1b[?7la\u{4e2d}b".as_bytes());
        (wrapping.screen_text(), not_wrapping.screen_text())
    });
    assert_eq!(wrapping, "a\nb\n\n");
    assert_eq!(not_wrapping, "b\n\n\n");
}

/// Resizing a screen that holds wide characters to one column, printing,
/// and resizing back works, on both screens.
///
/// Why: the reflow to one column looped forever, allocating rows, before
/// the vendored fix. Spec: `Size`, vendor/README.md.
#[test]
fn resizing_to_one_column_with_wide_characters() {
    let (text, size) = bounded("resizing to one column", || {
        let mut emulator = emulator(6, 3);
        feed(&mut emulator, "\u{4e2d}\u{6587}x\r\n\u{4e2d}".as_bytes());
        resize(&mut emulator, 1, 3);
        feed(&mut emulator, "\u{4e2d}y".as_bytes());
        let text = emulator.screen_text();
        feed(&mut emulator, "\x1b[?1049h\u{4e2d}\u{4e2d}".as_bytes());
        resize(&mut emulator, 4, 3);
        feed(&mut emulator, "\u{4e2d}z\x1b[?1049l".as_bytes());
        resize(&mut emulator, 1, 2);
        (text, emulator.size())
    });
    assert!(text.contains('y'), "{text:?}");
    assert_eq!(
        size,
        Size {
            columns: 1,
            lines: 2
        }
    );
}

/// A zero size, from construction or a resize, becomes one cell each way,
/// and wide text then behaves as on any one-column terminal.
///
/// Why: alacritty's grid needs at least one cell; the host never sends
/// zero, but a zero must not panic the supervisor. Spec: `Size`.
#[test]
fn a_zero_size_becomes_one_cell() {
    let (first_size, text, second_size) = bounded("a zero size", || {
        let mut emulator = Emulator::new(
            Size {
                columns: 0,
                lines: 0,
            },
            10,
        );
        let first_size = emulator.size();
        feed(&mut emulator, "\u{4e2d}a".as_bytes());
        let text = emulator.screen_text();
        resize(&mut emulator, 0, 5);
        (first_size, text, emulator.size())
    });
    assert_eq!(
        first_size,
        Size {
            columns: 1,
            lines: 1
        }
    );
    assert_eq!(text, "a\n");
    assert_eq!(
        second_size,
        Size {
            columns: 1,
            lines: 5
        }
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// Any output on terminals one to three columns wide, with resizes
    /// among those widths between the pieces, completes without a panic.
    ///
    /// Why: narrow grids are where alacritty's assumptions about room for
    /// wide characters and their spacers break; generated output reaches the
    /// combinations the hand-written cases above do not. Spec: `Size`.
    #[test]
    fn narrow_terminals_survive_any_output(
        pieces in prop::collection::vec((stream(30), 1u16..4, 1u16..4), 1..6),
    ) {
        bounded("generated output on narrow terminals", move || {
            let mut emulator = emulator(1, 2);
            for (output, columns, lines) in pieces {
                feed(&mut emulator, &output);
                feed(&mut emulator, "\u{4e2d}\u{1f600}".as_bytes());
                resize(&mut emulator, columns, lines);
                let _ = emulator.screen_text();
            }
        });
    }
}
