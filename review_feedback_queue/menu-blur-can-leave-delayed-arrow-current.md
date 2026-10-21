# Blur-only clicks may leave delayed menu arrows current

Reviewed commit: 1c671d26b1a5afdfc7a12d0fb986f349578a5250

## TLDR

Clicking unused space inside an open menu may still let an earlier delayed arrow reclaim focus. The shipped browser's
blur/focus sequence is unverified; no command activation or loss of work was established.

## Details

`crates/farhelm-ui/assets/menu-focus.js:20–29` advances the independent-focus epoch only on `focusin`. If pressing
nonfocusable header content blurs an item to the body without another focus-in, the epoch remains unchanged and the old
request passes `37–44`. The header is at `src/list/row.rs:2111–2135`; outside dismissal ignores internal presses
(`src/menu_panel.rs:1036–1042`). Item focusout clears Rust's focused position (`src/list/row.rs:2207`) but does not
invalidate the page epoch or sent request. The unit fixture's focus simulation models focus-in, not this native event
premise (`js-tests/menu-focus.test.js:67–72`). Establish the real engine behavior before changing cancellation policy.

Suggested bucket: **other**. Confidence: **likely conditional on blur-to-body without focus-in**. No security
consequence, activation or durable wrong state was established. Proposed drop: rare recoverable focus glitch or
competing-input acceptance may apply, but neither occurrence rarity nor the delayed request's bound was established
statically.

Possible cover: `SPEC.md:1392–1397` accepts competing inputs within tens of milliseconds; final `SPEC_impl.md:703–717`
rejects requests after independent focus but does not define blur-only transitions. The ledger's bounded mixed-input
decision at `TRIAGE_OUTCOMES.md:15644–15672` may apply, while `15258–15281` concerns item focus. FILTER's rare-glitch
and subsecond conditions remain uncertain. No exact Planned, BUGS or queue match was found; the existing notification
handback item concerns a different helper. Provenance: menu implementation `f2` and sibling corroboration, trailing
review through `fe0348ef8d4fabc3c6140a4fd89fb19dd34178b1`. No runtime reproduction was performed.

Original finding IDs: `menu-focus-implementation-1/f2`, `menu-focus-implementation-2/f2`, `menu-focus-tests-1/f2`.
