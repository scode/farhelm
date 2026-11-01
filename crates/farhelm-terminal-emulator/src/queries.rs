//! Answers to the program's terminal queries, and the handler that
//! puts them in front of alacritty's own. The same handler is where the
//! emulator departs from alacritty for tmux's behavior in a few non-query
//! places (see "Beyond queries" below).
//!
//! # The principle
//!
//! Until now a program's queries were answered by tmux, which sits between
//! the program and the browser's xterm.js and answers most of them itself
//! (the rest reach xterm.js). Programs have been tested against those
//! answers. So for each query this emulator answers what tmux 3.7c answers,
//! wherever that answer is true of this emulator, and deviates only where
//! tmux's answer would claim something false about it. The table below was
//! checked against the pinned tmux 3.7c running Farhelm's server
//! configuration, with and without a control-mode client attached (the
//! answers were the same).
//!
//! | Query | Answer | Versus tmux 3.7c |
//! | --- | --- | --- |
//! | DA1 `CSI c`, `CSI 0 c` | `CSI ? 1 ; 2 c` | same (alacritty alone says `CSI ? 6 c`) |
//! | DECID `ESC Z` | `CSI ? 1 ; 2 c` | tmux ignores it; vte reports it as DA1, indistinguishably |
//! | DA2 `CSI > c` | `CSI > 0 ; 2600 ; 1 c` (alacritty's own) | tmux says `CSI > 84 ; 0 ; 0 c`, which claims to be tmux |
//! | XTVERSION `CSI > q`, `CSI > 0 q` | `DCS > \| Farhelm <version> ST` | tmux says `tmux 3.7c` |
//! | DSR 5, DSR 6 (cursor position) | `CSI 0 n`, `CSI row ; col R` | same, including past the last column (below) |
//! | DECRQM `CSI Ps $ p` | see below | same except where noted below |
//! | OSC 10, 11, 12 `?` | the theme's color, or the program's own if it set one | 10 and 11 the same; tmux leaves 12 to xterm.js (below) |
//! | OSC 4 `; n ; ?` | the theme's palette entry, or the program's own | tmux leaves it to xterm.js (below) |
//! | OSC 10 `; ? ; ?` (two at once) | both colors | tmux answers neither; xterm.js answers both |
//! | `CSI 14 t` (text area in pixels) | `CSI 4 ; rows*32 ; cols*16 t` | same: tmux's default cell size, since no client reports one |
//! | `CSI 18 t` (text area in cells) | `CSI 8 ; rows ; cols t` | same |
//! | `CSI 15 t`, `16 t`, `19 t` | none | tmux answers; vte does not parse them |
//! | `CSI ? 996 n` (color scheme) | none | tmux says dark (`CSI ? 997 ; 1 n`); vte does not parse it |
//! | DECRQSS `DCS $ q ... ST` | none | tmux answers; vte ignores every DCS |
//! | kitty keyboard `CSI ? u` | none | same: kitty keyboard mode stays off |
//! | OSC 52 read `; c ; ?` | none | same: clipboard reads are never answered |
//! | `CSI ? 4 m`, `CSI ? 6 n`, DA3, ENQ | none | same |
//!
//! DA2: claiming to be tmux would be false, and it is risky, not just
//! inaccurate. A program that believes it runs in tmux may wrap sequences in
//! tmux's passthrough envelope (`DCS tmux ; ... ST`), which this emulator
//! drops like every DCS: a clipboard write sent that way would be lost.
//! alacritty's own answer (terminal type 0, version 2600 for 0.26.0) is what
//! this emulator is.
//!
//! XTVERSION raises the same question, and here not answering is not an
//! option: Claude Code asks it first, and when it gets no answer it asks
//! nothing further, including DECRQM 2026, so it decides the terminal lacks
//! synchronized output and stops bracketing its redraws (as read from Claude
//! Code 2.1.316's startup probe; vte parses the query only with Farhelm's
//! patch, `vendor/README.md`). The answer names Farhelm, the terminal the
//! user actually sees, with the Farhelm version this was built as
//! ([`crate::VERSION_REPORT`]). It names neither tmux (as above) nor
//! alacritty, whose own window has features this emulator does not, such as
//! the kitty keyboard protocol. An unfamiliar name is enough for Claude Code:
//! with any answer, unless `TERM_PROGRAM` is Apple's Terminal, it goes on to
//! ask DECRQM 2026, and takes a report of set or reset (which this emulator
//! gives) as support for synchronized output; it compares the name itself
//! only with iTerm2, tmux, ghostty, kitty and xterm.js, for other features.
//!
//! A cursor position report while a wrap is pending (the program has just
//! written the last column with autowrap on, and the next character goes to
//! the next line) names the column after the last, as tmux does, where
//! alacritty names the last column. Whether a wrap is pending depends on
//! autowrap when the column was written, not when the report is asked for,
//! as in tmux. Programs reading the position were tested against tmux's
//! answer, which also tells them apart the two states the cursor can be in
//! at that column.
//!
//! The color queries tmux does not answer pass through to xterm.js today,
//! which answers them from the same theme but always ends its answer with ST.
//! This emulator ends every color answer the way the query ended (BEL or ST),
//! as tmux does for the ones it answers.
//!
//! A query inside an open synchronized update is answered when the update
//! ends: at its end marker, at its 150 ms deadline, or when it overflows.
//! tmux answers at once, because it keeps parsing during an update and holds
//! back only drawing (probed: a cursor position query after an update start
//! is answered immediately). vte instead holds the update's bytes unparsed,
//! as alacritty does, so a program that waits for an answer before ending
//! its update waits until the deadline. This is accepted rather than worked
//! around: matching tmux would mean parsing inside the update, a change to
//! vte's buffering. No recorded agent session depends on it: the supervisor's
//! replay of recorded sessions (`emulator_parity`'s `recordings`) feeds each
//! recording to an emulator one byte at a time and fails if an update's end
//! releases any answer, and for the one Claude Code 2.1.316 and one Codex
//! 0.166.0 session recorded, none did.
//!
//! The answers vte cannot see (`CSI 15/16/19 t`, `CSI ? 996 n`, DECRQSS)
//! would each take a vte patch to route the query to a handler. They are left
//! unanswered, a known difference from tmux, because no agent recorded under
//! tmux sent them (Claude Code 2.1.316 and Codex 0.166.0, one session each).
//!
//! # DECRQM
//!
//! A mode report says set (1), reset (2), permanently reset (4) or not
//! recognized (0). This emulator reports the modes alacritty implements as
//! alacritty tracks them, and deviates from alacritty where tmux's answer is
//! the true one here:
//!
//! - 2026 (synchronized output): set while an update is open, as tmux does.
//!   alacritty always says reset, because its handler never sees an open
//!   update: vte runs the update itself.
//! - 3 (132 columns): permanently reset, as tmux does. This emulator never
//!   changes its width; alacritty says not recognized.
//! - 12 (cursor blink): from the cursor style, as alacritty does, but without
//!   alacritty's side effect of turning the default style into an explicit one.
//! - 1007 (alternate scroll) and 1042 (urgency hints): not recognized, as tmux
//!   says. alacritty records them, but nothing in Farhelm acts on them: they
//!   are features of alacritty's own window. The input encoder may claim 1007
//!   once it scrolls that way.
//! - 47 and 1047 (alternate screen variants): the alternate screen's state,
//!   as tmux reports them. alacritty does not implement them; this handler
//!   does (below).
//! - 2031 (color scheme notifications): not recognized. tmux reports it
//!   supported; this emulator never sends the notifications.
//! - ANSI mode 20 (newline): alacritty's report. tmux does not implement it and
//!   says not recognized, which would be false here.
//! - Mode 0, private or not: no answer, as tmux; alacritty answers it as not
//!   recognized.
//!
//! Every other mode, and every mode tmux and alacritty both implement, is
//! answered the same way by both.
//!
//! # Beyond queries
//!
//! Three changes depart from alacritty, each checked against tmux 3.7c:
//!
//! - Resetting any mouse tracking mode (1000, 1001, 1002 or 1003) turns mouse
//!   reporting off, whichever one is on. alacritty clears only the named
//!   mode, so a program that turned on 1003 and then reset 1000 would keep
//!   getting mouse reports, and after it exits its shell would receive them
//!   as typed garbage. The browser's xterm.js, today, also turns reporting
//!   off on a reset of any of 1000, 1002 and 1003.
//! - Modes 47 and 1047 switch to and from the alternate screen, which
//!   alacritty ignores. Like tmux, entering clears the alternate screen and
//!   keeps the cursor where it is; leaving keeps the cursor where the program
//!   left it on the alternate screen and leaves the saved cursor (DECSC)
//!   alone, unlike 1049, which saves and restores the cursor. Mixing these
//!   with 1049 (entering with one and leaving with the other) can leave the
//!   cursor elsewhere than tmux would, since alacritty keeps a cursor per
//!   screen where tmux keeps one.
//! - A full reset (RIS, `ESC c`) that clears a title reports the change
//!   (`Notice::Title`), which alacritty does silently. tmux keeps the title
//!   across RIS; this emulator clears it, as alacritty does.

