# Terminal size readout mixes points and CSS pixels

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

The new terminal size readout is documented as points while its value reports CSS pixels.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The specification, guide and readout terminology conflict. Preserve the intended numeric stepping behavior rather than
assuming the fix should change scaling or native font measurement.

Source locations:
`website/src/content/docs/docs/using/work-in-a-session.mdx:46-47; SPEC.md:2073-2074; SPEC_impl.md:1355-1357`. Historical
revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed commit d6645f771bee8ed03a66486ef8755834937d759d; introduced by afd0fe73c144156fb24db96ac782268e4b639e7d. TLDR:
users are told the new center number is a point size, but it reports xterm's CSS-pixel font size. Evidence:
terminal.js:2682-2684 paints terminalFontSize verbatim; :2725-2727 and :3520 pass the identical number to xterm
options.fontSize. Shipped assets/vendor/xterm.js:1 (minified, only exact matched excerpts read) emits font-size:
${this._optionsService.rawOptions.fontSize}px for terminal row CSS and applies the same px unit to font measurement and
composition text. No conversion to a point value occurs in the readout path. The new public docs :46-47,
SPEC.md:2073-2074 and SPEC_impl.md:1355-1357 call the displayed value points. This is a units/documentation conflict,
not a requested terminal-size behavior change. Suggested bucket other; confirmed source inconsistency; no highest route,
runtime mismatch reproduction or user-work harm claimed. proposed_drop none. possible_cover: SPEC_impl.md:1342-1346
explicitly preserves direct xterm numeric sizing while :1355 calls its readout point-size; the landed plan
terminal-zoom-icons.md:23-34 similarly requires the raw 14/default/step behavior and uses points terminology. These
requirements constrain the fix to correcting terminology unless size behavior is separately approved; they do not
explicitly acknowledge the px/pt distinction, so coverage is ambiguous rather than silently accepted. Searches across
SPEC.md, SPEC_impl.md, TODO.md Planned (empty), BUGS.md, plans/queue, queue INDEX and TRIAGE_OUTCOMES.md found no exact
decision accepting pixel values described as points or planned repair. No FILTER fully applies to this ordinary
public-doc statement; it needs no rare trigger. Existing JS step docstring already used points, but this commit newly
exposes and documents that unit to users. Smallest fix: describe the number as terminal font size or CSS pixels, keeping
default, range, storage and stepping unchanged. No tests/builds/experiments or external source fetches performed.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g110/f3`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
