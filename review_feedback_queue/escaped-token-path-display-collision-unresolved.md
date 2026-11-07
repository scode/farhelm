# Escaped token path display collision unresolved

Reviewed commit: 91b6dd23cac4856065f360e3969f0a9d98763be1

## TLDR

Escaped and literal token-file paths can look identical in approval text. A supported harmful pair of real files remains
unverified after investigation.

## Details

Confidence: unverified after investigation; established source facts and missing premises are distinguished below.
Suggested bucket: other.

Source location: `crates/farhelm-ui/src/approvals.rs:1333-1335 at 91b6dd23cac4856065f360e3969f0a9d98763be1`.

Source assessment (read with the later corrections below) (review record g077/d5, event 27):

Reviewed commit: 91b6dd23cac4856065f360e3969f0a9d98763be1. TLDR: the token-file path on a template approval cannot
distinguish a filename containing an invisible character from a filename containing the literal escape-marker text shown
for that character. Users cannot tell which exact credential file the template names from this newly added row.
Confidence confirmed source-derived display collision. Concrete inputs, written here in JSON notation to distinguish
bytes: path A is "/tokens/key<U+200B>"; path B is "/tokens/key\u200b". launch.rs:698-701 admits both because
github_token_file only requires an absolute path without NUL. approvals.rs:1333-1335 emits either raw path as
CardRow::Value; :635-642 calls display_peer. peer.rs:159-188 converts U+200B to <U+200B> but leaves a literal '<' and
its following ASCII text untouched; proto/text.rs:57-79 includes U+200B in the unsafe set. Both inputs therefore produce
identical row text /tokens/key<U+200B>, while container.rs:112-128 later reads the original path and :315-325 delivers
its distinct contents as GH_TOKEN. No test/runtime or file creation is needed to establish the string collision.
Suggested bucket other for the established inability to identify a path accurately; potential highest if two existing
files differ in credential authority and this ambiguity produces an unintended credential grant. No deployed pair,
lower-trust credential-selection sequence, actual secret exposure or exfiltration route was inspected, so those
consequences are not asserted as established. The original c4 refutation covers markup/control injection and answer
binding, not this distinct escape-notation collision. Caveat/intent evidence: peer.rs:210-226 expressly keeps titles and
paths on the broader readable display_peer alphabet, while display_identity already escapes literal '<' to avoid this
exact class of collision for identities. That broad readability choice is documented, but does not specifically settle
approval of a credential-file reference; no SPEC rule explicitly accepts ambiguous token-file names. Suggested fix:
disambiguate literal marker prefixes on sensitive operational-path approval rows, or add an exact escaped/raw-copy
identity disclosure while preserving readable normal paths. proposed_drop false; possible_cover unknown, parent matches.
Highest-capable consequence is retained for parent judgment rather than silently dropped on the readability comment.
Temporal: no changes to approvals.rs, peer.rs, proto/text.rs or proto/launch.rs through pinned tip
8a8c9579e04299410652f0e4681975a7198dfc7b alter the collision. Distinct from d1 omission of variable launch choices, l2
raw logs, fixed Podman label and the accepted paste hazard.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g077/d5`. The retained private
log preserves earlier wording and its corrections. This record does not claim runtime reproduction.
