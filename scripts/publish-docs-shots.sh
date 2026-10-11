#!/usr/bin/env bash
# Publish the docs website's screenshots (docs/docs-shots/SPEC.md).
#
# The PNGs never land on main. Each publish builds one snapshot, a root commit
# holding every screenshot the docs pages reference, and points the custom ref
# refs/docs-assets/keep at a keeper commit whose parents are the new snapshot
# and the earlier snapshots still inside the retention window (below). The website
# reads the images from that snapshot's raw GitHub URL, recorded in the
# manifest website/src/data/docs-shots.json, the one file on main a publish
# changes.
#
# Why a keeper under a custom ref: a snapshot that drops out of the keeper is
# reachable from nothing, so GitHub may garbage-collect it, which is the point
# (old images should not pile up anywhere). A snapshot stays in the keeper
# until six weeks after a newer one replaced it, so the docs at any release
# tag from the last six weeks keep their images: a tag pins whatever snapshot
# was current when it was cut, and that snapshot stopped being current only
# when its successor was published. The snapshot main's manifest pins is
# always kept, whatever its age: main as the remote has it, not just this
# checkout, which may be older than main or carry a manifest change that
# never landed, so pinning only the local manifest's snapshot could let the
# live docs' images go. The ref is outside refs/heads/ so that clones
# never fetch any of it: `jj git fetch` and a default `git fetch` only pull
# branches.
#
# This script is the ONLY thing that pushes the ref; an agent typing that push
# by hand has left the procedure. The ref is a constant and the push spells it
# fully qualified, with a lease on the keeper this run read, so a force-push
# can neither land on another ref nor erase a publish that raced this one.
#
# Usage:
#   scripts/publish-docs-shots.sh [--dry-run] [--checkout DIR] [--skip-raw-check]
#   scripts/publish-docs-shots.sh --self-test
#
# Screenshots are regenerated in bulk: a publish takes every shot the docs
# pages reference (by name, in a <Screenshot name="..."> element) from one
# complete run of scripts/docs-screenshots.sh, never a mix of runs, and
# refuses a one-page capture (--only), which is for drafting. That run's
# record (capture.json beside the shots) names the main commit it captured
# from and the checksum of every PNG it wrote, which each shot published must
# match; the manifest keeps the commit as `captured_from` and `captured_at`, so
# the next refresh can read main's history since then for UI changes. A
# captured shot no page references is left out. Two changes that both moved
# the manifest conflict on it; resolve that by rebasing onto the newer one
# and capturing and publishing again, never by editing the manifest.
#
# The one exception to "from the capture" is a published release shot: a
# shot a release's screenshot list names (website/src/release-notes/*.json)
# that the manifest lists as published. Release notes do not change once
# published, so those images are frozen (docs/docs-shots/SPEC.md, "Release
# shots are frozen"), and the capture skips them. Each one is taken byte for
# byte from the snapshot this checkout's manifest pins, which the keeper
# always retains; if that snapshot cannot be read or lacks the file, the
# publish refuses before pushing anything. A capture record that holds a
# published release shot anyway is refused unless it lists it under
# recaptured_release_shots, which only `scripts/docs-screenshots.sh
# --recapture-release-shot NAME` writes, for a deliberate correction; a
# capture by tooling from before the freeze therefore cannot replace one.
# A release shot that is not yet published (a new release's first
# screenshot) comes from the capture like any other.
#
# --dry-run does every check and builds both commits, then prints what it
# would push and write. --checkout publishes from another checkout (the
# self-test's). --skip-raw-check skips confirming that GitHub serves the new
# snapshot, for a remote that is not GitHub (the self-test's).
# --self-test exercises the whole thing against a bare repository, with no
# network; it is this script's validation.
#
# Every commit is built in a throwaway directory from `mktemp -d`. No git
# command here runs against the checkout except read-only ones (the origin
# URL, the source revision, the committer identity).
#
# NOTE: no reliance on `set -e` (inert under some harnesses); every step
# carries its own guard. Linux only, like the capture: it uses bash 4 and, in
# the self-test, GNU date.

# What is published and where. Constants on purpose; see the header.
readonly REF="refs/docs-assets/keep"
readonly RETENTION_DAYS=42
readonly LOCAL_DIR="website/public/docs-shots-local"
readonly MANIFEST="website/src/data/docs-shots.json"
readonly CONTENT_DIR="website/src/content/docs"
# Each release's screenshot list for the Release notes page, a JSON file named
# after the version that attaches shots to that release's entries
# (website/AGENTS.md, "Release notes"). Not pages, but the shots they name are
# published like any.
readonly RELEASE_NOTES_DIR="website/src/release-notes"
# Written by scripts/docs-screenshots.sh after a complete capture, and deleted
# by a one-page one.
readonly CAPTURE_RECORD="$LOCAL_DIR/capture.json"
# Below this a capture is not a picture of the UI, whatever `file` says.
readonly MIN_BYTES=2000

script_path="$(cd "$(dirname "$0")" && pwd)/$(basename "$0")" || exit 1
script_repo="$(cd "$(dirname "$0")/.." && pwd)" || exit 1

die() {
  echo "publish-docs-shots: $*" >&2
  exit 1
}

# The raw-URL base for a github remote, from either ssh or https spellings.
raw_base() {
  local remote="$1" path
  case "$remote" in
    git@github.com:*) path="${remote#git@github.com:}" ;;
    ssh://git@github.com/*) path="${remote#ssh://git@github.com/}" ;;
    https://github.com/*) path="${remote#https://github.com/}" ;;
    *) return 1 ;;
  esac
  path="${path%.git}"
  path="${path%/}"
  printf 'https://raw.githubusercontent.com/%s' "$path"
}

