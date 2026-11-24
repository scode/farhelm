# The signed announcement feed is not bound to its file name or freshness, so the standing brick-test feed or an older feed replays as the real one

Reviewed commit: e159ffc79ab147be44e92a92b59571650a4f322d

## TLDR

The announcement signature does not bind the file name or freshness, so the standing production-signed brick-test feed,
or any older feed, served as the real feed would be accepted permanently by every released Mac app (bucket uncertain;
recorded higher).

## Details

**highest.** Recorded by the trailing review of `1b3fe49a..e159ffc7`; the tip `e159ffc79ab147be44e92a92b59571650a4f322d`
still has it. Bucket uncertain between `high` (reviewer) and `highest`; recorded `highest` under the trailing review's
err-higher rule, since the signature check is the trust boundary meant to stop exactly this. What would move it down: a
distinct test trusted comment or a short per-run expiry on the test entry, or a decision that a replay of display-only,
farhelm.io-linked text is not a security consequence.

### Reviewer record (`g08-announcements-helm/f1`)

Location: `crates/farhelm-helm/src/network_announcements.rs:22`. Summary: Signed announcement feed is not bound to its
filename or freshness: the standing brick-test feed and any older feed replay as the production feed

Revision (turn 3): bucket raised from `other` to `high`, because of g10's input. Turn 2 named the condition for `high`:
the standing test feed being long-lived. g10 f5 shows that the tooling requires exactly that, so the condition is met by
the procedure. Route, details, fix, Reviewed commit and possible_cover are unchanged from the turn-2 record.

New premise (from g10, checked against the cited lines): every brick-test run probes the fixed public URL
https://get.farhelm.io/announcements-test.json and its .minisig (releasing/mac-vm-test/checkpoint_contract.py:49-52;
guest_operations.py:545-548). A 404 fails P2. The pair must use the production keys and comment
(releasing/AGENTS.md:81-84). Only the title is fixed. To keep passing for every future candidate, the entry must target
macOS with no near expiry and no version ceiling. So the standing, publicly fetchable, production-valid test entry
matches every released Mac helm from its min_version onward, indefinitely.

Consequence on the helm side: if get.farhelm.io ever serves that pair as announcements.json, every released Mac helm
whose check runs while it is served permanently gets the 'Farhelm brick test announcement'. The trigger can be
compromise, a CDN or path mapping error, or a publishing slip copying the wrong file. The entry lands in pending and
raises the bell. It then stays in Announcements history forever, because enqueue is permanent by id and there is no
retraction. The signature check, the only defence the helm has against a misbehaving origin, cannot tell the two files
apart.

Suggested bucket: high. This is a rare failure, but it reaches the whole Mac user base at once and its effect is
permanent and cannot be corrected by later feeds. The control meant to stop exactly this does not. Still not `highest`:
a replayed entry shows only maintainer-written plain text, and its link (if any) is limited by safe_link to HTTPS on
farhelm.io, which the origin attacker does not control. No data, credential or process loss is established. What would
move it to `highest`: a replayable signed entry whose instructions or link became unsafe (e.g. withdrawn for safety),
farhelm.io under the same control, or the app acting on announcement content beyond display and a farhelm.io link. What
would move it back to `other`: the maintainer adopting a distinct test trusted comment or a short, per-run expiry, which
removes the long-lived replayable entry, leaving only resurrection of withdrawn production entries.

Confidence: confirmed for the helm's acceptance path (source). The test pair's current publication is taken from the
tooling contract via g10; I made no network query.

proposed_drop: none.

possible_cover: none known. g10 f5 describes the same route from the tooling side and will be deduplicated with this
record.

### Also reported by `g10-brick-announcements/f5`

Location: `releasing/mac-vm-test/checkpoint_contract.py:49-52`. Summary: Brick test requires a permanently public,
production-signed test feed that released helms would accept as the real feed

