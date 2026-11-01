//! The query handler changes answers only: fed the same output, the emulator
//! ends in the same terminal state as a bare alacritty `Term` with the same
//! configuration, except where it deliberately follows tmux instead.
//!
//! The emulator puts its own handler between vte and alacritty so that it can
//! answer queries its way. vte's handler trait gives every method an empty
//! default, so a method the handler forgot to forward would not fail to
//! compile; it would silently drop whatever the program asked for, from a
//! cursor movement to a mode change. Comparing against an unwrapped `Term`
//! on generated output catches that. The output leaves out the mode changes
//! where the emulator departs from alacritty on purpose (mouse tracking
//! resets, modes 47 and 1047); `tests/modes.rs` and `tests/screen.rs` cover
//! those against tmux's behavior.
//!
//! Both are fed the output in one call, since inside a synchronized update
//! vte's result can depend on how the output is cut into calls.

mod common;

use common::output::alacritty_stream;
use common::{emulator, feed};
use farhelm_terminal_emulator::alacritty_terminal::Term;
use farhelm_terminal_emulator::alacritty_terminal::event::VoidListener;
use farhelm_terminal_emulator::alacritty_terminal::grid::{Dimensions, Grid};
use farhelm_terminal_emulator::alacritty_terminal::index::Line;
use farhelm_terminal_emulator::alacritty_terminal::term::cell::Cell;
use farhelm_terminal_emulator::alacritty_terminal::term::color::COUNT;
use farhelm_terminal_emulator::alacritty_terminal::term::{Config, Osc52};
use farhelm_terminal_emulator::alacritty_terminal::vte::ansi::Processor;
use farhelm_terminal_emulator::{Collected, FeedMode};
use proptest::prelude::*;

/// alacritty's `Dimensions` for the bare terminal, matching
/// `common::emulator(20, 5)`.
struct Size;

impl Dimensions for Size {
    fn total_lines(&self) -> usize {
        5
    }

    fn screen_lines(&self) -> usize {
        5
    }

    fn columns(&self) -> usize {
        20
    }
}

/// Every piece of terminal state a program can change and a viewer or a
/// later query can observe, compared field by field, so that a failure names
/// the field.
fn assert_same_state<A, B>(ours: &Term<A>, bare: &Term<B>) -> Result<(), TestCaseError> {
    assert_same_grid("active", ours.grid(), bare.grid())?;
    assert_same_grid("inactive", ours.inactive_grid(), bare.inactive_grid())?;
    prop_assert_eq!(ours.mode(), bare.mode());
    prop_assert_eq!(ours.title(), bare.title());
    prop_assert_eq!(ours.title_stack(), bare.title_stack());
    prop_assert_eq!(ours.cursor_style(), bare.cursor_style());
    prop_assert_eq!(ours.scroll_region(), bare.scroll_region());
    prop_assert_eq!(ours.tab_stops(), bare.tab_stops());
    prop_assert_eq!(ours.active_charset(), bare.active_charset());
    prop_assert_eq!(ours.keyboard_mode_stacks(), bare.keyboard_mode_stacks());
    for index in 0..COUNT {
        prop_assert_eq!(
            ours.colors()[index],
            bare.colors()[index],
            "color {}",
            index
        );
    }
    Ok(())
}

/// Two grids hold the same rows, history included, and the same cursors.
///
/// Row by row because `Grid`'s own equality asserts that neither grid's ring
/// buffer has rotated, which any output that scrolls breaks.
fn assert_same_grid(name: &str, ours: &Grid<Cell>, bare: &Grid<Cell>) -> Result<(), TestCaseError> {
    prop_assert_eq!(ours.history_size(), bare.history_size(), "{} history", name);
    for line in -(ours.history_size() as i32)..ours.screen_lines() as i32 {
        prop_assert!(
            ours[Line(line)] == bare[Line(line)],
            "{} line {}",
            name,
            line
        );
    }
    prop_assert_eq!(&ours.cursor, &bare.cursor, "{} cursor", name);
    prop_assert_eq!(
        &ours.saved_cursor,
        &bare.saved_cursor,
        "{} saved cursor",
        name
    );
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// The emulator and a bare `Term` end in the same state after the same
    /// output, including the contents of an update left open (flushed on
    /// both before comparing).
    ///
    /// Why: see the module docs; this is what makes the query handler's
    /// hand-written forwarding safe to trust. Spec: `QueryHandler` in
    /// `src/queries.rs`.
    #[test]
    fn the_emulator_changes_no_state_a_bare_term_would_not(output in alacritty_stream(200)) {
        let mut ours = emulator(20, 5);
        feed(&mut ours, &output);
        ours.flush_sync(FeedMode::Live, &mut Collected::default());

        let config = Config {
            scrolling_history: 1000,
            kitty_keyboard: false,
            osc52: Osc52::OnlyCopy,
            ..Config::default()
        };
        let mut bare = Term::new(config, &Size, VoidListener);
        let mut parser: Processor = Processor::new();
        parser.advance(&mut bare, &output);
        parser.stop_sync(&mut bare);

        assert_same_state(ours.term(), &bare)?;
    }
}
