# Sandbox confirmation does not bind the billed size it displays

Reviewed commit: 19f967247150131de3b37127df7f028669ffe269

## TLDR

A Tensorlake checkout launch can admit a newer recipe size than the one shown when the user confirms, if the same named
recipe is edited before admission.

## Details

**high.** Source inspection. Main reference `crates/gui/farhelm-gui/src/launcher/view/sandbox_question.rs:138`.

the prompt freezes the displayed size in SandboxAsk, but the answer discards that payload and calls submit with a
boolean (sandbox_question.rs:48–61, 105–110, 135–143 and 173–179). The request sends recipe name, place and confirmed
flag with no size or revision (farhelm-proto session_requests.rs:78–115. Helm sessions/sandbox.rs:57–65, 89–111).
Settings saves an existing recipe through the authenticated PUT route (settings/recipes/editor.rs:132–150, 530–552. Helm
lib.rs:809–815). tensorlake/recipes.rs:300–341, 564–590 changes the definition under the same name while leaving its
existing built snapshot state. environment/recipes.rs:279–289 requires only that a Tensorlake built snapshot exists.
Provisioning/tensorlake_add.rs:351–360 reads the current definition and lines 393–399 register its size
(environment/mod.rs:116–132). Tensorlake/hosts.rs:404–427 sends that recorded size in the provider request.
SPEC.md:1579–1582 specifies the confirmation displays CPU, memory and disk. SPEC.md:488–489 allows size edits for future
hosts, and SPEC_impl.md:6671–6676 says size changes alone do not invalidate a prepared snapshot. This permits an
eligible same-name change before admission even if the GUI retires a stale prompt on push. Bind the confirmation to the
displayed dimensions or a definition revision and compare against the definition at admission. This existing consent
boundary was found in the managed-checkout path. The root fallback did not introduce it. Bucket: high, billed resources
can exceed what was displayed. Confidence: source-supported admission and provider request. No bill was observed.
Possible coverage: sandbox_view_tests.rs:288–334 checks the displayed size and withholding the request, not a recipe
change at admission.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `checkout/l14`.
