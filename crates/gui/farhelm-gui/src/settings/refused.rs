//! A press that sent nothing because its key already had a save in flight, said beside the control that was pressed.
//!
//! Settings and the host settings dialog save through the operation store, which admits one request per key and queues
//! nothing ([`NotStarted::Busy`]). Every Settings save holds the preference row's key, and every save in a host's
//! dialog holds that host's, so a press that meets another save on its key sends nothing. The old app never let such a
//! press look like it worked: Settings queued the newest choice per field, and the host dialog disabled its controls
//! while a write ran. Here the control says why in its own failure line's place ([`Refused::line`], worded
//! [`Refused::WORDS`]), rather than every control on the key being drawn unavailable for the few milliseconds each save
//! takes. The migration's orchestrator asked for one of the two on 2026-11-14; this area chose the line, which needs no
//! change to the shared controls and keeps focus where the user put it.
//!
//! One [`Refused`] per view (the Settings dialog, a checkout field, the ntfy form, a host's dialog) remembers the one
//! control whose press was refused last, so the line explains only the latest refused press. It stays until a save from
//! that view starts, which shows the key was free again, or another press in it is refused. The blocking save usually
//! ends long before that, so the words are in the past tense: they say what happened to the press, not what is running
//! now.
//!
//! The words are this area's: [`NotStarted::Busy`] carries no wording of its own, so the line says what the user can do,
//! not which work held the key.

use gpui::SharedString;

use crate::operations::NotStarted;

/// Which control in one view had its press refused last for a save in flight on its key (see the module docs).
#[derive(Debug, Default)]
pub(crate) struct Refused(Option<SharedString>);

impl Refused {
    /// What the line says. Also the words of the host setup and host removal questions' permanent answers when their
    /// preference save meets another (`hosts::setup` and `hosts::remove`), so one refusal reads the same wherever it is
    /// shown.
    pub(crate) const WORDS: &str = "Not saved: another save was in progress. Try again.";

    /// Record how a press on `control` (its element id) went: a started save clears the line, a busy key moves it to
    /// `control`, and any other refusal (no UI session, a path that does not build) leaves it as it is, for the caller
    /// to log as before.
    pub(crate) fn record(
        &mut self,
        control: impl Into<SharedString>,
        started: &Result<(), NotStarted>,
    ) {
        match started {
            Ok(()) => self.0 = None,
            Err(NotStarted::Busy(_)) => self.refuse(control),
            Err(NotStarted::Retired | NotStarted::Path(_)) => {}
        }
    }

    /// Move the line to `control` without asking the store: for a press whose draft refused the edit because that
    /// draft's own save is in flight ([`crate::drafts::Draft::edit`]), which is work on the same key. A switch or a
    /// choice has no busy look of its own, so without this a second press during its own save would show nothing.
    pub(crate) fn refuse(&mut self, control: impl Into<SharedString>) {
        self.0 = Some(control.into());
    }

    /// Take the line away from `control`, if it is there: for a press on it that went through without starting a save
    /// (the tunnel switch's press that opens its warning), so the line no longer says that control's press was refused.
    /// A line beside another control stays, since nothing has shown that key free yet.
    pub(crate) fn clear(&mut self, control: &str) {
        if self.0.as_deref() == Some(control) {
            self.0 = None;
        }
    }

    /// The line beside `control`, if its press was the one refused last: `draw` builds it in the look of that control's
    /// own failure line from its element id (`<control>-busy`, where tests and the test bridge find it) and
    /// [`Refused::WORDS`].
    pub(crate) fn line<E>(
        &self,
        control: &str,
        draw: impl FnOnce(String, &'static str) -> E,
    ) -> Option<E> {
        (self.0.as_deref() == Some(control)).then(|| draw(format!("{control}-busy"), Self::WORDS))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operations::Work;

    /// The line must explain only the latest refused press and go once the key has proved free, or it would blame a
    /// save long finished. Specified: a press refused by its own draft shows its line there alone; a busy refusal on
    /// another moves it; a refusal without a session leaves it; a started save clears it.
    #[test]
    fn the_line_follows_the_latest_refused_press_until_a_save_starts() {
        let mut refused = Refused::default();
        let line =
            |refused: &Refused, control: &str| refused.line(control, |id, words| (id, words));
        refused.refuse("first");
        assert_eq!(
            line(&refused, "first"),
            Some(("first-busy".into(), Refused::WORDS))
        );
        refused.record("second", &Err(NotStarted::Busy(Work::Save)));
        assert_eq!(line(&refused, "first"), None, "moved");
        assert!(line(&refused, "second").is_some());
        refused.record("first", &Err(NotStarted::Retired));
        assert!(line(&refused, "second").is_some(), "not a busy key");
        refused.record("first", &Ok(()));
        assert_eq!(line(&refused, "second"), None, "a save started");
    }

    /// A press that went through without a save (the tunnel switch's, which opens its warning) must take away its own
    /// line, or "Not saved" would stand beside a switch the warning may turn on, and must leave another control's line,
    /// since no save has shown the key free. Specified: clearing another control leaves the line; clearing its own
    /// control removes it.
    #[test]
    fn clear_takes_away_only_its_own_controls_line() {
        let mut refused = Refused::default();
        refused.refuse("switch");
        refused.clear("other");
        assert!(
            refused.line("switch", |id, _| id).is_some(),
            "another control's clear"
        );
        refused.clear("switch");
        assert_eq!(refused.line("switch", |id, _| id), None);
    }
}
