### What this was about

Three review findings about maintainer tooling, none of it shipped to users, each triaged by you on 2026-10-05 as "fix
code" and scheduled together.

- **The README demo-video recorder** (`scripts/readme-video.sh`) kept its raw frames in a directory named after the
  `--output` path (`<output>.frames`) and deleted it, recursively, before every recording, without knowing it had made
  it. Pointing `--output` next to an existing directory of that name would erase what was in it.
- **The same recorder** writes one still per named moment of the video into `<name>-stills/` beside the MP4, and after
  encoding it deleted that directory first, whatever it held.
- **The docs-screenshot publisher** keeps each published set of screenshots (a snapshot) until six weeks after a newer
  one replaced it, and always keeps the one the docs' manifest pins. But it read that manifest from the checkout doing
  the publish. A checkout behind main could therefore let the snapshot the live docs use be dropped, after which GitHub
  may delete its images and the docs show broken images until the next capture and publish.

What landed:

- The recorder makes a fresh frames directory for each recording, beside the output (not in `/tmp`, which can be small
  and memory-backed), and removes only that one: after encoding, when a recording fails, and, through a teardown step
  added to the Playwright script that drives the video, when a step of the video fails partway.
- The recorder replaces an existing stills directory only when everything in it is a file named the way it names stills
  (plus the metadata files Finder and Windows Explorer leave behind). Otherwise it stops with a message naming the
  directory and asking you to move it aside. Stills from earlier recordings keep being replaced as before. It checks
  this before recording starts and again before writing the new MP4, so a refusal never comes after a long recording or
  leaves a new video beside old stills.
- The publisher also reads the manifest on main from GitHub (a shallow fetch) and keeps the snapshot it pins. A main
  without a manifest is fine; a main it cannot read, or a manifest there without a valid commit, stops the publish
  before anything is pushed. The docs-screenshot SPEC says so in one sentence, and the publisher's self-test covers it.

### Things you should know

- **The recorder now refuses where it used to delete.** A stills directory holding anything the recorder did not make,
  or a stills path that is a symlink, stops the run. That is the point of the fix, but a run that used to succeed can
  now fail with "move it aside and record again".
- **The frames directory is visible, not hidden** (`<output-name>.frames-XXXXXX` beside the MP4). It is removed on every
  path except a run killed outright; a visible name means such a leftover is easy to spot. Both reviewers found that a
  failed step between start and finish would otherwise leak one directory per failed run, since each run now makes a new
  one; the teardown step closes that.
- **Stills are recognised by name rather than by a marker file**, so directories from recordings made before this change
  need nothing. The cost is that a hand-made file named like a still (`01-something.png`) would be replaced.
- **The recorder fixes have no test in the repository.** The recorder never had one, and the harness used here was a
  throwaway; a follow-up could check it in if you want these rules guarded. A frames directory left by a run killed
  outright has to be deleted by hand.
- **The publisher reads main only when it has something to prune**, so a first publish, or one whose images did not
  change, never contacts main.

### Open questions and possible follow-ups

- None.

### The PRs

- #1649 chore: give the demo-video recorder a private frames directory.
- #1650 chore: refuse to replace a stills directory the recorder did not make.
- #1651 chore: keep the docs screenshots main pins when publishing.

### Checks run, reused and skipped

- Run for the recorder (#1649, #1650): a scratch harness that drives the real recorder with a stand-in browser page and
  real ffmpeg, checking which directories it creates and removes. It covered a fresh and an earlier stills directory,
  foreign files before and during a recording, a symlink, Finder's metadata file, failed starts and finishes, and an
  abandoned recording. The same harness run against main's recorder fails where expected, deleting the foreign files.
  Playwright's `--list` mode loads the changed capture spec. The e2e project has no type checker, and a full
  `scripts/readme-video.sh` run (whole build plus a staged fleet) was not needed, since the harness exercises the
  changed code directly.
- Run for the publisher (#1651): `scripts/publish-docs-shots.sh --self-test` and `shellcheck`, both clean. Copies of the
  script with each new check removed fail the new self-test cases, so those cases tell the behaviors apart.
- `dprint check` on the changed Markdown. Nothing else applies: no Rust, UI, or user-facing code changed.

### Review gate

Each PR was reviewed by Claude Opus 5.5 and gpt-6-astra at high effort, as you required. On #1649 both found the leaked
frames directory after a failed step, now fixed. On #1650 gpt-6-astra found nothing, and Opus raised Finder's metadata
file, a refusal that came only after the new MP4 was written, and wording, all fixed. On #1651 gpt-6-astra found
nothing, and Opus found that a manifest on main without a valid commit was read as pinning nothing (now refused), that
the test never made the fetch genuinely shallow, and that it did not check the refusal's reason; all fixed. Nothing was
declined.
