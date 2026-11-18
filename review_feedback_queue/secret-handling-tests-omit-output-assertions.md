# Secret-handling tests miss some display and diagnostic surfaces

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

Credential tests can pass even if a token, login file, or device credential appears outside the intended entry field or
request, because they rarely look for the secret in the painted frame, refusal text, or logs.

## Details

Haiku test pass, checked by gpt-6.2-sol and spot-checked at the reviewed commit. Suggested bucket **other**. Confidence
is source-confirmed for the missing assertions. This is a test gap. GUI paths below are relative to
`crates/gui/farhelm-gui/src/`.

### Painted and accessibility output

- H-T2-3, `components/tests/text_input.rs:173–196`. The masked-input test correctly searches the painted frame and
  proves that ordinary text paints. Check the masked node's exposed value and accessible label too.
  `hosts/add/tensorlake_tests.rs:147–153` shows how to read a frame node's value. Keep the model value check as the
  premise that typing actually happened.
- H-T6-7, `settings/credential_tests.rs:467–491,642–700,834–917`, and `settings/tensorlake_tests.rs:198–275`. The GitHub
  refusal checks the error label exactly and verifies that the draft and resend body keep the token. The Codex chooser
  checks the field model after a file read and the expected read error. The Tensorlake cases check outcomes, metadata,
  the typed model and the body. Add negative assertions for the fictional secret in the full painted frame and exposed
  node values during editing, after a refused save, and after a successful save or file read. A scoped trace capture can
  check that save, failed delivery and bad-file diagnostics omit the sentinel. GitHub create and replacement flows at
  `settings/credential_tests.rs:300–331,376–408` supply real test tokens and a partial-delivery response.
- H-T7-1 and H-T7-2, `settings/tests.rs:3026–3060,3114–3172`. The ntfy tests compare submitted bodies, clearing hints
  and the cleared model. Search the frame for the fictional access token before and after save, server change and the
  in-flight clear. If logging is asserted here, keep the failed-send test separate from the form test and use a scoped
  capture.

### Device credential

- H-T1-3, `crates/farhelm-helm-client/src/transfers/tests.rs:16,323–371`. The drop, pre-send and file-error cases check
  variant and upload outcome. Search the Display and Debug forms and captured diagnostics for the fixture's device
  credential while retaining those semantic checks.
- H-T1-4, `crates/farhelm-helm-client/tests/real_helm.rs:306–339,341–385`. The wrong-credential connection test checks
  401 and retry. The transfer assertions already compare complete refusal messages, covering the message field on those
  routes. Add assertions on captured connection and transfer logs and formatted errors for both the actual and rejected
  fixture credentials. Avoid putting a failed assertion's credential value in its diagnostic.

Possible cover: `masked-fields-expose-secrets-to-text-services.md`, `codex-login-file-rendered-in-cleartext.md`,
`gui-command-debug-includes-secret-body.md`, and `websocket-trace-logs-desktop-credential.md` concern production paths
and may gain tests for those routes. Here the gap is broader. SPEC.md “Settings” specifies write-only secrets and
SPEC_impl.md “Named credential catalog” keeps decoder and database diagnostics out of replies and logs. Launcher and
recipe read fixtures carry paths, credential names, or saved-key status only. Those make poor stand-ins for actual
secret input.
