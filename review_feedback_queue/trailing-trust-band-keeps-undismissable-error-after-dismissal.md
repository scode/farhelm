# Trust hint band keeps showing an uncloseable save error after the hint is dismissed

Reviewed commit: 66ad3aaa95ef03a2130e2d0194d58353912ae9da

## TLDR

After a failed or busy "Don't show again" click on a trust hint banner, dismissing that hint from Settings (or having
the first click succeed after a rapid second click hit the busy guard) leaves a red "Could not save hiding this trust
hint" / "Not saved" alert stuck above the terminal with no Close button until you leave the session or answer the
prompt.

## Details

**other.** Source inspection. Main reference
`crates/gui/farhelm-gui/src/session_pane/trust_band.rs:101-103,313-321,346-361`.

In `crates/gui/farhelm-gui/src/session_pane/trust_band.rs:101-103`, `Facts::shows_failure` is
`failed && self.question && self.agent_shown && !self.hidden && self.trust != Some(true)` and intentionally omits
`!self.dismissed` (pinned by `every_condition_withdraws_the_band_on_its_own` at
`crates/gui/farhelm-gui/src/session_pane/trust_band_tests.rs:372`). In `draw()` (`trust_band.rs:365-379`), the action
buttons (including `×`) are rendered only inside `.when(shows, ...)` while `shows` is false whenever `facts.dismissed`
is true. Two reachable paths leave an orphan, uncloseable alert when the dismissal is actually stored: (1) After a
refused/busy "Don't show again" sets `known.failed` (`trust_band.rs:303` or `318`), the user opens Settings and turns
off "Show the hint when <agent> asks about folder trust" (or another client/CLI writes
`<agent>_trust_hint_dismissed: true`). The preference push sets `facts.dismissed = true`, hiding the explanation and
`×`, while `known.failed` is never cleared and `shows_failure` stays true, leaving only the contradictory error line
visible. (2) If a rapid/queued second click on "Don't show again" runs before redraw while the first click's save is in
flight (`known.hiding == Some(agent)`), the second click hits `Err(NotStarted::Busy(_))` (`trust_band.rs:302-304`) and
sets `known.failed = Some((pressed, busy()))`; when the first click succeeds (`refused == false`), `hide_settled`
(`trust_band.rs:313-321`) clears `known.hiding` without clearing `known.failed`, and because `shows_failure` ignores
`self.dismissed`, the "Not saved" alert remains stuck after the dismissal succeeded. Fix direction: gate
`Facts::shows_failure` on `!self.dismissed` (and clear `known.failed` in `hide_settled` when `!refused`), which still
keeps the failure line visible when the corrective trust read fails (`trust == None`, `dismissed == false`) and when
another preference save blocks "Don't show again" (`known.hiding == None`, `dismissed == false`), while withdrawing the
error once the hint is actually dismissed or while the band's own dismissal is actively saving; update
`trust_band_tests.rs:372` accordingly.

Reviewer metadata (g17-trust-gui-and-goldens/f5): suggested bucket: other (cannot be highest: purely a stuck UI error
banner with no security or data-loss impact); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g17-trust-gui-and-goldens/f5`.
