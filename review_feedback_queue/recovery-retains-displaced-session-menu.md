# Fast desktop recovery can leave a session menu displaced

Reviewed commit: feffbbeec73633d2898ec33e63a7c4d4dfe7b5e1

## TLDR

Fast recovery from a failed desktop listing can leave an open session menu at its old position after the count heading
wraps. Its commands still target the original session, but someone relying on placement could mistake which session they
will stop or delete. The layout mechanism is conditional; actual wrapping, misidentification and loss were not observed.

## Details

`crates/farhelm-ui/src/list/view.rs:1384–1390` skips banner and row comparisons when the preceding result is Err.
Coalesced demand immediately starts another HTTP read (`reader.rs:517–535,654–665`; `api.rs:2315–2324`). That read can
finish while the native UI thread is descheduled. Locked Dioxus 0.7.10 polls a ready task before rerendering its equally
ranked dirty scope (`scheduler.rs:203–213`; `virtual_dom.rs:599–620`), so recovery can replace Err before row teardown.
Retained rows get no recovery mount measurement (`list/row.rs:1934–1938`; `list/bell.rs:202–206`). A changed fleet total
with unchanged filtered rows isolates the potentially wrapping header (`assets/app.css:1023–1033`). The pinned tip
retains the skipped baseline (`list/view.rs:1465–1471`). The later provisioning-remedy repair covers a different seam.

Suggested bucket: **provisionally highest** for mistaken process-ending activation; **other** for visual detachment
alone. Confidence: **confirmed conditional native source mechanism**. Browser applicability is unestablished. Frequency,
actual geometry and harmful human reliance remain unmeasured; no automatic activation or mechanical retargeting is
alleged. Stop remains immediate after busy checks (`list/row.rs:2499–2508`; `list/view.rs:1932–1986`), and Delete has an
opt-out path (`2204–2225`). The menu's labels, title and tint mitigate confusion without proving it impossible.

Possible cover: the rare visual-glitch filter covers only the benign subset. Independent review found whole-finding
exclusion ambiguous because wrong perceived target and work-loss consequences remain possible. `SPEC_impl.md:725–767`
requires displaced-menu closure. Complete ledger decisions at `TRIAGE_OUTCOMES.md:2267–2285,5136–5157,16322–16351`
choose fixes even with original-target callbacks; none accepts this recovery trigger. The notification Clear decision at
`16867–16879` has a different consequence, and pre-redraw subsecond acceptance does not clearly cover a menu staying
displaced after recovery. No matching Planned, BUGS or queue record was found. Proposed drop: none for this conditional
remainder.

Provenance: trailing review through `616d79f45d6a1d817833291429d144a5c686bb4b`, private run
`4fae501c-3686-4822-aa6c-037266637632`, `g057/f1`, fresh `a015/f5`. Matching dependency archives and source were
checked; the initial unrelated reqwest 0.12.28 read was discarded in favor of locked 0.13.5. No runtime reproduction.
