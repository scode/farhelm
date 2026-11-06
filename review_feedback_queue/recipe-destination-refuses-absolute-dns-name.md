# Recipe destination refuses absolute dns name

Reviewed commit: 68790603037bb51ccd8655e4a948c7ffb747760a

## TLDR

A valid final DNS dot is refused in recipe tunnel destinations and agent endpoint hostnames.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `crates/farhelm-proto/src/tensorlake/recipes.rs:370-377 at 68790603037bb51ccd8655e4a948c7ffb747760a`.

Source assessment (read with the later corrections below) (review record g063/c2, event 12):

Reviewed commit: 68790603037bb51ccd8655e4a948c7ffb747760a. TLDR: Users cannot save a recipe tunnel destination written
as an absolute DNS name such as api.example.com.; the shared validator incorrectly calls that valid spelling an invalid
host. The same helper also prevents endpoint URLs such as https://api.example.com./v1 from passing its structural gate.
Mechanism: check_tunnel_host at 358-379 splits on '.', then requires every piece to be nonempty at 370-371, treating the
terminal root label as an invalid interior empty label. check_recipe applies it to tunnel hosts and check_base_url calls
it at 432. SPEC_impl.md, Tensorlake recipes (introducing revision lines 5739-5745), says tunnel hosts are DNS names,
IPv4 literals or bracketed IPv6 literals in the shape ssh -R can carry without quoting; it does not state an
absolute-name exclusion. SPEC.md Topology describes the tunnel destination as a host and port seen from the helm and the
endpoint as an http(s) base URL. The function's own nonempty-label comment describes its implementation but is not an
authoritative supported-grammar exception. Primary DNS evidence: RFC1034 section 3.1,
https://www.rfc-editor.org/rfc/rfc1034.html#section-3.1, explicitly defines the terminal empty root label and printed
absolute names ending in a dot. Primary SSH evidence: https://man.openbsd.org/ssh.1#R specifies remote forwards with a
host field. OpenSSH portable V_10_0_P1 readconf.c,
https://github.com/openssh/openssh-portable/blob/V_10_0_P1/readconf.c#L3030-L3080, parse_fwd_field, only interprets
colon separators, slash path markers and escapes; a dot is ordinary host text. parse_forward at 3169-3211 copies a
four-field destination host unchanged and accepts its non-path valid-port form, with no terminal-dot exclusion. Actual
pinned Farhelm consumer, crates/farhelm-helm/src/tensorlake/tunnels.rs:237-244 at
8a8c9579e04299410652f0e4681975a7198dfc7b, interpolates the stored host into 127.0.0.1:<sandbox-port>:<host>:<port>,
which has no separator ambiguity for api.example.com. The concrete defect is the refusal itself; no stronger downstream
resolver, routing, security or irreversible-loss harm is claimed. Suggested bucket: other (correctness/convenience).
Confidence: confirmed static validation plus primary DNS/SSH syntax; no live lookup or tunnel needed to establish
rejection. Suggested fix: permit a single terminal root dot while retaining label checks for interior pieces and
preserving the user's spelling, and add focused validation cases for tunnel hosts and endpoint authorities. Temporal
status: introduced with recipe validation at this reviewed commit, before the later live tunnel consumer;
original-to-tip proto diff previously inspected leaves check_tunnel_host unchanged through
8a8c9579e04299410652f0e4681975a7198dfc7b. proposed_drop: false, superseding the earlier convenience-based set-aside.
possible_cover: unknown; parent handles filters and coverage. Parent-question reconciliation under existing ID c2, not
novel discovery. No unresolved c2 premise remains for this bounded refusal finding.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g063/c2`; supporting records
`g065/g063-c2`, `g064/f12`. The retained private log preserves earlier wording and its corrections. This record does not
claim runtime reproduction.
