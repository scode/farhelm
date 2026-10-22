# Sign-in recovery can retain a shortcut assignment after its save fails

Reviewed commit: 44051162a377d1d27360694d5ced54618cf12eb4

## TLDR

After sign-in recovery, Templates can keep showing an attempted shortcut assignment whose save later fails. Once the
dialog closes, the key can launch the old stored template instead. The lifecycle mismatch is conditional but established
in source; a harmful admitted template and consequential reliance remain unproved.

## Details

An older request can deliver a marked 401 while a newer binding writer remains pending. The qualifying request must
reach authentication after revocation, or have an already-produced 401 whose delivery is delayed: an accepted REST
handler cannot retroactively become unauthorized (`crates/farhelm-helm/src/auth.rs:378–390`). The binding edit's
preceding GET can succeed before rotation, then enqueue attempted map B before that older refusal arrives.

The process-owned writer survives removal of the original UI subtree
(`crates/farhelm-ui/src/api.rs:1267–1277,1377–1393`; `lib.rs:1778–1787`). Recovery's preference read waits for it, but
after five seconds can time out without issuing GET. Recovery nevertheless installs defaults plus pending B
(`lib.rs:2063–2084`). If the writer subsequently fails without committing B, settlement clears the queue while leaving
the newly mounted preference signal untouched (`api.rs:1133–1149,1334–1369`). The original component waiter no longer
exists. Another recognized 401 would reopen recovery and defeat this particular persistence route; use a later transport
failure or non-401 refusal.

Templates advertises the displayed assignment as immediately launching that template (`list/templates.rs:347–385`).
After closing it, a new page-context chord freshly resolves stored map A and can automatically submit it
(`list/view.rs:3559–3589,3624–3637,3862–3867`). Fresh resolution protects stored authority but does not obtain another
user decision about the changed indication.

Suggested bucket: **provisionally highest**, conditional on a materially different valid launch affecting user work;
otherwise high. Confidence: **confirmed conditional source mechanism**. Recovery must finish with over five seconds left
on the writer, and failure must follow seed installation. No actual harmful configuration, occurrence or loss was
observed. This differs from restoring an old map after a lost committed-save reply: here an optimistic copy survives
across component lifetimes after its write is discarded.

Possible cover: browser state-loss and cached-label acceptances have different triggers. The Near-term TODO is not
Planned coverage. Independent review rejected the whole-finding visual-glitch filter because the later key performs a
state-changing launch under a conflicting indication. No exact other coverage was found. Proposed drop: none.

Provenance: trailing review through `616d79f45d6a1d817833291429d144a5c686bb4b`, private run
`4fae501c-3686-4822-aa6c-037266637632`, `g010/f3` second branch, `a007/f2`, `g075/f1`. The producer withdrew its initial
display-only exclusion after cross-group synthesis. No runtime reproduction.
