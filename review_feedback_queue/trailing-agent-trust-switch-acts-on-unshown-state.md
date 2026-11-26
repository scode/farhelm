# Agent trust switch can still act on an unshown state after a refused save

Reviewed commit: be7cfc065c7a474f7ad9cf7f2da77cf5ef7e301e

## TLDR

On an agent's Settings page, the user turns trust on and the helm refuses the save. If the user presses Space again (to
turn it back off, since the switch still looks on) in the moment before the reverted switch appears, that press asks the
helm to turn trust on again, and if that save succeeds the agent's workspace trust is on against the user's latest
press. The same commit fixed exactly this for the conversation-hooks switch just below, but left the trust switch, which
is a permission decision, with the old behavior.

## Details

Priority bucket: `highest`.

Provenance: trailing code review of `953603f1a036eb5846ebf251d1564c5efd9c8e67..f0c042c9efea4a46bc02e0ad880c1a30aae3c85f`
(run `70f0f28c-538d-461e-b80e-783ddaafaadb`), reviewer record `t04a-settings-save-trust` `l1` (Claude Code `claude -p`,
`claude-opus-5-5`, effort high). Findings were validated by source and contract analysis; nothing was built, run or
reproduced.

### Recording notes

Recorded in the reviewer's suggested `highest` bucket: a permission grant (workspace trust) can be saved against the
user's last visible action. The trigger needs a key press inside a sub-second window, but FILTER.md's sub-second race
filter excludes trust-boundary consequences, so it does not apply. It would move to `high` if triage judges that the
switch visibly showing on immediately afterwards makes the grant one the user would notice and undo. Not covered by any
coverage source (TODO.md Planned, plans, BUGS.md, queue and TRIAGE_OUTCOMES.md are empty or unrelated at the tip).

### Reviewer record

Location:
`crates/gui/farhelm-gui/src/settings/agent_trust.rs:424-465 (tip); crates/gui/farhelm-gui/src/settings.rs:1610-1630 (tip)`

be7cfc065c changes hook_settings::row (hook_settings.rs:28-34, 63-67 at be7cfc065c) to capture `!checked` in an
ActionFrame keyed per agent and to call on_toggle only with a snapshot whose when_shown has fired, because 'A refusal
can reverse the switch during a keyboard redraw' (its comment) and SPEC_impl.md says 'GPUI can draw a dirty frame before
dispatching a key, so a handler for a newly drawn frame may have to refuse the press' and that the shown-value rule
'starts at the control that asks the question or sends the request directly'. The trust switch drawn above it shares the
same mechanics: it is a configured::Configured switch whose shown() is the optimistic draft value while saving and the
known value after a refusal (configured.rs:103-111 at tip), and its toggle goes through the same
SettingsDialog::toggle_configured (settings.rs:1234-1267 at tip). But agent_trust::block (agent_trust.rs:424-465 at tip;
unchanged in this respect by later range commits 571096872, 02cded84 and 3610a80a) still passes on_toggle straight to
Checkbox::on_toggle with .checked(trust.state.shown()), so the handler toggles to the negation of whatever its own,
possibly unpresented, frame drew. Route, mirroring the commit's own test
grok_hook_setup_and_its_hint_are_independent_settings: focus the trust switch, Space (sends enabled=true, switch shows
on), helm refuses, the refusal redraws the switch off in a frame not yet presented, the user (still seeing on) presses
Space intending off, and the new frame's handler sends enabled=true again. If that second PUT succeeds, trust is on
although the user's last press, against the switch they could see, asked for off. Source basis for the handler value:
Checkbox computes the toggle target as !self.checked when the element is built (components/checkbox.rs:241-245 at tip)
and its Space key handler calls on_toggle with that value (checkbox.rs:371-374), so an unbound on_toggle always acts on
the frame that receives the key, which is exactly what the hook fix guards against. Verify by copying the hook test's
sequence onto a trust switch (for example Claude's) with simulate_keystrokes after the refusal and observing a PUT to
the trust route. Fix: give agent_trust::block the same ActionFrame/when_shown binding as hook_settings::row (or move the
binding into a shared Configured-switch helper so the spare switch and future configuration switches inherit it). The
ordinary preference switches built by SettingsDialog::switch -> choices::group (settings.rs:1440-1456 at tip) use the
same unbound on_toggle; whether their optimistic/revert states create the same window (for example Skip session
confirmations) is worth auditing in the same fix.

Suggested bucket: highest — it can enable an agent's workspace trust (a permission grant) against the user's last action
on an established route; it would move to high if the maintainer judges that the switch visibly showing on immediately
afterwards, plus the user's earlier press asking for on, makes this a correctable display issue rather than a security
consequence.

Confidence: confirmed (source-established) - the trust switch's state (Configured), toggle path (toggle_configured) and
Checkbox wiring are the same code the hook switch used before be7cfc065c; the dirty-frame-before-dispatch premise is the
one SPEC_impl.md states and the commit's own hook test exercises. Not executed for the trust switch in this review.

proposed_drop: none

possible_cover: The triage outcome fixed by be7cfc065c (hook-settings-toggle-uses-unshown-state) may have been scoped to
the hook switch deliberately; not checked against the queue per the brief.

Revision (turn 2): sharpened the TLDR and route wording, added the Checkbox source references, and raised confidence
from likely to confirmed because the mechanism is identical code to the pre-fix hook switch.
