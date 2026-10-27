# Failed Claude Restart removes an established report pin

Reviewed commit: 463cac1528b5c5e259404975380d321a3fff0412

Additional reviewed implementation: 77ef80e12e754b7b0a2691ec5da37b132e86d02b

## TLDR

After Restart stops Claude but fails before replacement, its conversation can survive while its reporting pin is
cleared. A delayed eligible child report could then replace that Resume target. An ordinary supported producer of the
required hooked, unmarked nested child remains unestablished, so the harmful sequence is conditional.

## Details

Suggested bucket: **other**. Confidence: confirmed reset, abort and report-admission composition, with a missing
supported input producer. No vendor behavior, harmful Resume or runtime frequency was measured.

The original review concerned `plans/queue/claude-wrapper-capture.md:74–79,118–123,146–154` at the first reviewed
commit. Its narrow pin-retention refutation remains valid. The implementation review now supplies the formerly missing
control flow, rather than treating the plan as proof of shipped behavior.

At the additional implementation revision, paths below are under `crates/farhelm-supervisor/src/`:

- Restart drains reports before stopping the old tree (`service/core.rs:11312–11322,11533`), and opening a generation
  clears its pin (`store.rs:4142–4155`).
- Definitive pre-respawn failure can restore the old pane without its pin (`service/core.rs:12056–12061,11908–11935`;
  `store.rs:4238–4245`). Republication uses the current generation (`service/core.rs:11987–12006`).
- Recorded ancestry can anchor a delayed report against that dead pane (`service/core.rs:15930–15944`;
  `procs.rs:483–513`), receive the current generation (`service/report_files.rs:319–332`), and reach unpinned admission
  (`service/core/vendor/claude.rs:79–80,142–158`). Already-anchored stale reports and retained-parent-pin descendants
  remain separately fenced; those narrower refutations do not settle this re-anchoring path.

The remaining source/input premise matters: reporter injection is local to launch argv (`agent_kind/mod.rs:1251–1286`).
Wrappers forward that argv to their launched agent, without establishing recursive injection. At later reviewed commit
`3e23099d37a9cab5d893d1ec052b13042cc77f74`, `crates/farhelm/tests/e2e/real_agent_capture.rs:580–612` explicitly supplies
child hooks as a stand-in for user/project settings and says the parent's injected setting is not inherited. Synthetic
reports, preserved user hooks and inherited credentials do not establish an ordinary supported producer or the complete
harmful sequence. This is a missing premise, not merely unmeasured occurrence.

Possible cover: trailing tip `a2c2f8861418f8222eca096c797b9f31585ecc1a`, `SPEC.md:2313–2332` and
`SPEC_impl.md:3055–3080`, explicitly accepts reset and abort without restoring the pin. The ordinary initial child-first
interval is also accepted. Whether those paragraphs accept the whole post-stop, definitive-abort and delayed-report
consequence remains ambiguous; the independent review retains that remainder. `TRIAGE_OUTCOMES.md:16086–16097` is
Codex-specific. `SPEC.md:2211–2218` accepts continuing to resume stored capture after tracking stops; do not count that
derivative consequence alone as another defect. Unread parent-report displacement before admission is separate.

Provenance: original `a077/f1`, `g131/f3`; implementation `g078/f2`, `g079/f1`, independently retained by `a011/f1`.
These are shared identities for one concern. Proposed drop: none for the conditional remainder; no fixes or triage were
performed.