# Every shot name the docs pages and the release notes' screenshot lists
# reference, one per line, sorted. A name is `<page>/<shot>` in lowercase kebab
# case, the form the capture writes.
referenced_shots() {
  python3 - "$1/$CONTENT_DIR" "$1/$RELEASE_NOTES_DIR" <<'EOF' || die "cannot read the docs pages"
import json, pathlib, re, sys
names = set()
def add(source, name):
    if not isinstance(name, str) or not re.fullmatch(r"[a-z0-9]+(-[a-z0-9]+)*/[a-z0-9]+(-[a-z0-9]+)*", name):
        raise SystemExit(f"{source}: shot name {name!r} is not <page>/<shot> in lowercase kebab case")
    names.add(name)
pages, releases = (pathlib.Path(arg) for arg in sys.argv[1:])
for page in pages.rglob("*.md*"):
    # A <Screenshot> inside an MDX comment or a code fence renders nothing,
    # so it is not a reference.
    text = re.sub(r"\{/\*.*?\*/\}", "", page.read_text(encoding="utf-8"), flags=re.S)
    text = re.sub(r"^(```|~~~).*?^\1", "", text, flags=re.S | re.M)
    for match in re.finditer(r"<Screenshot\b[^>]*?\sname=[\"']([^\"']+)[\"']", text, re.S):
        add(page, match.group(1))
for listing in sorted(releases.glob("*.json")) if releases.is_dir() else []:
    for shot in json.loads(listing.read_text(encoding="utf-8")).get("screenshots", []):
        add(listing, shot.get("name"))
print("\n".join(sorted(names)))
EOF
}

# The snapshot commit main's manifest pins on `remote`, or nothing when the
# remote has no main or main has no manifest (before the first publish lands).
# Fetched shallowly into the private repository `tmp`, so a publish does not
# download main's history. Dies when main cannot be read or its manifest
# cannot be parsed: the caller uses the answer to decide what NOT to prune,
# so an unreadable main must stop the publish rather than read as "nothing
# pinned". ls-remote's exit status 2 means "no such ref", the same way the
# keeper's absence is told apart from an unreachable remote.
remote_main_pin() {
  local remote="$1" tmp="$2"
  git ls-remote --exit-code "$remote" refs/heads/main >/dev/null 2>"$tmp/main-ls.err"
  case $? in
    0) ;;
    2) return 0 ;;
    *)
      cat "$tmp/main-ls.err" >&2
      die "cannot read main from $remote to see which snapshot it pins; not pruning blind"
      ;;
  esac
  git -C "$tmp" fetch -q --depth 1 "$remote" "+refs/heads/main:refs/remote-main" ||
    die "cannot fetch main from $remote to see which snapshot it pins; not pruning blind"
  git -C "$tmp" cat-file -e "refs/remote-main:$MANIFEST" 2>/dev/null || return 0
  # A manifest without a full commit hash is as unreadable as broken JSON:
  # reading it as "nothing pinned" would prune the live docs' snapshot.
  git -C "$tmp" show "refs/remote-main:$MANIFEST" | python3 -c '
import json, re, sys
try:
    commit = json.load(sys.stdin)["commit"]
except Exception as error:
    raise SystemExit(f"not a manifest: {error!r}")
if not isinstance(commit, str) or not re.fullmatch(r"[0-9a-f]{40}", commit):
    raise SystemExit(f"its commit is not a full commit hash: {commit!r}")
print(commit)' || die "cannot read $MANIFEST on main; not pruning blind"
}

# The snapshot commit the manifest pins, or nothing before the first publish.
manifest_commit() {
  local file="$1/$MANIFEST"
  test -f "$file" || return 0
  python3 -c 'import json, sys; print(json.load(open(sys.argv[1])).get("commit") or "")' "$file" || die "cannot read $MANIFEST"
}

# The published release shots, one per line, sorted: every shot a release's
# screenshot list names that the checkout's manifest lists under `shots`,
# meaning the snapshot it pins holds it. Nothing before the first publish.
# The same definition e2e/docs-shots/release-shots.ts and
# scripts/docs-screenshots.sh apply.
published_release_shots() {
  python3 - "$1/$MANIFEST" "$1/$RELEASE_NOTES_DIR" <<'EOF' || die "cannot read which release shots are published"
import json, pathlib, sys
manifest, releases = (pathlib.Path(arg) for arg in sys.argv[1:])
published = set(json.loads(manifest.read_text(encoding="utf-8")).get("shots", {})) if manifest.is_file() else set()
listed = set()
for listing in sorted(releases.glob("*.json")) if releases.is_dir() else []:
    listed.update(shot.get("name") for shot in json.loads(listing.read_text(encoding="utf-8")).get("screenshots", []))
print("\n".join(sorted(listed & published)))
EOF
}

# Check the capture record against the shots to publish and say where each
# comes from. Arguments: the checkout, the published release shots as one
# newline-separated argument, then every referenced shot. Prints
# "captured_from captured_at" on the first line, then each published release
# shot the capture left alone, one per line, for the caller to carry over
# from the pinned snapshot. Every other referenced shot must be on disk
# exactly as the capture wrote it. Refuses anything else: no record, an
# incomplete one, a shot the run did not produce or that has changed since,
# or a published release shot the capture holds without listing it as
# deliberately recaptured.
read_capture() {
  local repo="$1" frozen="$2"
  shift 2
  local file="$repo/$CAPTURE_RECORD"
  test -f "$file" || die "publishing needs one complete capture of every page; run scripts/docs-screenshots.sh without --only"
  python3 - "$file" "$repo/$LOCAL_DIR" "$frozen" "$@" <<'EOF' || die "cannot publish these shots; run scripts/docs-screenshots.sh without --only, then publish"
import hashlib, json, re, sys
record_path, local, frozen, *names = sys.argv[1:]
frozen = set(frozen.split())
record = json.load(open(record_path))
if record.get("complete") is not True:
    raise SystemExit("the capture on disk is not complete")
sha, at = record.get("captured_from", ""), record.get("captured_at", "")
if not re.fullmatch(r"[0-9a-f]{40}", sha) or not at:
    raise SystemExit("the capture record lacks its main commit or time")
shots = record.get("shots") or {}
recaptured = record.get("recaptured_release_shots", [])
if not isinstance(recaptured, list):
    raise SystemExit("the capture record's recaptured_release_shots is not a list")
carried = []
for name in names:
    if name in frozen and name not in shots:
        if name in recaptured:
            raise SystemExit(f"the capture record says it recaptured {name} but holds no such shot")
        carried.append(name)
        continue
    if name in frozen and name not in recaptured:
        raise SystemExit(
            f"the capture holds {name}, a published release shot, without marking it as deliberately recaptured; "
            "published release shots stay as published unless scripts/docs-screenshots.sh --recapture-release-shot names them"
        )
    if name not in shots:
        raise SystemExit(f"a page references the shot {name}, which the capture did not produce; add it to e2e/docs-shots/ and capture again")
    try:
        data = open(f"{local}/{name}.png", "rb").read()
    except OSError:
        raise SystemExit(f"the shot {name} is missing from {local}")
    if hashlib.sha256(data).hexdigest() != shots[name]:
        raise SystemExit(f"the shot {name} changed after the capture that recorded it")
print(sha, at)
for name in carried:
    print(name)
EOF
}