use alacritty_terminal::Term;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::TermMode;
use alacritty_terminal::vte::ansi::{
    Attr, CharsetIndex, ClearMode, CursorShape, CursorStyle, Handler, Hyperlink, KeyboardModes,
    KeyboardModesApplyBehavior, LineClearMode, Mode, ModifyOtherKeys, NamedMode, NamedPrivateMode,
    PrivateMode, Rgb, ScpCharPath, ScpUpdateMode, StandardCharset, TabulationClearMode,
    cursor_icon::CursorIcon,
};

use crate::effects::{Effect, Listener};
use crate::theme;

/// Primary device attributes: a VT100 with advanced video (`1 ; 2`), what
/// tmux (built without sixel) and xterm.js both answer.
pub(crate) const PRIMARY_DEVICE_ATTRIBUTES: &str = "\x1b[?1;2c";

/// The cell size, in pixels, that `CSI 14 t` assumes: tmux's defaults
/// (`DEFAULT_XPIXEL` and `DEFAULT_YPIXEL`), which is what tmux answers with
/// because Farhelm's control-mode client never reports a pixel size. The
/// emulator does not know the viewer's real cell size either.
pub(crate) const ASSUMED_CELL_PIXELS: (usize, usize) = (16, 32);

/// The parser's view of the terminal for the length of one call into vte:
/// alacritty's [`Term`], with the queries in the table above answered here
/// instead.
///
/// Every [`Handler`] method is spelled out. The trait gives each one an empty
/// default, so a method left out here would silently do nothing rather than
/// fail to compile; `tests/delegation.rs` feeds generated output through
/// this handler and through a bare `Term` and compares the two.
pub(crate) struct QueryHandler<'a> {
    pub(crate) term: &'a mut Term<Listener>,
    /// Where answers go, in order with alacritty's own (see `effects`).
    pub(crate) listener: &'a Listener,
    /// Whether a synchronized update is open, for DECRQM 2026. Lives in the
    /// emulator between calls, because an update opened in one feed is
    /// replayed by vte, queries included, in a later one.
    pub(crate) sync_open: &'a mut bool,
}

