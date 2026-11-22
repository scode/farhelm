# Trust-hint save errors use the live selection instead of the drawn session

Reviewed commit: 44137d0659d28b9cadf90a8ffe96a65930035aad

## TLDR

A busy or refused Don't show again click can attach its error to a different session if selection moved after the hint
was drawn.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/session_pane/trust_band.rs:286`.

the revised trust-hint section, SPEC_impl.md:3522–3527, describes the band and its dismissals. SPEC.md:1769–1774 keeps a
failed shared dismissal beside the hint in the session that showed it, and the draw-time binding rule is preserved at
24bc0580:SPEC_impl.md:1069–1079. trust_band.rs:457–488 captures the drawn agent only for this action, although the drawn
key is available and captured for × (490–502). hide_everywhere reads the current selected key (285–288), then attributes
the busy error and any asynchronous refusal to it (301–331). draw shows a matching key's error (368–384). The shared
preference command still uses the drawn agent (292–299). Only error ownership is affected. The existing test at
trust_band_tests.rs:647–716 changes selection after dispatch and checks the response is suppressed, leaving the
draw-to-handler interval untested. Pass the drawn session key with the agent and use live selection only to decide
whether its error may still be shown. Bucket: other (local error routing). Confidence confirmed for live-state binding,
with a visible error on another session conditional on stale callback delivery, which was not observed. Possible
coverage: none. L27 concerns resetting a different, occurrence-only dismissal.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `recovery_contract/l28`.