# The manifest's recorded capture commit, or nothing.
manifest_captured_from() {
  local file="$1/$MANIFEST"
  test -f "$file" || return 0
  python3 -c 'import json, sys; print(json.load(open(sys.argv[1])).get("captured_from") or "")' "$file" || die "cannot read $MANIFEST"
}

# Refuse anything that is not a plausible capture.
check_png() {
  local file="$1" bytes
  test "$(file --brief --mime-type "$file")" = "image/png" || die "$file is not a PNG"
  bytes=$(wc -c <"$file") || die "cannot size $file"
  test "$bytes" -ge "$MIN_BYTES" || die "$file is only $bytes bytes"
}

# Pin `commit` in the manifest with the capture it came from, and each shot's
# pixel size read from the PNGs under `tree`. The only writer of the manifest.
write_manifest() {
  local repo="$1" base="$2" commit="$3" captured_from="$4" captured_at="$5" tree="$6"
  shift 6
  mkdir -p "$(dirname "$repo/$MANIFEST")" || die "mkdir failed"
  python3 - "$repo/$MANIFEST" "$base" "$commit" "$captured_from" "$captured_at" "$tree" "$@" <<'EOF' || die "cannot write $MANIFEST"
import json, struct, sys
out, base, commit, captured_from, captured_at, tree, *names = sys.argv[1:]
shots = {}
for name in names:
    with open(f"{tree}/{name}.png", "rb") as png:
        header = png.read(24)
    width, height = struct.unpack(">II", header[16:24])
    shots[name] = {"width": width, "height": height}
manifest = {
    "_comment": "Written by scripts/publish-docs-shots.sh; never edit by hand. See docs/docs-shots/SPEC.md.",
    "base": base,
    "commit": commit,
    "captured_from": captured_from,
    "captured_at": captured_at,
    "shots": shots,
}
with open(out, "w", encoding="utf-8") as f:
    json.dump(manifest, f, indent=2, sort_keys=True)
    f.write("\n")
EOF
}

