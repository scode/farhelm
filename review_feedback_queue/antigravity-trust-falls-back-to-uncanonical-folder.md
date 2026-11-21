# Antigravity trust preparation silently substitutes the submitted path after canonical lookup fails

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

If the folder can no longer be resolved or encoded during launch preparation, the Antigravity trust setting can use the
submitted spelling.

## Details

The wire cwd and the canonical directory are UTF-8 strings. Ordinary create uses `canonical_cwd_text` to reject an
unencodable target at `crates/farhelm-supervisor/src/service/core.rs:3447–3472,9139–9177`, yet sends the original
expanded spelling as the first `launch_cwd` (`:9026–9034,11405–11419`). The compiler's whole-value Antigravity slot
(`crates/farhelm-proto/src/session_launch.rs:48–76`) is recognized only on a structured Antigravity selection with
resolved trust and in its settings pair before a Custom terminator
(`crates/farhelm-supervisor/src/agent_kind/mod.rs:2380–2403`). The shared fill at `:2345–2355` swallows lookup and UTF-8
errors and uses the submitted path in `trustedWorkspaces`. `core.rs:14559–14583` publishes that spelling separately as
`working_directory`. `crates/farhelm-supervisor/src/launch.rs:1207–1212,1231,1276–1282` checks directory entry but makes
no encoding check on its resolved destination. A missing directory can still fail there. The source also admits an OS
directory entry through an encodable alias when the target's own bytes are unencodable after admission. For a stable
target the earlier guard closes the branch. New managed checkout, retry and Restart normally hand the accepted or
revalidated canonical spelling onward (`core.rs:9776–9785,9831–9835,11166–11197,12417–12429`). SPEC.md:1759–1763
requires the canonical host cwd for trust and :5096–5102 requires a clear refusal at surfaces that meet unencodable
paths. Make the Antigravity trust fill require a canonical UTF-8 result and carry that same verified path into
execution. Retain deliberately separate Command and Codex behavior.

Confidence: confirmed by inspection, with a changed lookup result needed after ordinary admission. No vendor decision or
out-of-scope trust grant is established.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `agTrust/l9`, `agUI/l3`.
