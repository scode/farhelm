# Private IPv6 proxy URLs are outside the fixture privacy guards

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

The fixture checks can approve a screen containing a private IPv6 proxy URL. This is a guard omission for valid
URL/configuration text, not evidence that a private address or credential was published.

## Details

Source coordinates from the original records: `crates/farhelm-supervisor/src/agent_kind/screen_fixtures.rs:189-234`;
`scripts/capture-agent-screens.py:98-100; crates/farhelm-supervisor/src/agent_kind/screen_fixtures.rs:214-234`.
References below use these paths' basenames unless another path or revision is stated.

Recorder disposition: **verified**. Triage priority: **other**. Source-confirmed conditional behavior does not imply an
observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the finding.

Later-turn synthesis rechecked the exact guard and its write interface. The example
http://[fd12:3456:789a::2]:8080/backend-api/codex is a syntactically valid bracketed IPv6 HTTP URL using an fd-prefixed
unique-local address, not a malformed-host bypass. For a captured warning containing that URL, the Rust guard extracts
fd12:3456:789a::2 at 217-218; splitting that host on dot and parsing u8 yields None at 226; it matches neither the
private IPv4 cases at 227-231 nor localhost/host/::1 at 233. Thus contains_local_url returns false and the independent
fixture assertion at 300-304 accepts it. The newly added detector tests only bracketed ::1 for IPv6 (323-343).

The previously inspected scrubber interface remains the same source premise: capture-agent-screens.py:98-102 only lists
bracketed ::1 among IPv6 forms; scrub at 140-148 and check at 150-161 reuse that regex. Assuming the sample address does
not coincidentally match an identity literal, it survives both. Recorder.save at 273-284 writes body/head after those
two calls, so there is no additional URL guard at that immediate write boundary. docs/agent-screen-fixtures.md:54-60
promises replacement/refusal of private-network URLs and asks for manual diff review; a human can catch the omission,
but the machine guards do not cover it. g048 owns the scrubber interface review and will independently check that side;
I do not claim its mandatory coverage.

Confirmed: detector/control-flow omission for the concrete valid IPv6 text. Not observed or claimed: an installed vendor
using that configuration, an actual captured IPv6 warning, a committed/published private URL, credentials in that URL,
credential theft, or any resulting security compromise. This revision makes those limits explicit; it does not lower an
established disclosure consequence, because none was previously established. Suggested bucket: other, unchanged, for the
maintainer-tooling guard omission. Confidence: confirmed from source for the omission; real capture/publication route
not demonstrated. proposed_drop: false. possible_cover: none; g013's existing-screen candidate is a different
source-refuted question, not coverage for an input class this guard misses.

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02. Discovery-turn prerequisite synthesis read own log and latest
relevant g048/g012/g013 records. Preserve f3's confirmed private IPv6 endpoint omission and its other bucket; g048/f1
describes that same omission and should not be separately queued. Preserve the original source basis and exact valid URL
example from prior f3 revisions. Parent identifies f3 as duplicate g048/f1, while g048 proposes f3 as canonical; this
naming choice is the parent's, not a reason to drop the substantive finding. proposed_drop: separate duplicate item
only, if parent retains g048/f1; exact duplicate basis is identical detector branches, private IPv6 input class,
automatic acceptance consequence and host-classification fix. possible_cover: g048/f1.

The incoming g048/f2 userinfo evidence is a distinct, stronger input class and is not absorbed into f3's other bucket.
For http://captureacct:proxysecret@localhost:8080/backend-api/codex or http://captureacct:proxysecret@[::1]:8080/, the
independent Rust checker takes captureacct as its host and the email checker has no dotted domain; the Python sanitizer
likewise expects the host immediately after ://. These forms preserve reusable credential bytes, unlike f3's
address-only example. g048/f2's latest record establishes the vendor premise with official pinned Codex rust-v0.166.0
source: config/mod.rs:4035-4053 and model-provider-info/src/lib.rs:567-570,745-749 preserve the configured base_url
string, and tui/src/chatwidget/model_popups.rs:42-79 displays it without credential redaction. Farhelm drive_codex saves
that menu at capture-agent-screens.py:605-608. This source-supported configurable/displayed URL route resolves the
original missing vendor premise, including for f3's endpoint-only class; no real private IPv6 capture is observed.
g048/f2 remains the canonical credential finding, highest, with no proposed drop here. It establishes acceptance/writing
of credential-bearing configuration text into fixtures intended for the documented public-commit workflow, conditional
on subsequent human publication. It does not establish that real credentials were captured, publicly committed, stolen,
or used. No highest consequence is lowered merely because the tool is maintainer tooling or because no actual leak was
observed. g013/c2 is a source-refuted question about different existing fixture text, not a compensating guard or
coverage for either input class. Confidence: confirmed detector omissions locally; vendor configuration/display premise
supplied by the reviewed g048 primary-source evidence and parent. No new issue is claimed from this reconciliation.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g047/f3`, `g048/f1`.
The retained append-only histories preserve all earlier claims and corrections. Reviewed source references name their
own revisions; the inclusive reviewed tip is the hash above. This review used source inspection only, with no product
runtime experiments or live-install changes.