# Build the snapshot and keeper and, unless dry, push them and write the
# manifest. `repo` is the checkout whose pages, captures, origin, and
# manifest are used.
publish() {
  local repo="$1" dry="$2" raw_check="$3"
  local remote base source tmp old_keep newest pinned main_pinned name file tree empty snapshot keep cutoff parent replaced url
  local capture captured_from captured_at frozen
  local -a names parents carried
  local -A carry=()
  local tool
  for tool in git file python3; do
    command -v "$tool" >/dev/null || die "needs $tool"
  done
  if [ "$raw_check" = true ]; then
    command -v curl >/dev/null || die "needs curl to confirm GitHub serves what it publishes"
  fi
  remote="$(git -C "$repo" config --get remote.origin.url)" || die "the checkout has no origin remote"
  base="$(raw_base "$remote")" || die "origin is not a github remote: $remote"
  source="$(git -C "$repo" rev-parse --short HEAD 2>/dev/null)" || source="unknown"
  mapfile -t names < <(referenced_shots "$repo")
  test -n "${names[0]:-}" || die "no docs page references a screenshot; nothing to publish"
  pinned="$(manifest_commit "$repo")" || exit 1
  frozen="$(published_release_shots "$repo")" || exit 1
  capture="$(read_capture "$repo" "$frozen" "${names[@]}")" || exit 1
  mapfile -t carried < <(tail -n +2 <<<"$capture")
  capture="${capture%%$'\n'*}"
  captured_from="${capture%% *}"
  captured_at="${capture#* }"
  for name in "${carried[@]}"; do carry[$name]=1; done

  tmp="$(mktemp -d)" || die "mktemp failed"
  # shellcheck disable=SC2064 # expand now: the path is fixed for this run
  trap "rm -rf '$tmp'" EXIT
  git -C "$tmp" init -q || die "git init failed"
  local -a git_id=(-c "user.name=$(git -C "$repo" config user.name || echo farhelm)"
    -c "user.email=$(git -C "$repo" config user.email || echo farhelm@invalid)" -c commit.gpgsign=false)

  # The current keeper, which holds every retained snapshot, newest first.
  # Missing only before the very first publish. ls-remote's exit status 2
  # means "no such ref", which is how a first publish is told apart from a
  # remote that cannot be reached.
  old_keep=""
  newest=""
  git ls-remote --exit-code "$remote" "$REF" >/dev/null 2>"$tmp/ls.err"
  case $? in
    0)
      git -C "$tmp" fetch -q "$remote" "+$REF:refs/old-keep" || die "cannot fetch $REF"
      old_keep="$(git -C "$tmp" rev-parse refs/old-keep)" || die "cannot read the fetched keeper"
      newest="$(git -C "$tmp" rev-parse "$old_keep^1")" || die "cannot read the newest snapshot"
      ;;
    2) ;;
    *)
      cat "$tmp/ls.err" >&2
      die "cannot read $REF from $remote"
      ;;
  esac

  # A published release shot the capture left alone comes from the snapshot
  # this checkout's manifest pins, which the keeper always retains as a
  # parent, so the fetch above brought it along. Checked here, before any
  # push, so a missing source refuses the publish instead of dropping the
  # shot from the release notes.
  if [ "${#carried[@]}" -gt 0 ]; then
    test -n "$old_keep" ||
      die "the published release shots (${carried[*]}) must come from the pinned snapshot, but $remote has no $REF"
    git -C "$tmp" cat-file -e "$pinned^{commit}" 2>/dev/null ||
      die "the published release shots (${carried[*]}) must come from the pinned snapshot $pinned, which $REF no longer holds"
  fi
  mkdir -p "$tmp/tree" || die "mkdir failed"
  for name in "${names[@]}"; do
    file="$tmp/tree/$name.png"
    mkdir -p "$(dirname "$file")" || die "mkdir failed"
    if [ -n "${carry[$name]:-}" ]; then
      git -C "$tmp" cat-file -e "$pinned:$name.png" 2>/dev/null ||
        die "the published release shot $name is missing from the pinned snapshot $pinned"
      git -C "$tmp" cat-file blob "$pinned:$name.png" >"$file" || die "cannot read $name from $pinned"
    else
      cp "$repo/$LOCAL_DIR/$name.png" "$file" || die "cannot copy $name"
    fi
    check_png "$file"
  done

  tree="$(cd "$tmp/tree" && GIT_DIR="$tmp/.git" GIT_INDEX_FILE="$tmp/index" git add -A . && GIT_DIR="$tmp/.git" GIT_INDEX_FILE="$tmp/index" git write-tree)" ||
    die "cannot build the snapshot tree"
  # The newest snapshot already holds exactly these pixels: push nothing, but
  # still record the capture, since a regeneration that changed nothing is
  # worth knowing about when the next refresh reads history from it (and the
  # manifest may pin an older snapshot, after a publish whose manifest change
  # never landed).
  if [ -n "$newest" ] && [ "$(git -C "$tmp" rev-parse "$newest^{tree}")" = "$tree" ]; then
    if [ "$pinned" = "$newest" ] && [ "$(manifest_captured_from "$repo")" = "$captured_from" ]; then
      echo "publish-docs-shots: nothing changed since $newest; not publishing"
    elif [ "$dry" = true ]; then
      echo "dry run: nothing to push; would pin $base/$newest, captured from $captured_from, in $MANIFEST"
    else
      write_manifest "$repo" "$base" "$newest" "$captured_from" "$captured_at" "$tmp/tree" "${names[@]}"
      echo "publish-docs-shots: $newest already holds these shots; recorded the capture from $captured_from in $MANIFEST"
    fi
    return 0
  fi
  snapshot="$(git -C "$tmp" "${git_id[@]}" commit-tree "$tree" -m "docs screenshots captured from main at $captured_from (checkout $source)")" ||
    die "cannot build the snapshot commit"

  # Retention. Each earlier snapshot was current until the next newer one
  # replaced it; keep it while that replacement is younger than the window,
  # so a tag cut while it was current keeps its images for the full window.
  # The snapshots this checkout's manifest and main's manifest pin are kept
  # regardless. Main is read only when there is a keeper to prune.
  parents=(-p "$snapshot")
  if [ -n "$old_keep" ]; then
    main_pinned="$(remote_main_pin "$remote" "$tmp")" || exit 1
    cutoff=$(($(date +%s) - RETENTION_DAYS * 86400))
    replaced=$(date +%s)
    for parent in $(git -C "$tmp" rev-list --parents -n 1 "$old_keep" | cut -d' ' -f2-); do
      if [ "$replaced" -ge "$cutoff" ] || [ "$parent" = "$pinned" ] || [ "$parent" = "$main_pinned" ]; then
        parents+=(-p "$parent")
      fi
      replaced="$(git -C "$tmp" log -1 --format=%ct "$parent")" || die "cannot date $parent"
    done
  fi
  empty="$(git -C "$tmp" mktree </dev/null)" || die "cannot build the empty tree"
  keep="$(git -C "$tmp" "${git_id[@]}" commit-tree "$empty" "${parents[@]}" \
    -m "docs screenshots replaced less than $RETENTION_DAYS days ago")" || die "cannot build the keeper commit"
  url="$base/$snapshot"

  if [ "$dry" = true ]; then
    echo "dry run: would push $keep to $REF (keeping $((${#parents[@]} / 2)) snapshots)"
    echo "dry run: would pin $url in $MANIFEST"
    return 0
  fi
  git -C "$tmp" push -q "$remote" "$keep:$REF" "--force-with-lease=$REF:$old_keep" ||
    die "push to $REF failed (another publish may have raced this one; run again)"

  if [ "$raw_check" = true ]; then
    local tries=0
    until curl -fsSI -o /dev/null "$url/${names[0]}.png"; do
      tries=$((tries + 1))
      test "$tries" -lt 20 || die "pushed, but GitHub does not serve $url/${names[0]}.png; the manifest was not written"
      sleep 3
    done
  fi

  write_manifest "$repo" "$base" "$snapshot" "$captured_from" "$captured_at" "$tmp/tree" "${names[@]}"
  echo "published $snapshot (keeper $keep), captured from main at $captured_from; commit $MANIFEST"
}