Reviewed commit: 356718b9d0bca3a27758ec6904688da2f54b9639 (still present at tip
e159ffc79ab147be44e92a92b59571650a4f322d); the publication contract it relies on is releasing/AGENTS.md:78-84 at the
tip.

TLDR: To test announcements, the release procedure keeps a test announcement permanently published on Farhelm's public
download server, signed with the real release key in exactly the way real announcements are. Because the app does not
check which file a signed feed came from, anyone who can make the download server answer the normal announcement address
with that test file (a publishing mistake, a misconfigured mirror or CDN rule, or someone with write access to the
server) makes every released Mac app show users a 'Farhelm brick test announcement'. The brick test design makes such a
replayable, validly signed test feed exist at all times.

Details: Cross-group input from g08 (helm side): the signed feed is not bound to its file name or freshness. From this
group's scope:

- The tooling requires the test feed at a fixed public URL: `ANNOUNCEMENTS_TEST_PATH = 'announcements-test.json'`
  (releasing/mac-vm-test/checkpoint_contract.py:51), probed as https://get.farhelm.io/announcements-test.json and
  .minisig (guest_operations.py:545-548) on every brick test run. The comment at checkpoint_contract.py:49-50 and the
  docs call it 'the standing fixture'/'standing feed': it is meant to stay published between releases, not be published
  per run.
- It must be signed with the production keys and the same purpose: releasing/AGENTS.md:81-84 'The same purpose and keys
  apply to the standing `announcements-test.json` and `announcements-test.json.minisig` used at P2. Keep the test entry
  out of the production feed. Its title is **Farhelm brick test announcement** and it must target the candidate Mac
  without expiring before the test.' The candidate only accepts it if it verifies with RELEASE_KEY_RING and trusted
  comment `farhelm announcements` (SPEC_impl.md, 'Shared preferences and announcements'), the same comment the
  production feed uses, so the tooling cannot pass with a distinguishable purpose.
- Entries: the tooling fixes only the title, `ANNOUNCEMENTS_TEST_TITLE = 'Farhelm brick test announcement'`
  (checkpoint_contract.py:52); body, link, id and targeting are whatever the external publisher wrote. To keep passing
  for future candidates it must target macOS with no max_version below upcoming releases and no near expiry, i.e. it is
  eligible on essentially every released Mac helm. What a user would see if it were served as announcements.json: an
  unread 'New announcement' bell notice and an Announcements dialog titled 'Farhelm brick test announcement' with the
  fixture's body (and possibly a farhelm.io link), once per helm (ids are permanent, so it never repeats).
- The tooling never checks that the standing entry is narrowly targeted or short-lived; nothing in the procedure
  requires an expiry tied to the run. Fix options (spanning g08): give the test feed a distinct signed purpose (e.g.
  trusted comment `farhelm announcements test`) that the helm accepts only under the brick-test override, so the
  production path rejects it; or bind the filename into the trusted comment; and/or require the standing entry to carry
  a short `expires_at` renewed per brick test.

Suggested bucket: other — the route needs control of what get.farhelm.io serves (or a publishing mistake), and the
payload is fixed maintainer-authored benign text with links confined to farhelm.io; it moves to high/highest only if the
standing fixture carried content harmful to users, which nothing shows.

Confidence: likely — confirmed that the tooling requires a standing, public, production-signed, same-purpose feed;
unverified premise: a route by which get.farhelm.io serves the test bytes at announcements.json (operator error or
origin/CDN compromise), and the actual published targeting/expiry of the standing entry (not in the repository).

proposed_drop: none

possible_cover: g08-announcements-helm finding that the signed announcement feed is not bound to its file name or
freshness (same root cause, helm side).

Review evidence (findings logs, delivery audits, dispositions and drop-check verdicts) is retained under run
`71907637-41f3-4337-888c-8515d58c5dbb` as `g08-announcements-helm/f1`, `g10-brick-announcements/f5`.
