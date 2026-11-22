# Wake failure alerts in Restart with hardcode Not launched instead of Restart error

Reviewed commit: 66ad3aaa95ef03a2130e2d0194d58353912ae9da

## TLDR

When a Restart with attempt fails while waking its suspended or stopped host, the alert beside the Restart button starts
with "Not launched." instead of "Restart error.".

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/launcher/launch/wake.rs:150`.

SPEC.md ("Creation", lines 1559-1562) specifies: "In the session launcher every refused launch (New, Clone, Replace
with, quick launch) shows as a bordered alert with a warning icon beside Launch, starting 'Not launched.' before the
refusal's own words, and announced to assistive technology. Restart mode uses the same alert beside Restart, with
'Restart error.' before the refusal's own words." However, `crates/gui/farhelm-gui/src/launcher/launch/wake.rs:150`
(`reply`) hardcodes `.text("Not launched. Resuming the host failed: ")` and `wake.rs:278` (`failure_words`) hardcodes
`.text("Not launched. ")`. `crates/gui/farhelm-gui/src/launcher/view.rs:2583-2587` renders `Note::WakeRefusal(reason)`
directly, bypassing the mode-aware `lead` ("Restart error." for `Mode::RestartWith`). Fix direction: Choose the lead
from the launcher's mode ("Restart error." for `Mode::RestartWith`, "Not launched." otherwise) either in `wake.rs` or
when rendering `Note::WakeRefusal` in `view.rs:2583-2587`.

Reviewer metadata (g10-wake-launch-state-and-requests/f5): suggested bucket: other (not highest: minor UI copy mismatch
with SPEC.md, no security or data-loss impact); confidence: confirmed; possible cover: none.

Additional source record at `66ad3aaa95ef03a2130e2d0194d58353912ae9da`,
`crates/gui/farhelm-gui/src/launcher/view.rs:2583` (g11-wake-launch-views-and-panes/f4):

`SPEC.md:1560-1562` specifies: "In the session launcher every refused launch (New, Clone, Replace with, quick launch)
shows as a bordered alert with a warning icon beside Launch, starting 'Not launched.' before the refusal's own words,
and announced to assistive technology. Restart mode uses the same alert beside Restart, with 'Restart error.' before the
refusal's words." In `crates/gui/farhelm-gui/src/launcher/view.rs:2554-2587`, ordinary refusals apply
`lead = if restart { "Restart error." } else { "Not launched." }`, but lines 2583-2586 override `alert` with
`Note::WakeRefusal(reason)` verbatim. Because `crates/gui/farhelm-gui/src/launcher/launch/wake.rs:149-151` and `276-280`
unconditionally prepend `"Not launched. "` regardless of `draft.mode`, a failed wake in `Mode::RestartWith(_)` displays
`"Not launched. ..."` instead of `"Restart error. ..."`. Fix direction: Store only the wake failure body in
`Note::WakeRefusal` and prepend `lead` (`"Restart error. "` vs `"Not launched. "`) in `view.rs:2583-2587`, or pass the
mode's lead into `wake::reject`.

Reviewer metadata (g11-wake-launch-views-and-panes/f4): suggested bucket: other (could not be in highest: wording/spec
mismatch only); confidence: confirmed; possible cover: none.

Additional source record at `9ece1d1f889a47c5dd6bd82b5f384c8fd5e87e75`,
`crates/gui/farhelm-gui/src/launcher/request_tests.rs:133-166` (g12-wake-folder-launch-tests/l1):

Verified against 9ece1d1f889a47c5dd6bd82b5f384c8fd5e87e75 and pinned 66ad3aaa95ef03a2130e2d0194d58353912ae9da.

1. Test gap in g12: crates/gui/farhelm-gui/src/launcher/request_tests.rs:133-166 replaced
   a_suspended_environment_names_the_resume_needed_before_launch (which covered New, Clone, Restart with, and Replace
   with) with a_resumable_environment_allows_the_press_before_it_can_create covering only New mode (leaving restart_body
   returning None while suspended/stopped and waiting_for_place refusal in Restart with untested). Likewise,
   crates/gui/farhelm-gui/src/session_pane/environment_tests.rs:92-100,251-270 removed both
   session-header-restart-options and session-header-replace-options from the suspended-disabled check but only added
   the_suspended_replacement_caret_offers_the_launcher for Replace, leaving the suspended Restart caret untested.
   crates/gui/farhelm-gui/src/launcher/wake_tests.rs:1-328 has no Restart with wake tests.
2. Spec mismatch masked by this gap: SPEC.md lines 1559-1562 require: "In the session launcher every refused launch
   (New, Clone, Replace with, quick launch) shows as a bordered alert with a warning icon beside Launch, starting 'Not
   launched.' before the refusal's own words, and announced to assistive technology. Restart mode uses the same alert
   beside Restart, with 'Restart error.' before the refusal's words." However,
   crates/gui/farhelm-gui/src/launcher/launch/wake.rs:149-151 and 276-280 hardcode "Not launched. " into HostSentence
   (and wake_state.rs:232,235,253 / wake.rs:212,218 also hardcode launch phrasing), and
   crates/gui/farhelm-gui/src/launcher/view.rs:2583-2587 replaces the mode-aware alert (which computed lead = "Restart
   error." at lines 2554-2558) with Note::WakeRefusal(reason) verbatim. Fix direction: Store only the wake failure
   reason (without a hardcoded lead) in Note::WakeRefusal (or pass the mode lead from Launcher::render when building the
   alert so Restart with uses "Restart error. "), and restore Restart with / Replace with / Clone coverage in
   request_tests.rs, suspended Restart caret coverage in session_pane/environment_tests.rs, and Restart with wake
   coverage in wake_tests.rs.

Reviewer metadata (g12-wake-folder-launch-tests/l1): suggested bucket: other (could not be in highest: wording mismatch
and missing test coverage with no security or data-loss impact); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g10-wake-launch-state-and-requests/f5`,
`g11-wake-launch-views-and-panes/f4`, `g12-wake-folder-launch-tests/l1`.