# End-to-end check against a bare repository standing in for GitHub. What it
# proves: a page referencing a missing shot and a non-PNG are both refused
# before any push; a dry run pushes and writes nothing; a publish creates only
# refs/docs-assets/keep (never a branch) and a snapshot holding exactly the
# referenced shots, recording the capture's main commit; a partial or
# incomplete capture is refused; a later publish drops shots no page
# references; publishing an unchanged set pushes nothing but records the new
# capture; snapshots older than the window drop out of the keeper while
# younger ones stay, except those pinned by this checkout's manifest or by
# main's on the remote; and an unreadable manifest on main refuses the
# publish before anything is pushed. For published release shots, which are
# frozen: a later capture that lacks one carries it over byte for byte from
# the pinned snapshot; a capture that holds one without the recapture
# marker is refused and pushes nothing; a marked recapture replaces it; and
# a pinned snapshot that is gone or lacks the file refuses the publish
# before anything is pushed. Each publish runs as a child process
# whose own environment points git at a test config that rewrites the
# GitHub-shaped origin to the bare repository, so this process's environment
# never changes.
self_test() {
  local work bare checkout cfg page fail=0 keep snapshot
  local origin="git@github.com:example/repo.git"
  command -v convert >/dev/null || die "self-test needs ImageMagick's convert"
  work="$(mktemp -d)" || die "mktemp failed"
  bare="$work/remote.git"
  checkout="$work/checkout"
  cfg="$work/gitconfig"
  git init -q --bare "$bare" || die "bare init failed"
  printf '[url "%s"]\n\tinsteadOf = %s\n' "$bare" "$origin" >"$cfg" || die "cannot write the test gitconfig"
  run() { env GIT_CONFIG_GLOBAL="$cfg" "$script_path" --checkout "$checkout" --skip-raw-check "$@"; }
  # What a complete capture leaves beside the shots, from the main commit $1:
  # every PNG on disk, complete unless $2 is false, and the published release
  # shots named from $3 onward listed as deliberately recaptured, as
  # scripts/docs-screenshots.sh --recapture-release-shot records them.
  captured() {
    python3 - "$checkout/$LOCAL_DIR" "$1" "${2:-true}" "${@:3}" >"$checkout/$CAPTURE_RECORD" <<'EOF' || die "cannot write a test capture record"
import hashlib, json, pathlib, sys
local, sha, complete, *recaptured = sys.argv[1:]
root = pathlib.Path(local)
shots = {str(p.relative_to(root))[: -len(".png")]: hashlib.sha256(p.read_bytes()).hexdigest() for p in root.rglob("*.png")}
print(json.dumps({"complete": complete == "true", "captured_from": sha, "captured_at": "2026-10-03T12:00:00Z", "shots": shots, "recaptured_release_shots": recaptured}))
EOF
  }
  # A publish that must be refused for the reason `pattern` names, and push
  # nothing. Matching the message matters where a fixture could trip more
  # than one refusal: frozen release shots add a refusal of their own, and a
  # case meant to prove another must not pass because of it.
  refused() {
    local pattern="$1" label="$2" before out
    before="$(remote_git rev-parse -q --verify "$REF" || echo none)"
    out="$(run 2>&1)" && { echo "FAIL $label: it published"; fail=1; return; }
    grep -q -- "$pattern" <<<"$out" || { echo "FAIL $label: refused for another reason: $out"; fail=1; }
    test "$(remote_git rev-parse -q --verify "$REF" || echo none)" = "$before" || { echo "FAIL $label: the ref moved"; fail=1; }
  }
  # The git blob id of a file's bytes: equal ids mean byte-identical images.
  blob_of() { git hash-object "$1"; }
  local main1=1111111111111111111111111111111111111111 main2=2222222222222222222222222222222222222222
  remote_git() { git -C "$bare" "$@"; }

  page="$checkout/$CONTENT_DIR/docs/page.mdx"
  mkdir -p "$(dirname "$page")" "$checkout/$LOCAL_DIR/page" "$checkout/$LOCAL_DIR/notes" "$checkout/$RELEASE_NOTES_DIR" || die "mkdir failed"
  printf -- '---\ntitle: t\n---\n\n<Screenshot name="page/one" alt="a" />\n<Screenshot\n  name="page/two"\n  alt="b"\n/>\n' >"$page"
  # A release's screenshot list, naming a shot no page references, so the
  # first snapshot holding notes/v1 is the check that the release notes'
  # screenshot lists are read at all. Once published, notes/v1 is the frozen
  # release shot the later cases carry over, refuse, and recapture.
  printf -- '{"screenshots": [{"entry": 1, "name": "notes/v1", "alt": "c"}]}\n' >"$checkout/$RELEASE_NOTES_DIR/v1.0.0.json"
  local release="notes/v1" release_png="$checkout/$LOCAL_DIR/notes/v1.png"
  git -C "$checkout" init -q || die "checkout init failed"
  git -C "$checkout" -c user.name=t -c user.email=t@invalid -c commit.gpgsign=false commit -q --allow-empty -m init || die "commit failed"
  git -C "$checkout" config user.name t
  git -C "$checkout" config user.email t@invalid
  git -C "$checkout" remote add origin "$origin" || die "remote add failed"
  # Noise, so the PNGs clear the size floor for the right reason.
  convert -size 120x80 plasma:fractal "$checkout/$LOCAL_DIR/page/one.png" || die "cannot synthesise a PNG"
  convert -size 120x80 plasma:fractal "$release_png" || die "cannot synthesise a PNG"

  test "$(raw_base git@github.com:scode/farhelm.git)" = "https://raw.githubusercontent.com/scode/farhelm" || { echo "FAIL raw_base ssh"; fail=1; }
  raw_base "file:///elsewhere" >/dev/null && { echo "FAIL raw_base accepted a non-github remote"; fail=1; }

  run >/dev/null 2>&1 && { echo "FAIL published without a capture record"; fail=1; }
  captured "$main1"
  run >/dev/null 2>&1 && { echo "FAIL published with a referenced shot missing"; fail=1; }
  printf 'not a png' >"$checkout/$LOCAL_DIR/page/two.png"
  captured "$main1"
  run >/dev/null 2>&1 && { echo "FAIL published a non-PNG"; fail=1; }
  test -z "$(remote_git for-each-ref)" || { echo "FAIL a refused publish pushed something"; fail=1; }
  convert -size 120x80 plasma:fractal "$checkout/$LOCAL_DIR/page/two.png" || die "cannot synthesise a PNG"
  convert -size 120x80 plasma:fractal "$checkout/$LOCAL_DIR/page/unused.png" || die "cannot synthesise a PNG"
  # A shot that changed after the capture recorded it is refused: the record
  # still has the non-PNG's checksum.
  run >/dev/null 2>&1 && { echo "FAIL published a shot that changed after its capture"; fail=1; }
  # A record whose main commit is not a full commit hash is refused.
  captured "not-a-commit"
  run >/dev/null 2>&1 && { echo "FAIL published a capture without a valid main commit"; fail=1; }
  test -z "$(remote_git for-each-ref)" || { echo "FAIL a refused publish pushed something"; fail=1; }
  captured "$main1"

  run --dry-run >/dev/null || { echo "FAIL dry run"; fail=1; }
  test -z "$(remote_git for-each-ref)" || { echo "FAIL a dry run pushed"; fail=1; }
  test -f "$checkout/$MANIFEST" && { echo "FAIL a dry run wrote the manifest"; fail=1; }

  run >/dev/null || { echo "FAIL first publish"; fail=1; }
  test "$(remote_git for-each-ref --format='%(refname)')" = "$REF" || { echo "FAIL the remote has refs other than $REF"; fail=1; }
  # Spelled out rather than derived from $REF: the property is that clones
  # never fetch screenshots, which holds only while nothing lands under
  # refs/heads/, whatever the constant says.
  test -z "$(remote_git for-each-ref refs/heads/)" || { echo "FAIL a publish created a branch"; fail=1; }
  case "$REF" in refs/heads/* | refs/tags/*) echo "FAIL $REF is a branch or tag"; fail=1 ;; esac
  keep="$(remote_git rev-parse "$REF")"
  snapshot="$(manifest_commit "$checkout")"
  test "$(remote_git rev-list --parents -n 1 "$keep" | wc -w)" -eq 2 || { echo "FAIL first keeper should have one parent"; fail=1; }
  test "$(remote_git rev-parse "$keep^1")" = "$snapshot" || { echo "FAIL the manifest does not pin the keeper's newest snapshot"; fail=1; }
  test "$(remote_git ls-tree -r --name-only "$snapshot" | tr '\n' ' ')" = "notes/v1.png page/one.png page/two.png " ||
    { echo "FAIL the snapshot should hold exactly the referenced shots"; fail=1; }
  grep -q '"width": 120' "$checkout/$MANIFEST" || { echo "FAIL the manifest lacks the shot size"; fail=1; }
  test "$(manifest_captured_from "$checkout")" = "$main1" || { echo "FAIL the manifest lacks the capture's main commit"; fail=1; }

  # From here on notes/v1 is a published release shot, so a capture leaves it
  # alone, as the real one does: it is not on disk and not in the record.
  local release_blob
  release_blob="$(blob_of "$release_png")" || die "cannot hash a test PNG"
  rm "$release_png" || die "cannot remove a test PNG"
  captured "$main1"

  # Bulk only: a capture missing a referenced shot (what a one-page run
  # leaves) is refused, never filled in from an earlier snapshot, and so is a
  # capture whose record says it is incomplete. The carry-over of frozen
  # release shots is no exception to that: page/one is not one.
  mv "$checkout/$LOCAL_DIR/page/one.png" "$work/one.png" || die "cannot move a test PNG"
  refused "the shot page/one is missing" "a partial capture"
  mv "$work/one.png" "$checkout/$LOCAL_DIR/page/one.png" || die "cannot restore a test PNG"
  # Everything else about this record is valid, so only `complete` refuses it.
  captured "$main1" false
  refused "not complete" "an incomplete capture"

  # A full recapture with new pixels publishes a second snapshot, and the
  # published release shot, absent from that capture, is carried over from
  # the snapshot the manifest pins byte for byte.
  local first="$snapshot"
  convert -size 120x80 plasma:fractal "$checkout/$LOCAL_DIR/page/two.png" || die "cannot synthesise a PNG"
  captured "$main1"
  run >/dev/null || { echo "FAIL second publish"; fail=1; }
  test "$(remote_git rev-list --parents -n 1 "$REF" | wc -w)" -eq 3 || { echo "FAIL the keeper should hold both snapshots"; fail=1; }
  snapshot="$(manifest_commit "$checkout")"
  test "$snapshot" != "$first" || { echo "FAIL the second publish did not pin a new snapshot"; fail=1; }
  test "$(remote_git rev-parse "$snapshot:$release.png")" = "$release_blob" ||
    { echo "FAIL a published release shot was not carried over byte for byte"; fail=1; }

  # A recapture from a newer main that changed no pixels pushes nothing but
  # records the newer commit, so the next refresh reads history from there.
  keep="$(remote_git rev-parse "$REF")"
  captured "$main2"
  run >/dev/null || { echo "FAIL unchanged publish"; fail=1; }
  test "$(remote_git rev-parse "$REF")" = "$keep" || { echo "FAIL an unchanged set was pushed"; fail=1; }
  test "$(manifest_captured_from "$checkout")" = "$main2" || { echo "FAIL an unchanged recapture was not recorded"; fail=1; }

  # A capture that holds the published release shot without the recapture
  # marker, as tooling from before the freeze would write, is refused rather
  # than replacing the frozen image. Marked as deliberately recaptured, the
  # same capture replaces it.
  convert -size 120x80 plasma:fractal "$release_png" || die "cannot synthesise a PNG"
  captured "$main2"
  refused "without marking it as deliberately recaptured" "an unmarked recapture of a published release shot"
  captured "$main2" true "$release"
  run >/dev/null || { echo "FAIL a marked recapture of a published release shot"; fail=1; }
  snapshot="$(manifest_commit "$checkout")"
  test "$(remote_git rev-parse "$snapshot:$release.png")" = "$(blob_of "$release_png")" ||
    { echo "FAIL a marked recapture did not replace the published release shot"; fail=1; }
  test "$(blob_of "$release_png")" != "$release_blob" || die "the recaptured test PNG has the old pixels"
  rm "$release_png" || die "cannot remove a test PNG"
  captured "$main2"

  # A page that stops referencing a shot: page/one leaves the next snapshot,
  # page/two stays, and the release's shot stays as the release list names it.
  printf -- '---\ntitle: t\n---\n\n<Screenshot name="page/two" alt="b" />\n' >"$page"
  run >/dev/null || { echo "FAIL publish after dropping a reference"; fail=1; }
  test "$(remote_git ls-tree -r --name-only "$(manifest_commit "$checkout")" | tr '\n' ' ')" = "notes/v1.png page/two.png " ||
    { echo "FAIL an unreferenced shot was published"; fail=1; }

  # Test fixtures for the retention cases: snapshot commits with chosen
  # dates (all sharing the current tree), a keeper over a list of them, and
  # repinning the manifest. Each case installs its own keeper on the remote.
  aged() {
    env GIT_COMMITTER_DATE="$(date -d "$1 days ago" -R)" git -C "$bare" -c user.name=t -c user.email=t@invalid \
      commit-tree "$snapshot^{tree}" -m "aged $1"
  }
  keeper_of() {
    local args=() commit
    for commit in "$@"; do args+=(-p "$commit"); done
    remote_git -c user.name=t -c user.email=t@invalid commit-tree "$(remote_git mktree </dev/null)" "${args[@]}" -m keep
  }
  pin() {
    python3 -c '
import json, sys
manifest = json.load(open(sys.argv[1]))
manifest["commit"] = sys.argv[2]
json.dump(manifest, open(sys.argv[1], "w"))' "$checkout/$MANIFEST" "$1" || die "cannot repin the manifest"
  }
  kept() { remote_git rev-list --parents -n 1 "$REF" | grep -q "$1"; }
  snapshot="$(manifest_commit "$checkout")"

  # A snapshot is kept until six weeks after it was REPLACED, not after it
  # was published: of 10, 45, and 50 days old, the 45-day one was replaced
  # only 10 days ago and stays; the 50-day one was replaced 45 days ago and
  # goes.
  local d10 d45 d50
  d10="$(aged 10)" || die "cannot build an aged snapshot"
  d45="$(aged 45)" || die "cannot build an aged snapshot"
  d50="$(aged 50)" || die "cannot build an aged snapshot"
  remote_git update-ref "$REF" "$(keeper_of "$d10" "$d45" "$d50")" || die "cannot install a test keeper"
  pin "$d10"
  convert -size 120x80 plasma:fractal "$checkout/$LOCAL_DIR/page/two.png" || die "cannot synthesise a PNG"
  captured "$main1"
  run >/dev/null 2>&1 || { echo "FAIL publish over an aged keeper"; fail=1; }
  kept "$d10" || { echo "FAIL the previous newest snapshot was dropped"; fail=1; }
  kept "$d45" || { echo "FAIL a snapshot replaced inside the window was dropped"; fail=1; }
  kept "$d50" && { echo "FAIL a snapshot replaced outside the window was kept"; fail=1; }

  # A lone snapshot published long ago, pinned until today, was current until
  # this very publish: it must stay, or every recent tag loses its images.
  local lone
  lone="$(aged 60)" || die "cannot build an aged snapshot"
  remote_git update-ref "$REF" "$(keeper_of "$lone")" || die "cannot install a test keeper"
  pin "$lone"
  convert -size 120x80 plasma:fractal "$checkout/$LOCAL_DIR/page/two.png" || die "cannot synthesise a PNG"
  captured "$main1"
  run >/dev/null 2>&1 || { echo "FAIL publish over a lone old snapshot"; fail=1; }
  kept "$lone" || { echo "FAIL the long-current snapshot was dropped by the publish replacing it"; fail=1; }

  # The pinned snapshot stays whatever its age, even when retention alone
  # would drop it (here it was replaced 50 days ago).
  local p5 p50 p70
  p5="$(aged 5)" || die "cannot build an aged snapshot"
  p50="$(aged 50)" || die "cannot build an aged snapshot"
  p70="$(aged 70)" || die "cannot build an aged snapshot"
  remote_git update-ref "$REF" "$(keeper_of "$p5" "$p50" "$p70")" || die "cannot install a test keeper"
  pin "$p70"
  convert -size 120x80 plasma:fractal "$checkout/$LOCAL_DIR/page/two.png" || die "cannot synthesise a PNG"
  captured "$main1"
  run >/dev/null 2>&1 || { echo "FAIL publish with an old pin"; fail=1; }
  kept "$p70" || { echo "FAIL the snapshot the manifest pins was dropped"; fail=1; }

  # Main's own manifest pins a snapshot too, read from the remote rather than
  # this checkout: here the checkout pins the newest snapshot while main pins
  # one that retention alone would drop (replaced 50 days ago), and it stays.
  # A manifest on main that cannot be parsed refuses the publish before it
  # pushes anything; a main without a manifest, as before the first publish
  # landed, is fine. (The cases above ran with no main on the remote at all.)
  main_manifest() {
    local src="$work/main-src"
    rm -rf "$src" || die "cannot build a test main"
    mkdir -p "$src/$(dirname "$MANIFEST")" || die "cannot build a test main"
    git -C "$src" init -q || die "cannot build a test main"
    # A parent commit first, so the publisher's depth-1 fetch of main really
    # is shallow and its push goes out from a shallow repository, as a real
    # main's does.
    git -C "$src" -c user.name=t -c user.email=t@invalid -c commit.gpgsign=false commit -q --allow-empty -m base ||
      die "cannot build a test main"
    if [ -n "$1" ]; then
      printf '%s\n' "$1" >"$src/$MANIFEST" || die "cannot build a test main"
    else
      printf 'no manifest yet\n' >"$src/README" || die "cannot build a test main"
    fi
    git -C "$src" add -A || die "cannot build a test main"
    git -C "$src" -c user.name=t -c user.email=t@invalid -c commit.gpgsign=false commit -q -m main ||
      die "cannot build a test main"
    git -C "$src" push -q --force "$bare" HEAD:refs/heads/main || die "cannot push a test main"
  }
  local m5 m50 m70
  m5="$(aged 5)" || die "cannot build an aged snapshot"
  m50="$(aged 50)" || die "cannot build an aged snapshot"
  m70="$(aged 70)" || die "cannot build an aged snapshot"
  remote_git update-ref "$REF" "$(keeper_of "$m5" "$m50" "$m70")" || die "cannot install a test keeper"
  pin "$m5"
  main_manifest "{\"commit\": \"$m70\"}"
  convert -size 120x80 plasma:fractal "$checkout/$LOCAL_DIR/page/two.png" || die "cannot synthesise a PNG"
  captured "$main1"
  run >/dev/null 2>&1 || { echo "FAIL publish with main pinning an old snapshot"; fail=1; }
  kept "$m70" || { echo "FAIL the snapshot main's manifest pins was dropped"; fail=1; }
  local unreadable out
  convert -size 120x80 plasma:fractal "$checkout/$LOCAL_DIR/page/two.png" || die "cannot synthesise a PNG"
  captured "$main1"
  keep="$(remote_git rev-parse "$REF")"
  for unreadable in "not json" '{"other": "x"}' '{"commit": ""}'; do
    main_manifest "$unreadable"
    out="$(run 2>&1)" && { echo "FAIL published with an unreadable manifest on main: $unreadable"; fail=1; }
    grep -q "on main; not pruning blind" <<<"$out" || { echo "FAIL refused for another reason: $unreadable"; fail=1; }
    test "$(remote_git rev-parse "$REF")" = "$keep" || { echo "FAIL an unreadable main moved the ref"; fail=1; }
  done
  main_manifest ""
  run >/dev/null 2>&1 || { echo "FAIL a main without a manifest blocked publishing"; fail=1; }

  # A published release shot whose pinned snapshot lacks the file (the
  # manifest still lists it) is refused rather than dropped from the release
  # notes or taken from anywhere else.
  local lacking
  lacking="$(remote_git ls-tree "$snapshot^{tree}" | grep -v $'\tnotes$' | remote_git mktree)" || die "cannot build a test tree"
  lacking="$(remote_git -c user.name=t -c user.email=t@invalid commit-tree "$lacking" -m lacking)" || die "cannot build a test snapshot"
  remote_git update-ref "$REF" "$(keeper_of "$lacking")" || die "cannot install a test keeper"
  pin "$lacking"
  convert -size 120x80 plasma:fractal "$checkout/$LOCAL_DIR/page/two.png" || die "cannot synthesise a PNG"
  captured "$main1"
  refused "is missing from the pinned snapshot" "a published release shot missing from the pinned snapshot"

  # A manifest pinning a snapshot the keeper no longer holds refuses a
  # capture that leaves a published release shot to be carried over from
  # it, but does not block publishing a full recapture, the release shot
  # included.
  pin 0123456789abcdef0123456789abcdef01234567
  convert -size 120x80 plasma:fractal "$checkout/$LOCAL_DIR/page/one.png" || die "cannot synthesise a PNG"
  convert -size 120x80 plasma:fractal "$checkout/$LOCAL_DIR/page/two.png" || die "cannot synthesise a PNG"
  captured "$main1"
  refused "which $REF no longer holds" "a published release shot whose pinned snapshot is gone"
  convert -size 120x80 plasma:fractal "$release_png" || die "cannot synthesise a PNG"
  captured "$main1" true "$release"
  run >/dev/null 2>&1 || { echo "FAIL a vanished pin blocked a full recapture"; fail=1; }
  rm "$release_png" || die "cannot remove a test PNG"

  # A capture too small to be a picture of the UI is refused.
  convert -size 8x8 xc:red "$checkout/$LOCAL_DIR/page/two.png" || die "cannot synthesise a PNG"
  captured "$main1"
  refused "is only" "a tiny PNG"

  # A publish racing another (simulated by a pre-push hook that moves the
  # remote ref while this push is under way) must fail rather than overwrite
  # the other publish. NOTE: git's own push protocol refuses this case even
  # without the lease, so this does not prove the lease; the lease covers the
  # earlier window between this script reading the keeper and starting its
  # push, which a test cannot reach without a seam in the script.
  local hooks raced
  convert -size 120x80 plasma:fractal "$checkout/$LOCAL_DIR/page/two.png" || die "cannot synthesise a PNG"
  captured "$main1"
  hooks="$work/hooks"
  mkdir -p "$hooks" || die "mkdir failed"
  raced="$(remote_git rev-parse "$REF^1")"
  printf '#!/bin/sh\ngit --git-dir="%s" update-ref %s %s\n' "$bare" "$REF" "$raced" >"$hooks/pre-push"
  chmod +x "$hooks/pre-push"
  printf '[core]\n\thooksPath = %s\n' "$hooks" >>"$cfg"
  run >/dev/null 2>&1 && { echo "FAIL a publish overwrote a racing publish"; fail=1; }
  test "$(remote_git rev-parse "$REF")" = "$raced" || { echo "FAIL the racing publish's ref was overwritten"; fail=1; }

  rm -rf "$work"
  test "$fail" -eq 0 || die "self-test failed"
  echo "publish-docs-shots: self-test passed"
}

dry=false
raw_check=true
checkout="$script_repo"
case "${1:-}" in
  --self-test)
    test $# -eq 1 || die "--self-test takes no other arguments"
    self_test
    exit 0
    ;;
esac
while [ $# -gt 0 ]; do
  case "$1" in
    --dry-run) dry=true ;;
    --skip-raw-check) raw_check=false ;;
    --checkout)
      shift
      test -n "${1:-}" || die "--checkout needs a directory"
      checkout="$1"
      ;;
    -h | --help)
      sed -n '2,61p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *) die "unknown argument: $1" ;;
  esac
  shift
done
publish "$checkout" "$dry" "$raw_check"
