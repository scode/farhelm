# When a waiting close stops on a refusal or missed hosts, Settings shows the page without the reason in view

Reviewed commit: ee8ef7294a6638d7905fca7dd27083d0d0bb45c9

## TLDR

Turn on a switch near the bottom of a long Settings page, such as Checkouts' keep-a-spare switch, move to another page
and press Escape. If the helm refuses that save, Settings stays open and jumps to the switch's page, but at the top. On
a short window the failure line under the switch is below the visible part of the page. The person sees Settings ignore
Escape for no visible reason and has to scroll down to find out why.

## Details

Line numbers are at the pinned tip b2d540a8dacfb94140e5eb0927d78e447ce46111; the code is unchanged since the reviewed
commit. On a refusal met while waiting, `close_settled` (editing.rs:1060-1093) calls
`reveal_save_failure(refusal.section)` for a switch (`CloseRefusal::page`, recorded by `switch_settled`,
settings.rs:1276-1279, and by the Tensorlake section's receipt, editing.rs:1047-1053), then `scroll_to_close_refusal`
(editing.rs:836-845). `reveal_save_failure` (editing.rs:821-832) calls `show(section)` when another page is shown, and
`show` resets the page scroll to the top (settings.rs:986-990). `scroll_to_close_refusal` only acts for Notifications
(the ntfy refusal or a failed Send test). Form refusals are pinned just above the footer (SPEC_impl.md "Settings puts
the ... actions in the dialog footer and pins refusals and progress just above the actions"), so they stay visible, but
a switch's failure line sits under the switch inside the scrolling body (Tensorlake: tensorlake.rs:247-260; the dialog's
switches via `choices::group`; Checkouts' spare via `checkouts::spare`, `SPARE_ERROR_ID`). Waiting for switch saves is
new in this commit (before it, a close went on and dropped the switch's error, per the old SPEC.md text in the diff), so
this is a route the commit added. Evidence that a switch can sit below the fold:
`the_tallest_sections_last_line_clears_the_footer_scrolled_to_the_end` (settings/tests.rs:5120-5148) establishes as its
premise that in a 560-pixel window Checkouts' last line, the spare switch's documentation link, starts below the body's
visible part. The spare switch's failure line is drawn after that link (checkouts.rs:110-118: switch, link row, then
`configured::failure(SPARE_ERROR_ID, ...)`), so it lies lower still; and the Tensorlake creation switch sits below the
experimental warning and the key form. The tests the commit added
(`a_refused_switch_save_keeps_a_waiting_close_open_on_its_page`, `a_close_waits_for_either_tunnel_save`, tests.rs) use
the default 1600x1000 window and check `drawn(error_id)`, which does not show the line is visible. Fix: generalize
`scroll_to_close_refusal` so the revealed refusal is scrolled into view for every switch's page (the ntfy mechanism,
`scroll_to_ntfy_refusal`, measured after layout), and add a test at 720x560 with the spare switch refused while a close
waits from another page, asserting `shows_whole(SPARE_ERROR_ID, DIALOG_BODY)`. Related case on the same mechanism:
`scroll_to_close_refusal` keys on page and form state, not on which refusal stopped the close. On Notifications, a
refused sound switch (the Sounds card is at the top of that page) met while waiting scrolls the page to the ntfy line at
the bottom whenever an earlier failed Send test or ntfy refusal is still shown
(`alerts.save_refused() || alerts.test_failed()`, editing.rs:837-841), moving the switch's failure line away from view.
The same fix covers it: scroll to the refusal the close met.

Reviewer revision notes: Revision (turn 4): possible_cover now names known finding g02/f2, which covers this finding's
Notifications sub-case. Revision (turn 3, second): added the Notifications wrong-scroll case. Revision (turn 3): the
spare failure line is drawn after the docs link, so confidence is raised to confirmed for that case. Title: When a
waiting close stops for a refused switch, Settings shows that page from the top, and only Notifications scrolls to the
refusal.

### Triage notes

- Recorded bucket: `high`. Three routes to one defect class in Settings' waiting close: the page it reveals does not
  bring the reason into view. g01's reviewer said it could be high if short Settings windows are common; recorded as
  high because the bucket is uncertain. FILTER.md's 'Dialog explanations out of view but reachable by scrolling' may
  cover part of it where the window is unusually small; triage decides. g02/c8 found g01/f3 holds only when there are
  more missed-host lines than fit.
- Suggested bucket (reviewer): other (a misleading but recoverable display; nothing is lost and the page is the right
  one).
- Confidence: confirmed for Checkouts' spare switch at a 560-pixel window, from source plus an existing test's asserted
  premise (not executed this review): the page is shown from the top, only Notifications scrolls, and the failure line
  sits below a link that test asserts starts below the visible body. For other switches (Tensorlake creation, tunnels)
  it is likely, depending on window height.
- possible_cover: the Notifications wrong-scroll case in this finding is the same defect as known finding g02/f2
  (refused Sounds switch scrolled to the bottom by an older ntfy error); the main case here (a switch's page shown from
  the top with its failure line below the fold, Checkouts' spare confirmed) is not covered by it. Both share one fix in
  `scroll_to_close_refusal`.
- proposed_drop: none

### Related reviewer records

#### g02/f2 (finding): A waiting close that meets a refused Sounds switch scrolls Notifications to the bottom when an older ntfy error is still shown, away from the switch's failure

Location: crates/gui/farhelm-gui/src/settings/editing.rs:1085

On Notifications, someone whose earlier Send test failed (or whose ntfy save was refused) flips a Sounds switch and
presses Close. If the helm refuses the switch, Settings stays open as it should, but jumps to the bottom of the page
where the old ntfy error is, so in a short window the switch that actually failed, and its failure line, are scrolled
out of view.

close_settled (editing.rs:1077-1085) shows a refusal by revealing its page and then calls scroll_to_close_refusal for
every refusal kind. scroll_to_close_refusal (editing.rs:836-845) does not look at which refusal is being shown: it sets
scroll_to_ntfy_refusal whenever the shown page is Notifications and the ntfy form currently reports save_refused() or
test_failed() (ntfy.rs:620-629), and render then scrolls the page to the bottom (settings.rs:1824-1826). The Sounds
switches belong to Notifications (choices.rs:140-144, Section::Notifications, so switch_settled records
CloseRefusal::page(Section::Notifications), settings.rs:1276-1278), and the comment on scroll_to_close_refusal says the
ntfy lines sit 'below its form and the sound controls'. test_failed reads `tested`, which stays Some(Err) from any
earlier failed Send test until the next Test or save (ntfy.rs:693-695, 719-722), and save_refused includes a stale busy
line. So the refusal the close resumed for (a switch near the top) gets the scroll meant for an ntfy error at the
bottom. The same mis-scroll applies when a waiting close's refusal is a CheckAnnouncements switch, if that sits above
the ntfy form. This is distinct from g03/f4 (no scroll at all for a low switch on other pages): here the page is
scrolled, to the wrong place. The scroll-to-ntfy helper existed before, but #5104 newly routes switch refusals through
it. Fix: scroll only for refusals of the ntfy form (CloseRefusal.edit naming the ntfy entity), or reveal the refused
switch's own control. Verify by reading the cited lines.

Suggested bucket: other (visual only; the switch keeps its own refusal line and Settings stays open).

Confidence: confirmed from source for the scroll decision; that the switch's line leaves the viewport depends on window
height versus the Notifications page length, which I did not measure.

#### g01/f3 (finding): Close and Save with missed hosts can leave the hosts it did not reach scrolled out of view in a short window

Location: crates/gui/farhelm-gui/src/settings/editing.rs:855

When you close Settings, choose Save, and a replaced credential could not be delivered to some hosts, Settings
deliberately stays open on the Credentials overview so you can see which hosts were missed. In a window too short to
show both the top of the overview and the credential list's Add button, the overview scrolls down to that Add button,
and the "Saved; not delivered to ..." lines at the top are out of view. Settings has stayed open with no visible reason
until you scroll up.

SPEC.md, "Settings" (around line 2781): "A credential replacement that missed hosts while Settings was closing keeps it
open on the Credentials overview, where those hosts are listed." show_delivery_reports (editing.rs:854-862) says it
stays "on the top of the Credentials overview" and resets page_scroll to 0. But the save's Done completion first runs
Credentials::close_editor (credentials.rs:410-427), which calls focus::focus_after_editor_close(&self.add[group], mark,
None, ...) (focus.rs:617-632). For a save that Close and Save sent, the mark is taken inside Editor::save
(editor.rs:316) during the loop's dispatch, after the confirm press, so has_moved_on is false and the move goes ahead.
It waits until the Add button is drawn and then calls reveal_focused / reveal_control (focus.rs:1235-1290), which
scrolls the dialog body by the least distance that brings Add into view. That undoes the reset to the top whenever Add
sits below the fold. The tip's own test admits it at credential_tests.rs, in
missed_hosts_reported_while_closing_keep_settings_on_the_overview (the comment before its shows_whole assertion): "this
shows the line is in view but not that the scroll was reset: the closing editor moves focus to its list's Add button,
whose scroll into view wins over the reset in a window too short for both." So the test does not pin the SPEC promise,
and the code does not keep it in short windows. The route where the editor's own Save is followed by Escape is not
affected: the Escape is newer input than the mark, so the focus move is refused and the reset stands. How to verify: run
that test's first half in a test window short enough that the overview's top and the GitHub tokens Add button cannot
both be shown, then assert shows_whole for credentials-undelivered-0. Fix options: when the close shows delivery
reports, keep focus off the Add button (for example by having show_delivery_reports take or put focus on the first
report line, or at the top of the page), or scroll after the pending focus move settles. Then tighten the test to use a
short window.

Suggested bucket: other. The report exists and scrolling up shows it; the harm is that Settings did not close with no
reason in view. It could be high if short Settings windows are common, since the case is exactly the one where a person
needs to see which hosts did not get the credential.

Confidence: likely. The ordering is traced in source and matches the test's own comment. The unverified premise is that
the reveal scroll runs after the reset at the real dialog geometry; it depends on layout, and I ran nothing.

#### g02/c9 (candidate): Cross-check of g03/f4 (switch refusal shown scrolled to top): confirmed; the related wrong-target scroll is f2

Location: crates/gui/farhelm-gui/src/settings/editing.rs:836

Bears on known finding g03/f4. Checked: close_settled reveals a refusal with reveal_save_failure/reveal_edit_failure,
which calls show(section), and show resets the page scroll to the top when the section changes (settings.rs:986-991);
the only follow-up scroll is scroll_to_close_refusal, which scrolls only Notifications and only for ntfy form state
(editing.rs:836-845). So a low switch's failure line on another page can start off-screen, as g03/f4 says. The same
pattern for a refused form in the close question's Save predates #5104 (the parent's scroll_to_close_refusal was
ntfy-only too). The different defect I found in this code, a wrong scroll target on Notifications, is recorded as f2.

#### g02/c8 (candidate): Cross-check of g01/f3 (missed-host lines scrolled out of view after Close and Save): holds only for more lines than fit

Location: crates/gui/farhelm-gui/src/settings/editing.rs:855

Bears on known finding g01/f3. Checked: show_delivery_reports (editing.rs:855-862) shows Credentials and sets the page
scroll to the top; the overview draws the delivery-failure lines as its first children, before the help, warning, key
form and lists (credentials.rs:567-588). So the first missed hosts are always in view; g01/f3 is right only in that
lines beyond the viewport height in a short window are not. I see no broader or different defect in this code; no
separate record.

### Provenance

Trailing code review of `dbb153ec94149a598a747af8ab306a3df0514641..b2d540a8dacfb94140e5eb0927d78e447ce46111` (run
`ef8bcf48-a7d4-4d80-9e04-457b0ff30acc`), Claude Code `claude-opus-5-5` at high effort under persistent-review.

- g03/f4 (finding), group g03 "Settings close waits for outstanding work: spec, panes and tests", findings log line 14.
- g02/f2 (finding), group g02 "Settings close waits for outstanding work: editing core", findings log line 10.
- g01/f3 (finding), group g01 "Settings credential editor stays open until its save answers", findings log line 6.
- g02/c9 (candidate), group g02 "Settings close waits for outstanding work: editing core", findings log line 12.
- g02/c8 (candidate), group g02 "Settings close waits for outstanding work: editing core", findings log line 11.
