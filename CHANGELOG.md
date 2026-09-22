# Changelog

> NOTE: This is a format draft, not a real changelog. The v0.13.0 section is a mock built from the commits on main since
> v0.12.0 as if they were being released today, so the wording is a first pass by the agent and not curated. The v0.12.0
> section is the real one this change would bootstrap. Everything from the first `##` heading down is what
> `CHANGELOG.md` would contain; the notes at the end are about the draft, not part of the file.

Notable user-facing changes in each stable release of Farhelm. Release candidates and dev builds are not listed; their
changes appear under the stable release that follows them. Entries are written for someone running Farhelm, not for
someone reading its source, so internal mechanics are left out unless they change what you have to do.

## v0.13.0 - 2026-09-22

### ✨ Highlights

#### Session archiving is gone

The archive verb and the archived state no longer exist. Any session that was archived becomes visible again in the
session list with its retained output and outcome, and nothing is launched on its behalf. Deleting a session still
archives its owned GitHub checkout when the last reference to that checkout goes away; that part is unchanged.

Agents talking to the supervisor see this as a protocol and JSON schema version bump. An agent built against an older
schema that still sends archive verbs will be refused, so update the helm and supervisor together as usual. (#834)

#### Cursor can be launched from the composer

Cursor is now a harness choice in the launch composer, using the same generic launch path as any other command. This is
launch only: Farhelm does not track a Cursor session's conversation and cannot resume one, so the session shows no
conversation identity and Resume stays unavailable for it. No Cursor configuration is touched and no hooks are
installed. (#844)

### 💥 Breaking

- Session archiving is removed; archived sessions become visible again and the agent protocol and schema versions
  advance. (#834)

### 🚀 Added

- Cursor can be launched from the composer, without conversation tracking or Resume. (#844)
- The sidebar marks each session with its harness's official logo instead of a letter. Claude keeps a Farhelm-drawn mark
  because Anthropic's brand terms do not allow the spark to be reused; Muse Code uses a community icon since it has no
  official one. (#847)
- Codex conversation identity is only captured when the hook can prove it is running in the session's foreground, so a
  stale or foreign report can no longer attach itself to the wrong session. Reports from hook assets older than this
  release are refused rather than trusted, so re-run provisioning on hosts that were set up before it. (#811)

### 🔧 Fixed

- The harness mark in a session row no longer shifts sideways depending on whether the row also shows a permission lock.
  (#850)
- Detaching from a terminal now always reaches the supervisor, even when the browser tab that asked for it went away
  before the detach finished. Previously such a session could stay marked as attached until the connection dropped.
  (#843)
- The `$farhelm help` examples are self-contained again and no longer carry restart caveats that belonged elsewhere.
  (#828)

## v0.12.0 - 2026-09-21

### ✨ Highlights

#### The launch composer looks like the rest of Farhelm

The dialog that starts a new session had its own palette, corner radius, shadow, and selection color. It now uses the
same ones as the main view, and selection is drawn the same way everywhere: the selected session in the sidebar, the
selected terminal tab, and every chosen option in the composer share one look. Recent-setup rows show only the settings
a setup sets explicitly, or "defaults" when it sets none, and line up as columns so the row that differs is easy to
spot. (#819, #820)

### 🔄 Changed

- The launch composer uses the main view's colors, corners, and selection style, and recent-setup rows show only what
  each setup sets. (#819, #820)

### 🔧 Fixed

- Buttons, inputs, and selects in the launch composer render in the UI typeface instead of the platform's sans-serif.
  (#818)

---

## Notes on this draft

Things the sample above is meant to demonstrate, and the choices it embodies, so you can push back on them one at a
time:

- **Two parts per release.** Highlights first, as `####` subsections with prose, for anything that needs more than a
  sentence. Then the categories, where every entry is exactly one bullet. A highlighted item also gets its one-liner in
  its category, so the category lists are complete on their own and Highlights is a reading aid. A release with nothing
  worth a paragraph has no Highlights heading. The cost is the duplicated summary line, visible in v0.12.0 where the
  release is small enough that the one-liner nearly restates the highlight.
- **Heading shape.** `## v0.13.0 - 2026-09-22` is what dist's parser needs: the version comes first after an optional
  `v`, and everything after it is free text. dist also uses that whole heading as the GitHub release name, so
  `v0.13.0 - 2026-09-22` is what the release would be called. If you would rather the release stay named `v0.13.0`, the
  date can go on its own line under the heading instead.
- **Category order.** Breaking, Added, Changed, Fixed, Removed, and a category is omitted when empty. `feat!` and any
  other `!` type land under Breaking, `style` under Changed. The order is the reader's priority order: what might bite
  you first, then what is new, then what merely changed.
- **PR references.** `(#834)` at the end of each entry, comma-separated when several PRs make one entry (#819, #820).
  GitHub autolinks these on the release page and in the repo view; they are plain text anywhere else, which I think is
  fine.
- **Prose intro.** Neither section has one; the format allows one paragraph directly under the release heading when a
  release has a theme. With Highlights present it is mostly redundant, so I would keep it optional and rare.
- **Emoji.** One per category heading, leading, so it reads as an icon: ✨ Highlights, 💥 Breaking, 🚀 Added, 🔄
  Changed, 🔧 Fixed, 🗑️ Removed. Category headings are below the release level, so the parser never looks at them and
  they stay out of the GitHub release name. 💥 rather than 🔥 for Breaking because 🔥 is also the common mark for
  removed code, and 🔄 rather than ♻️ for Changed because recycling reads as refactoring. The release heading itself
  stays emoji-free: anything before the version there breaks parsing, and anything after it lands in the release name.
- **Wording quality.** The #811 and #843 entries were written from the diff because neither commit has a body or PR
  description. They are the kind of entry where a fragment written at PR time would have saved the release-time
  archaeology, and also the kind where I would expect you to rewrite the sentence. The #834 highlight names what the
  user must do, which is the answer I would propose to the earlier open question about breaking entries.
