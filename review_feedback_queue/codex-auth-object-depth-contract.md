# The opaque Codex login contract omits a parser depth bound

Reviewed commit: 49ce8cb76e9f4c841e15122a89264c7c908171dc

## TLDR

The specification says a saved Codex login is validated only as a JSON object no larger than 64 KiB, but the parser also
limits nesting depth. This is a confirmed wording mismatch. No affected login file produced by Codex, login outage or
credential loss has been established.

## Details

Suggested bucket: **other**. Confidence: confirmed contract/implementation mismatch; supported-vendor compatibility
impact remains unverified after investigation. Introduced by `3126f25e98b5468b70cdf6ae2b683365c6699d9d`.

`crates/farhelm-proto/src/credentials.rs:68–85` treats the file as an opaque bounded object but uses ordinary
`serde_json::from_str<Value>`. Cargo.lock selects serde_json 1.0.151 (`5595–5598`); its
`de.rs:59–68,1370–1384,1431–1446` imposes a 128-container recursion bound. A syntactically valid object under 300 bytes
containing 128 nested arrays is therefore outside the implemented acceptance set despite the size/object-only wording in
SPEC.md, Credentials, `1951–1952` and SPEC_impl.md, Named credential catalog, `5907–5908`.

The intended input is the `auth.json` emitted by `codex login` after ChatGPT sign-in, as the editor and user guide
instruct (`crates/farhelm-ui/src/credentials_settings.rs:387`; the sandbox guide at the reviewed revision, `53–55`). The
catalog tests (`credentials_tests.rs:186–208`) and delivery tests use shallow fabricated objects. Delivery uploads the
retained bytes without implementing a vendor schema (`provisioning/credential_delivery.rs:503–546`). Focused source,
docs and vendor searches found no real auth fixture or vendored Codex serializer establishing this depth in a supported
file. No actual credential file was read and no runtime or vendor experiment ran.

The missing practical premise is a Codex-produced, vendor-accepted file within 64 KiB whose container depth reaches the
bound. An upstream supported-version serializer/schema or sanitized vendor-generated fixture accepted by Codex would
settle that. The retained finding is the narrow contract mismatch; it is not a claim that current or future vendor files
reach the limit. Clarifying the bound is proportionate; this evidence does not justify disabling recursion safety or
introducing a more complex parser.

Possible cover: the product may intend only vendor-generated files and implicit parser safety limits, but no explicit
depth exception or exact recording coverage was found. Proposed drop: none for the wording mismatch. Provenance: run
`4ca8e296-4f73-4429-a34b-031af1582085`, `g002/g002-codex-depth`.
