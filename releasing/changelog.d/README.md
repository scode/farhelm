# Changelog fragments

One file per user-facing change that has landed on main since the last stable release, waiting to be curated into
`CHANGELOG.md` when that release is cut. The rules, the front matter, and the curation step are in
`releasing/AGENTS.md`; `releasing/check-changelog.py fragments` reports which changes since the last release still lack
one.

Names are mnemonic and free-form (`cursor-launch.md`, `fix-detach-notification.md`); nothing parses them. The leading
front matter block (`kind:`, optionally `pr:`) is the only structure, and the prose is a draft, not the final entry.