impl QueryHandler<'_> {
    fn reply(&self, text: String) {
        self.listener.push(Effect::Reply(text));
    }

    /// Mode 47 or 1047 set: show a cleared alternate screen with the cursor
    /// where it is, keeping the primary screen's saved cursor, which
    /// alacritty's screen swap (made for 1049) overwrites.
    fn enter_alternate_screen(&mut self) {
        if self.term.mode().contains(TermMode::ALT_SCREEN) {
            return;
        }
        let saved = self.term.grid().saved_cursor.clone();
        self.term.swap_alt();
        self.term.inactive_grid_mut().saved_cursor = saved;
    }

    /// Mode 47 or 1047 reset: show the primary screen again, with the cursor
    /// (position and pen) where the program left it on the alternate screen,
    /// rather than where it was when the alternate screen was entered.
    fn leave_alternate_screen(&mut self) {
        if !self.term.mode().contains(TermMode::ALT_SCREEN) {
            return;
        }
        let cursor = self.term.grid().cursor.clone();
        self.term.swap_alt();
        self.term.grid_mut().cursor = cursor;
    }

    /// Turn every mouse tracking mode off, as resetting any one of them does
    /// in tmux and in the browser's xterm.js.
    fn end_mouse_tracking(&mut self) {
        for mode in [
            NamedPrivateMode::ReportMouseClicks,
            NamedPrivateMode::ReportCellMouseMotion,
            NamedPrivateMode::ReportAllMouseMotion,
        ] {
            <Term<Listener> as Handler>::unset_private_mode(self.term, mode.into());
        }
    }

    /// The DECRQM state of a private mode (see the module docs), or `None`
    /// for no answer.
    fn private_mode_state(&self, mode: PrivateMode) -> Option<u8> {
        const NOT_RECOGNIZED: u8 = 0;
        const PERMANENTLY_RESET: u8 = 4;
        let set_or_reset = |set: bool| if set { 1 } else { 2 };
        let flag = |flag: TermMode| set_or_reset(self.term.mode().contains(flag));
        let named = match mode {
            PrivateMode::Named(named) => named,
            PrivateMode::Unknown(0) => return None,
            PrivateMode::Unknown(47 | 1047) => return Some(flag(TermMode::ALT_SCREEN)),
            PrivateMode::Unknown(_) => return Some(NOT_RECOGNIZED),
        };
        Some(match named {
            NamedPrivateMode::CursorKeys => flag(TermMode::APP_CURSOR),
            NamedPrivateMode::ColumnMode => PERMANENTLY_RESET,
            NamedPrivateMode::Origin => flag(TermMode::ORIGIN),
            NamedPrivateMode::LineWrap => flag(TermMode::LINE_WRAP),
            NamedPrivateMode::BlinkingCursor => set_or_reset(self.term.cursor_style().blinking),
            NamedPrivateMode::ShowCursor => flag(TermMode::SHOW_CURSOR),
            NamedPrivateMode::ReportMouseClicks => flag(TermMode::MOUSE_REPORT_CLICK),
            NamedPrivateMode::ReportCellMouseMotion => flag(TermMode::MOUSE_DRAG),
            NamedPrivateMode::ReportAllMouseMotion => flag(TermMode::MOUSE_MOTION),
            NamedPrivateMode::ReportFocusInOut => flag(TermMode::FOCUS_IN_OUT),
            NamedPrivateMode::Utf8Mouse => flag(TermMode::UTF8_MOUSE),
            NamedPrivateMode::SgrMouse => flag(TermMode::SGR_MOUSE),
            NamedPrivateMode::AlternateScroll | NamedPrivateMode::UrgencyHints => NOT_RECOGNIZED,
            NamedPrivateMode::SwapScreenAndSetRestoreCursor => flag(TermMode::ALT_SCREEN),
            NamedPrivateMode::BracketedPaste => flag(TermMode::BRACKETED_PASTE),
            NamedPrivateMode::SyncUpdate => set_or_reset(*self.sync_open),
        })
    }

    /// The DECRQM state of an ANSI mode, or `None` for no answer.
    fn mode_state(&self, mode: Mode) -> Option<u8> {
        let set_or_reset = |flag: TermMode| {
            if self.term.mode().contains(flag) {
                1
            } else {
                2
            }
        };
        match mode {
            Mode::Named(NamedMode::Insert) => Some(set_or_reset(TermMode::INSERT)),
            Mode::Named(NamedMode::LineFeedNewLine) => {
                Some(set_or_reset(TermMode::LINE_FEED_NEW_LINE))
            }
            Mode::Unknown(0) => None,
            Mode::Unknown(_) => Some(0),
        }
    }
}

