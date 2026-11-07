# Ambiguous resume retains old connection token

Reviewed commit: 8a7f66ede90651d47d1facc1e877ce81f842af1a

## TLDR

A successful retry after an uncertain Resume can retain the old connection token. Resulting gateway or operation
failures remain unverified after investigation.

## Details

Confidence: unverified after investigation; established source facts and missing premises are distinguished below.
Suggested bucket: other.

Source location:
`crates/farhelm-helm/src/tensorlake/lifecycle.rs:81-102,132-145,169-182 at 8a7f66ede90651d47d1facc1e877ce81f842af1a`.

Source assessment (read with the later corrections below) (review record g034/l2, event 18):

Reviewed commit 8a7f66ede90651d47d1facc1e877ce81f842af1a, unchanged relevant branches through pinned tip
8a8c9579e04299410652f0e4681975a7198dfc7b. Selected interface g044:l5 confirms this existing finding, not a discovery.
TLDR: after an externally suspended sandbox resumes but its API reply is lost, successful Resume retry can retain the
pre-suspension connection token; actual operation harm is unverified after investigation. Original lifecycle.rs:81-102
invalidates only after Ok(true); mutation/poll errors :172-182 skip it, cleanup :132-145 skips a Connected actor, and
subsequent Running GET returns Ok(false) at :169-170. hosts_tests.rs:1346-1399 establishes the pre-detection Connected
premise. UI api.rs:5466-5481 accepts any Connected row after Resume. SPEC_impl.md Tensorlake hosts requires actual
Resume to retire the pre-suspension connection. Discovery-turn interface reconciliation read
manager.rs:4700-4752,5045-5075 at the reviewed commit: refresh timeout sets end_connection Some (:4719), but returned
refresh errors set end_connection None (:4747); publication mints an incarnation only when the client pointer changes
(:5052-5069). CORRECTION to earlier wording: ordinary refresh is not an unconditional bounded recovery. Timeout or
actual connection death causes recovery; healthy/returned-error refresh on the same client preserves the old token. No
lasting operation failure can be concluded without the missing gateway premise, and no fixed failure duration is
claimed. Existing SSH/spec comments describe old-master hangs during suspension, but readable evidence does not settle
channel usability after successful ambiguous Resume. That gateway-impact premise remains unavailable; per instruction it
was not reopened with new experiments. Settling evidence remains a controlled gateway trace or equivalent readable
captured evidence; a mutate-then-error fake checks only token retention. Suggested bucket other; confidence confirmed
omission, conditional likely user impact. proposed_drop none. possible_cover g044:l5 is supporting duplicate ownership
input. g035:l4 remains DISTINCT dead-Retired-actor failure with stranded TensorlakeChanging; no consequence borrowed
from it. Caveats no runtime/cloud tests, no exact GUI initiation sequence or security/data-loss route established. Fix
retire known pre-suspension incarnation before attempting actual Resume or keep needs-recovery state across ambiguous
errors so a later Running shortcut cannot bypass it.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g034/l2`; supporting records
`g044/l5`, `g040/g034:l2`, `g049/iface_g034_l2`, `g045/interface-g034-l2`. The retained private log preserves earlier
wording and its corrections. This record does not claim runtime reproduction.
