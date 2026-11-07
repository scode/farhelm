# Sandbox termination poll exceeds phase deadline

Reviewed commit: 8a8c9579e04299410652f0e4681975a7198dfc7b

## TLDR

Sandbox deletion can exceed its stated termination-polling allowance because each lookup receives a fresh timeout beyond
the phase's remaining time.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `crates/farhelm-helm/src/tensorlake/hosts.rs:244-262 at 8a8c9579e04299410652f0e4681975a7198dfc7b`.

Source assessment (read with the later corrections below) (review record g032/g032-f5, event 10):

Reviewed original commit: 88bf83e267b5066815448885c61ed4148b851930, hosts.rs:223-241; rechecked at pinned main
8a8c9579e04299410652f0e4681975a7198dfc7b, hosts.rs:244-262. TLDR: sandbox deletion can spend longer than the stated
30-second termination polling allowance before refusing and asking the user to retry. The latest pinned source retains
the same mechanism: deadline set at :244, get_sandbox awaited under independent bounded at :246, deadline examined only
after the reply at :252, and one-second sleep at :262. bounded at :268-278 applies REMOVAL_CALL_TIMEOUT of 20 seconds
(:33), without considering remaining termination time. Two successful nonterminal replies taking 19 seconds each,
separated by one-second sleep, reach the refusal at about 39 seconds. SPEC_impl.md, Tensorlake hosts, :6050-6053 still
explicitly promises polling '(at most 30 seconds...)' and separately bounds each API call at 20 seconds. The source
docstring at hosts.rs:236-237 also says the poll is within TERMINATION_WAIT. This is the polling phase only: no claim of
a 30-second whole-removal limit, and preceding termination/name resolution and subsequent snapshot cleanup legitimately
have independent budgets. The returned reason uses the configured 30-second limit rather than actual elapsed time
(:256-258). Suggested fix: wrap this phase in its deadline, or bound each lookup by remaining time and preserve
retryable ownership on expiry. Verification proposal remains a paused-clock API fixture with delayed successful
nonterminal replies; no test was run. Suggested bucket: other; only extra bounded waiting and misleading timing are
established, with no security/data/process-loss route. Confidence: confirmed by source inspection/timing arithmetic
only, not provider observation. proposed_drop: false. possible_cover: unknown, intentionally not searched. Synthesis
result: original finding retained, unrepaired at pinned main, with corrected current references and unchanged limited
scope.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g032/g032-f5`; supporting
records `g033/f7`, `g035/d4`, `g040/l6`. The retained private log preserves earlier wording and its corrections. This
record does not claim runtime reproduction.