/// Forward each listed [`Handler`] method to alacritty's `Term` unchanged.
macro_rules! delegate {
    ($(fn $name:ident(&mut self $(, $arg:ident: $ty:ty)*);)*) => {
        $(
            #[inline]
            fn $name(&mut self $(, $arg: $ty)*) {
                <Term<Listener> as Handler>::$name(self.term $(, $arg)*)
            }
        )*
    };
}

impl Handler for QueryHandler<'_> {
    delegate! {
        fn set_title(&mut self, title: Option<String>);
        fn set_cursor_style(&mut self, style: Option<CursorStyle>);
        fn set_cursor_shape(&mut self, shape: CursorShape);
        fn input(&mut self, c: char);
        fn goto(&mut self, line: i32, col: usize);
        fn goto_line(&mut self, line: i32);
        fn goto_col(&mut self, col: usize);
        fn insert_blank(&mut self, count: usize);
        fn move_up(&mut self, rows: usize);
        fn move_down(&mut self, rows: usize);
        fn move_forward(&mut self, cols: usize);
        fn move_backward(&mut self, cols: usize);
        fn move_down_and_cr(&mut self, rows: usize);
        fn move_up_and_cr(&mut self, rows: usize);
        fn put_tab(&mut self, count: u16);
        fn backspace(&mut self);
        fn carriage_return(&mut self);
        fn linefeed(&mut self);
        fn bell(&mut self);
        fn substitute(&mut self);
        fn newline(&mut self);
        fn set_horizontal_tabstop(&mut self);
        fn scroll_up(&mut self, rows: usize);
        fn scroll_down(&mut self, rows: usize);
        fn insert_blank_lines(&mut self, count: usize);
        fn delete_lines(&mut self, count: usize);
        fn erase_chars(&mut self, count: usize);
        fn delete_chars(&mut self, count: usize);
        fn move_backward_tabs(&mut self, count: u16);
        fn move_forward_tabs(&mut self, count: u16);
        fn save_cursor_position(&mut self);
        fn restore_cursor_position(&mut self);
        fn clear_line(&mut self, mode: LineClearMode);
        fn clear_screen(&mut self, mode: ClearMode);
        fn clear_tabs(&mut self, mode: TabulationClearMode);
        fn set_tabs(&mut self, interval: u16);
        fn reverse_index(&mut self);
        fn terminal_attribute(&mut self, attr: Attr);
        fn set_mode(&mut self, mode: Mode);
        fn unset_mode(&mut self, mode: Mode);
        fn set_scrolling_region(&mut self, top: usize, bottom: Option<usize>);
        fn set_keypad_application_mode(&mut self);
        fn unset_keypad_application_mode(&mut self);
        fn set_active_charset(&mut self, index: CharsetIndex);
        fn configure_charset(&mut self, index: CharsetIndex, charset: StandardCharset);
        fn set_color(&mut self, index: usize, color: Rgb);
        fn reset_color(&mut self, index: usize);
        fn clipboard_store(&mut self, clipboard: u8, base64: &[u8]);
        // Never answered: alacritty refuses clipboard reads under the
        // copy-only OSC 52 setting `Emulator::new` fixes.
        fn clipboard_load(&mut self, clipboard: u8, terminator: &str);
        fn decaln(&mut self);
        fn push_title(&mut self);
        fn pop_title(&mut self);
        fn text_area_size_chars(&mut self);
        fn set_hyperlink(&mut self, link: Option<Hyperlink>);
        fn set_mouse_cursor_icon(&mut self, icon: CursorIcon);
        // The kitty keyboard protocol is off (`Emulator::new`), so alacritty
        // ignores these four.
        fn report_keyboard_mode(&mut self);
        fn push_keyboard_mode(&mut self, mode: KeyboardModes);
        fn pop_keyboard_modes(&mut self, to_pop: u16);
        fn set_keyboard_mode(&mut self, mode: KeyboardModes, behavior: KeyboardModesApplyBehavior);
        fn set_modify_other_keys(&mut self, mode: ModifyOtherKeys);
        fn report_modify_other_keys(&mut self);
        fn set_scp(&mut self, char_path: ScpCharPath, update_mode: ScpUpdateMode);
    }

    /// DA1 (and DECID, which vte reports the same way) as tmux answers it;
    /// DA2 as alacritty does; anything else unanswered, as both do.
    fn identify_terminal(&mut self, intermediate: Option<char>) {
        match intermediate {
            None => self.reply(PRIMARY_DEVICE_ATTRIBUTES.to_owned()),
            Some(_) => <Term<Listener> as Handler>::identify_terminal(self.term, intermediate),
        }
    }

    /// DSR, with the cursor position past the last column while a wrap is
    /// pending (see the module docs).
    fn device_status(&mut self, arg: usize) {
        // The vendored alacritty's wrap flag stands for tmux's cursor one
        // past the last column: `Term::input` raises it at the last column
        // only with autowrap on, where tmux moves past the column
        // (vendor/README.md).
        let cursor = &self.term.grid().cursor;
        if arg == 6 && cursor.input_needs_wrap {
            let (line, column) = (cursor.point.line.0 + 1, cursor.point.column.0 + 2);
            self.reply(format!("\x1b[{line};{column}R"));
            return;
        }
        <Term<Listener> as Handler>::device_status(self.term, arg);
    }

    /// XTVERSION, which alacritty leaves unanswered (see the module docs).
    fn report_version(&mut self) {
        self.reply(crate::VERSION_REPORT.to_owned());
    }

    fn report_mode(&mut self, mode: Mode) {
        if let Some(state) = self.mode_state(mode) {
            self.reply(format!("\x1b[{};{state}$y", mode.raw()));
        }
    }

    fn report_private_mode(&mut self, mode: PrivateMode) {
        if let Some(state) = self.private_mode_state(mode) {
            self.reply(format!("\x1b[?{};{state}$y", mode.raw()));
        }
    }

    /// Track an open synchronized update for DECRQM, and enter the
    /// alternate screen for 47 and 1047 (module docs, "Beyond queries");
    /// alacritty sets every other mode as usual.
    fn set_private_mode(&mut self, mode: PrivateMode) {
        match mode {
            PrivateMode::Named(NamedPrivateMode::SyncUpdate) => *self.sync_open = true,
            PrivateMode::Unknown(47 | 1047) => return self.enter_alternate_screen(),
            _ => {}
        }
        <Term<Listener> as Handler>::set_private_mode(self.term, mode);
    }

    /// The counterpart of [`Self::set_private_mode`], plus tmux's rule that
    /// resetting any mouse tracking mode ends mouse reporting. vte calls this
    /// for 2026 both on the program's own end marker and when it ends an
    /// update itself (timeout, overflow).
    fn unset_private_mode(&mut self, mode: PrivateMode) {
        const TRACKING: [NamedPrivateMode; 3] = [
            NamedPrivateMode::ReportMouseClicks,
            NamedPrivateMode::ReportCellMouseMotion,
            NamedPrivateMode::ReportAllMouseMotion,
        ];
        match mode {
            PrivateMode::Named(NamedPrivateMode::SyncUpdate) => *self.sync_open = false,
            PrivateMode::Unknown(47 | 1047) => return self.leave_alternate_screen(),
            PrivateMode::Named(named) if TRACKING.contains(&named) => {
                return self.end_mouse_tracking();
            }
            // Highlight tracking, which neither alacritty nor tmux
            // implements; tmux still ends tracking when it is reset.
            PrivateMode::Unknown(1001) => return self.end_mouse_tracking(),
            _ => {}
        }
        <Term<Listener> as Handler>::unset_private_mode(self.term, mode);
    }

    /// RIS, telling the owner when it clears a title (module docs, "Beyond
    /// queries").
    fn reset_state(&mut self) {
        let had_title = self.term.title().is_some();
        <Term<Listener> as Handler>::reset_state(self.term);
        if had_title {
            self.listener.push(Effect::Title);
        }
    }

    /// Answer a color query from the program's own color if it set one, else
    /// from Farhelm's theme, in the same format tmux and alacritty use
    /// (16 bits per channel, the terminator the query used).
    ///
    /// Answered here rather than through alacritty's `ColorRequest` event
    /// because the event is handled after the whole feed: a program that asks
    /// and then changes the color in one write would get the new color.
    fn dynamic_color_sequence(&mut self, prefix: String, index: usize, terminator: &str) {
        let Some(Rgb { r, g, b }) = self.term.colors()[index].or_else(|| theme::color(index))
        else {
            return;
        };
        self.reply(format!(
            "\x1b]{prefix};rgb:{r:02x}{r:02x}/{g:02x}{g:02x}/{b:02x}{b:02x}{terminator}"
        ));
    }

    /// `CSI 14 t`, with tmux's assumed cell size ([`ASSUMED_CELL_PIXELS`]).
    fn text_area_size_pixels(&mut self) {
        let (cell_width, cell_height) = ASSUMED_CELL_PIXELS;
        let height = self.term.screen_lines() * cell_height;
        let width = self.term.columns() * cell_width;
        self.reply(format!("\x1b[4;{height};{width}t"));
    }
}
