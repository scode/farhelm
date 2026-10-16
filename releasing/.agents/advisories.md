# Publishing release advisories

The advisories file withdraws published releases from installed apps: its format and limits, how it is signed, how a
withdrawal is lifted, and what clients do with one. It is the main file of the release advisories reading list in
`releasing/AGENTS.md`; read it in full before preparing an advisory.

What the advisories publication is, and who may author, sign or publish it, is in `releasing/AGENTS.md`, "Rules for
every release". Nothing is live merely because a client or these instructions shipped; an absent advisories file means
nothing to say.

Write UTF-8, starting with exactly `farhelm advisories`, then one withdrawal per line:

```text
farhelm advisories
revoked v1.2.3 short plain-text explanation
```

Use stable release tags, one ASCII space after `revoked` and after the tag, then a reason. Keep each line at most 4096
bytes, each reason at most 240 Unicode characters and the whole file at most 64 KiB. Control characters are removed from
displayed reasons; blank reasons and malformed known directives are ignored with a log line. Last valid duplicate wins.
Unknown directives are ignored permanently by older clients, so additions must use new directives and preserve this
header and the meaning of existing ones. CRLF and a last line without newline are accepted. The signature covers the
whole file, including ignored content.

On the trusted signing host, use a key that installed clients already trust and a **prehashed** minisign signature with
trusted comment exactly `farhelm advisories`. With the maintainer's signing-key path supplied, the signing operation is
`minisign -S -s <key-path> -m advisories -x advisories.minisig -t 'farhelm advisories'`. The external tooling must
verify the resulting body/signature pair against the intended public ring and exact comment before publication, enforce
the caps (signature at most 4 KiB), and publish both files. Rotation constraints still apply: a client without the
signer in its ring ignores that publication. Do not use a release's `farhelm vX.Y.Z` comment. Publishing a temporarily
mismatched pair is a failed fetch, not a partial accepted withdrawal; clients retain their last verified in-run snapshot
until a matching pair arrives.

To lift a withdrawal, remove its line and sign/publish the complete replacement. A header-only signed file lifts all
withdrawals. Deleting the files does not lift a withdrawal already learned during a running app's lifetime. Body 404 is
quiet; other failures are logged as no new information. There is no disk cache or replay protection beyond D3's existing
trust boundary: the site can withhold or replay authenticated information.

A withdrawn running release is warned about, and update checks refuse withdrawn targets. A withdrawn update already
installed and waiting for restart requires manual recovery: the client removes Restart to update and installs nothing
over it, even on demand. An ordinary relaunch may still open that withdrawn version. Do not promise automatic recovery
or choose an older replacement on the strength of an old/missing Running record; a newer supervisor may have migrated
state before failing startup. The follow-up in TODO.md covers stronger attempted-version evidence. The standalone
installer, helm and remote hosts do not read advisories.
