# Character-width cache retains attacker-controlled strings indefinitely — possible

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Rendered combining-character strings could grow memory beyond scrollback limits.

## Details

F31 — **possible** — `crates/farhelm-ui/assets/vendor/xterm.js:1; module 2744; bytes [91748,92175); R:1899–1917`;
`crates/farhelm-ui/assets/vendor/xterm.js:1, module 2744, bytes [91748,92077), readable lines 1899–1913` —
Character-width cache retains attacker-controlled strings indefinitely — possible

The terminal's character-width cache retains each distinct rendered combining-character string with a positive measured
width after its cell disappears. Scrollback limits do not bound those keys; measurement changes, explicit clearing, or
terminal disposal eventually release them. The accumulation mechanism is established, while practical browser memory
pressure and whole-window effects remain unmeasured. Assess a proportionate upstream or integration remedy that bounds
retained entries and key bytes, without assuming a local vendor patch or elaborate availability defense is required.

## Evidence and triage context

- crates/farhelm-ui/assets/vendor/xterm.js:1 constructs the DOM renderer and its WidthCache; the row factory passes
  complete cell strings to get.
- crates/farhelm-ui/assets/vendor/xterm.js:1, module 2744, stores every distinct positively measured non-flat key in
  _holey without eviction.
- crates/farhelm-ui/assets/terminal.js:3449 constructs Terminal and does not load an alternative renderer; :4337 writes
  incoming bytes into it.
- crates/farhelm-ui/assets/vendor/xterm.js:1; representation [private review artifact] stores every distinct
  positive-width non-flat key in _holey without eviction.
- Actual get-method tokens retained 1,000 distinct keys with an injected positive measurement result.
- Readable line 1776 supplies actual rendered cell strings; readable lines 1890 and 1893 clear on disposal or explicit
  cache clearing rather than content eviction; readable line 396 selects the DOM renderer.
- crates/farhelm-ui/assets/terminal.js:3449 constructs the shipped terminal with bounded scrollback, which does not
  bound this cache.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- TRIAGE_OUTCOMES.md:6061 bounds transport frames, not rendered-string retention. SPEC.md:2290 accepts availability
  effects only when not easily avoided; it does not conclusively cover this unbounded cache. The Maybe later terminal
  replacement in TODO.md:655 is not a planned fix for this defect.
- SPEC.md:2290 conditionally accepts difficult-to-avoid DoS degradation; ease of an unpatched-vendor remedy has not been
  established. TRIAGE_OUTCOMES.md:6061 concerns queued frame bytes, and BUGS.md:53 concerns host-side tmux
  notifications. Neither covers this cache.

Caveats:

- Retention lasts until cache clearing or terminal disposal, not literally forever.
- Only strings actually rendered and measured with positive width are retained.
- Attack rate and memory pressure in Chromium or WebKit were not measured.
- Only actually rendered strings with positive measured width enter the cache.
- Not literally indefinite: measurement changes and terminal disposal release entries.
- Attack throughput, retained-byte growth and browser memory pressure remain unverified.
- Only strings actually rendered and measured with positive width enter the map.
- Rendering coalescence affects accumulation rate.
- Browser memory growth, exhaustion, and effects on other controls were not measured.
- The offline proof injected measurement results; it did not render glyphs in a browser.
- Only strings actually rendered with positive measured width enter the cache. Coalesced rendering affects the
  accumulation rate.
- No OOM, cross-session disruption, process death or loss of work was demonstrated.
- Keep the proposed action as remedy assessment; do not prescribe a vendor patch or elaborate defense contrary to the
  availability exception.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `vendor_01_sec:p1:F2`, `vendor_01_cor:p1:F4`.

- `vendor_01_sec:p1:F2`: confidence as filed: possible; suggested bucket as filed: highest.
- `vendor_01_cor:p1:F4`: confidence as filed: possible; suggested bucket as filed: highest.
