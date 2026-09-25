#!/usr/bin/env bash
# Publish the README hero screenshot (docs/readme-hero/SPEC.md).
#
# The PNG never lands on main. It goes to the `readme-assets` branch as a
# single root commit holding a single file, force-pushed so the branch never
# accumulates history, and README.md and the docs website's landing page are
# both rewritten to reference that commit's raw URL. This script is the ONLY thing that knows the branch name and the
# only thing that pushes it; an agent typing the push by hand has left the
# procedure. Everything that could name the wrong ref is a constant or read
# from the checkout, never an argument, and the push spells the fully
# qualified ref so a force-push cannot land anywhere else.
#
# Usage:
#   scripts/publish-readme-hero.sh [--dry-run] PNG
#   scripts/publish-readme-hero.sh --self-test
#
# --dry-run does every check and builds the commit, then prints the hash and
# URL it would have pushed and written, without pushing or editing either page.
# --self-test exercises the whole thing against a bare repository created
# for the test, with no network; it is the check that guards the push logic
# and it is what CI-equivalent validation of this script means.
#
# The commit is built in a throwaway directory from `mktemp -d`. No git
# command here ever runs against the maintainer's checkout except read-only
# ones (the origin URL, the source revision, the committer identity), so
# the working tree, the current branch, and jj's view of it are untouched.
#
# NOTE: no reliance on `set -e` (inert under some harnesses); every step
# carries its own guard.

# What is published and where. Constants on purpose; see the header.
readonly BRANCH="readme-assets"
readonly ASSET="readme-hero.png"
readonly MARKER_OPEN="<!-- readme-hero-url -->"
readonly MARKER_CLOSE="<!-- /readme-hero-url -->"
# The website's landing page shows the same image. It is MDX, which rejects
# HTML comments outright, so its markers are MDX comments with the same name.
readonly SITE_PAGE="website/src/content/docs/docs/index.mdx"
readonly SITE_MARKER_OPEN="{/* readme-hero-url */}"
readonly SITE_MARKER_CLOSE="{/* /readme-hero-url */}"
# Below this the capture is not a picture of the UI, whatever `file` says.
readonly MIN_BYTES=20000

script_repo="$(cd "$(dirname "$0")/.." && pwd)" || exit 1

die() {
  echo "publish-readme-hero: $*" >&2
  exit 1
}

# Expected pixel size, from the scenario the capture ran against. Parsed by
# the same json5 package the capture uses, so the two cannot disagree about
# what the file says.
expected_size() {
  local repo="$1"
  # shellcheck disable=SC2016 # the ${} is JavaScript, evaluated by node
  node -e '
    const JSON5 = require(process.argv[1]);
    const scenario = JSON5.parse(require("fs").readFileSync(process.argv[2], "utf8"));
    const v = scenario.viewport;
    process.stdout.write(`${v.width * v.scale}x${v.height * v.scale}`);
  ' "$repo/e2e/node_modules/json5" "$repo/docs/readme-hero/scenario.json5"
}

# Refuse anything that is not a full-size PNG capture. A blank or truncated
# image is the realistic wrong thing to publish and each check here is cheap.
check_png() {
  local repo="$1" png="$2" mime size expected bytes
  test -f "$png" || die "no such file: $png"
  mime="$(file --brief --mime-type "$png")" || die "file(1) failed on $png"
  test "$mime" = "image/png" || die "$png is $mime, not image/png"
  bytes="$(wc -c <"$png")" || die "cannot size $png"
  test "$bytes" -ge "$MIN_BYTES" || die "$png is only $bytes bytes; a real capture is far larger"
  size="$(identify -format '%wx%h' "$png")" || die "identify(1) failed on $png"
  expected="$(expected_size "$repo")" || die "cannot read the scenario's viewport"
  test "$size" = "$expected" || die "$png is ${size}, the scenario declares ${expected}"
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

# Replace the link destination of the one `![...](...)` found between each
# file's two markers. Arguments are MODE URL, then FILE OPEN CLOSE triples.
# Every file is checked before any is written, so either all spans move or
# none does; MODE `check` stops there, which is how publish() refuses a
# broken page before it pushes rather than after. The markers wrap the whole image element rather than
# sitting inside its parens: a link destination is parsed as a literal
# string, not run through inline-HTML handling, so an HTML comment placed
# inside it becomes part of the URL instead of being hidden, and the
# embedded space in "<!-- readme-hero-url -->" then truncates the
# destination at the first space and breaks the whole image markdown (this
# broke the README once already — GitHub fell back to rendering the raw
# `![...](...)` text). Keeping the markers outside the parens lets them do
# their job as ordinary invisible HTML comments while the destination stays
# a plain, unbroken URL.
rewrite_spans() {
  python3 - "$@" <<'EOF' || die "hero URL rewrite failed"
import re
import sys
mode, url, *triples = sys.argv[1:]
if mode not in ("check", "write") or not triples or len(triples) % 3:
    raise SystemExit("usage: rewrite_spans check|write URL (FILE OPEN CLOSE)...")
rewritten = []
for i in range(0, len(triples), 3):
    path, open_marker, close_marker = triples[i : i + 3]
    text = open(path, encoding="utf-8").read()
    if text.count(open_marker) != 1 or text.count(close_marker) != 1:
        raise SystemExit(f"{path}: expected exactly one {open_marker} … {close_marker} pair")
    start = text.index(open_marker) + len(open_marker)
    end = text.index(close_marker)
    if end < start:
        raise SystemExit(f"{path}: the closing marker precedes the opening one")
    span = text[start:end]
    new_span, count = re.subn(r"\]\([^)]*\)", f"]({url})", span, count=1)
    if count != 1:
        raise SystemExit(f"{path}: no single image link destination found between the markers")
    rewritten.append((path, text[:start] + new_span + text[end:]))
if mode == "write":
    for path, text in rewritten:
        with open(path, "w", encoding="utf-8") as out:
            out.write(text)
EOF
}

# Every page that shows the hero, in one place: the README and the website's
# landing page, each with its own marker syntax.
rewrite_hero_urls() {
  local mode="$1" repo="$2" url="$3"
  rewrite_spans "$mode" "$url" \
    "$repo/README.md" "$MARKER_OPEN" "$MARKER_CLOSE" \
    "$repo/$SITE_PAGE" "$SITE_MARKER_OPEN" "$SITE_MARKER_CLOSE"
}

# Build the single-file root commit and, unless dry, push it and rewrite
# both pages. `repo` is the checkout whose origin and pages are used;
# separating it from the script's own location is what lets the self-test
# run the real procedure against a temporary checkout.
publish() {
  local repo="$1" png="$2" dry="$3"
  local remote source name email tmp hash base url
  check_png "$repo" "$png"
  # Both pages' markers must be intact before anything is pushed: a push
  # followed by a failed rewrite would leave the branch pointing at an image
  # that neither page references.
  rewrite_hero_urls check "$repo" "https://example.invalid/$ASSET"
  # The configured value, not `remote get-url`: the latter applies any
  # `insteadOf` rewrite, which is how the self-test redirects pushes to a
  # local bare repository, and the URL written into the pages must be the
  # github one either way. The push itself applies the rewrite at push time.
  remote="$(git -C "$repo" config --get remote.origin.url)" || die "the checkout has no origin remote"
  base="$(raw_base "$remote")" || die "origin is not a github remote: $remote"
  source="$(git -C "$repo" rev-parse HEAD)" || die "cannot read the checkout's HEAD"
  name="$(git -C "$repo" config user.name || true)"
  email="$(git -C "$repo" config user.email || true)"
  test -n "$name" || name="farhelm readme hero"
  test -n "$email" || email="readme-hero@invalid"

  tmp="$(mktemp -d)" || die "mktemp failed"
  # shellcheck disable=SC2064 # expand now: the path must survive any later reassignment
  trap "rm -rf '$tmp'" EXIT
  git -C "$tmp" init -q || die "git init failed"
  cp "$png" "$tmp/$ASSET" || die "cannot copy $png"
  git -C "$tmp" add "$ASSET" || die "git add failed"
  git -C "$tmp" -c "user.name=$name" -c "user.email=$email" commit -q \
    -m "README hero captured from $source" || die "git commit failed"
  hash="$(git -C "$tmp" rev-parse HEAD)" || die "cannot read the new commit"
  url="$base/$hash/$ASSET"

  if [ "$dry" = true ]; then
    echo "dry run: would push $hash to $remote as refs/heads/$BRANCH"
    echo "dry run: would write $url into $repo/README.md and $repo/$SITE_PAGE"
    rm -rf "$tmp"
    return 0
  fi
  # The one destructive line. Fully qualified refspec; nothing else can move.
  git -C "$tmp" push --force -q "$remote" "HEAD:refs/heads/$BRANCH" || die "push to $remote refs/heads/$BRANCH failed"
  rewrite_hero_urls write "$repo" "$url"
  echo "pushed $hash to refs/heads/$BRANCH"
  echo "README.md and $SITE_PAGE now reference $url"
  rm -rf "$tmp"
}

# End-to-end check against a bare repository standing in for github. What it
# proves: a wrong-shaped file is refused before any push, only
# refs/heads/readme-assets exists on the remote after a publish, a second
# publish replaces the branch rather than extending it, the README and
# website spans are both rewritten to exactly the pushed commit's URL, a page
# with broken markers is refused before anything is pushed, a rewrite that
# fails on one file leaves the others untouched, and a dry run touches
# nothing. The checkout's origin is a github-shaped URL so that the URL
# derivation is the real one; a git `insteadOf` rewrite, installed through
# GIT_CONFIG_GLOBAL for the duration of the test only, sends that URL's
# pushes to the bare repository instead. The variable is exported inside
# this function and dies with the process, so nothing outside the self-test
# ever sees it.
self_test() {
  local work bare checkout png expected first second dry fail=0
  local origin="git@github.com:example/repo.git"
  command -v convert >/dev/null || die "self-test needs ImageMagick's convert"
  work="$(mktemp -d)" || die "mktemp failed"
  bare="$work/remote.git"
  checkout="$work/checkout"
  git init -q --bare "$bare" || die "bare init failed"
  printf '[url "%s"]\n\tinsteadOf = %s\n' "$bare" "$origin" >"$work/gitconfig" || die "cannot write the test gitconfig"
  export GIT_CONFIG_GLOBAL="$work/gitconfig"
  # A checkout that looks enough like this repo for publish(): the scenario,
  # the json5 package, a README and a website page with markers, and an
  # origin.
  mkdir -p "$checkout/docs/readme-hero" "$checkout/e2e" "$checkout/$(dirname "$SITE_PAGE")" || die "mkdir failed"
  cp "$script_repo/docs/readme-hero/scenario.json5" "$checkout/docs/readme-hero/" || die "cannot copy the scenario"
  ln -s "$script_repo/e2e/node_modules" "$checkout/e2e/node_modules" || die "cannot link node_modules"
  printf '# t\n\n%s\n![hero](OLD)\n%s\n' "$MARKER_OPEN" "$MARKER_CLOSE" >"$checkout/README.md"
  printf -- '---\ntitle: t\n---\n\n%s\n\n![hero](OLD)\n\n%s\n' "$SITE_MARKER_OPEN" "$SITE_MARKER_CLOSE" >"$checkout/$SITE_PAGE"
  git -C "$checkout" init -q || die "checkout init failed"
  git -C "$checkout" -c user.name=t -c user.email=t@invalid add -A >/dev/null || die "add failed"
  git -C "$checkout" -c user.name=t -c user.email=t@invalid commit -q -m init || die "commit failed"
  git -C "$checkout" remote add origin "$origin" || die "remote add failed"

  expected="$(expected_size "$script_repo")" || die "cannot read the scenario"
  png="$work/hero.png"
  # A flat image compresses below the size floor, so paint noise into it:
  # the floor is meant to catch a blank capture, and a blank test image
  # would trip it for the wrong reason.
  convert -size "$expected" plasma:fractal "$png" || die "cannot synthesise a test PNG"

  test "$(raw_base git@github.com:scode/farhelm.git)" = "https://raw.githubusercontent.com/scode/farhelm" || { echo "FAIL raw_base ssh"; fail=1; }
  test "$(raw_base https://github.com/scode/farhelm)" = "https://raw.githubusercontent.com/scode/farhelm" || { echo "FAIL raw_base https"; fail=1; }
  raw_base "file:///elsewhere" >/dev/null && { echo "FAIL raw_base accepted a non-github remote"; fail=1; }

  # Refusals, before anything is pushed.
  printf 'not a png' >"$work/bad.txt"
  (publish "$checkout" "$work/bad.txt" false >/dev/null 2>&1) && { echo "FAIL published a text file"; fail=1; }
  convert -size 8x8 xc:black "$work/tiny.png" || die "cannot make tiny.png"
  (publish "$checkout" "$work/tiny.png" false >/dev/null 2>&1) && { echo "FAIL published an undersized PNG"; fail=1; }
  # A website page without its markers is refused before the push too.
  mv "$checkout/$SITE_PAGE" "$work/site-page.mdx" || die "cannot move the site page aside"
  printf 'no markers\n' >"$checkout/$SITE_PAGE"
  (publish "$checkout" "$png" false >/dev/null 2>&1) && { echo "FAIL published with a markerless site page"; fail=1; }
  grep -q "OLD" "$checkout/README.md" || { echo "FAIL a refused publish edited the README"; fail=1; }
  mv "$work/site-page.mdx" "$checkout/$SITE_PAGE" || die "cannot restore the site page"
  test "$(git -C "$bare" for-each-ref | wc -l)" -eq 0 || { echo "FAIL a refusal still pushed"; fail=1; }

  # A dry run builds the commit and touches neither the remote nor the README.
  dry="$(publish "$checkout" "$png" true)" || { echo "FAIL dry run"; fail=1; }
  case "$dry" in
    *"https://raw.githubusercontent.com/example/repo/"*"/$ASSET"*) ;;
    *) echo "FAIL dry run URL: $dry"; fail=1 ;;
  esac
  grep -q "OLD" "$checkout/README.md" || { echo "FAIL dry run edited the README"; fail=1; }
  grep -q "OLD" "$checkout/$SITE_PAGE" || { echo "FAIL dry run edited the site page"; fail=1; }
  test "$(git -C "$bare" for-each-ref | wc -l)" -eq 0 || { echo "FAIL dry run pushed"; fail=1; }

  (publish "$checkout" "$png" false >/dev/null) || { echo "FAIL first publish"; fail=1; }
  test "$(git -C "$bare" for-each-ref --format='%(refname)')" = "refs/heads/$BRANCH" || { echo "FAIL remote refs: $(git -C "$bare" for-each-ref)"; fail=1; }
  test "$(git -C "$bare" rev-list --count "$BRANCH")" -eq 1 || { echo "FAIL first publish is not a root commit"; fail=1; }
  test "$(git -C "$bare" ls-tree --name-only "$BRANCH")" = "$ASSET" || { echo "FAIL branch holds more than the asset"; fail=1; }
  first="$(git -C "$bare" rev-parse "$BRANCH")"

  (publish "$checkout" "$png" false >/dev/null) || { echo "FAIL second publish"; fail=1; }
  test "$(git -C "$bare" rev-list --count "$BRANCH")" -eq 1 || { echo "FAIL second publish extended the branch"; fail=1; }
  second="$(git -C "$bare" rev-parse "$BRANCH")"
  test "$first" != "$second" || { echo "FAIL second publish did not replace the commit"; fail=1; }

  # The image destination is exactly the second commit's URL in both pages
  # and nothing else moved, including the alt text and the markers themselves.
  test "$(cat "$checkout/README.md")" = "$(printf '# t\n\n%s\n![hero](https://raw.githubusercontent.com/example/repo/%s/%s)\n%s\n' "$MARKER_OPEN" "$second" "$ASSET" "$MARKER_CLOSE")" || {
    echo "FAIL README rewrite: $(cat "$checkout/README.md")"; fail=1
  }
  test "$(cat "$checkout/$SITE_PAGE")" = "$(printf -- '---\ntitle: t\n---\n\n%s\n\n![hero](https://raw.githubusercontent.com/example/repo/%s/%s)\n\n%s\n' "$SITE_MARKER_OPEN" "$second" "$ASSET" "$SITE_MARKER_CLOSE")" || {
    echo "FAIL site page rewrite: $(cat "$checkout/$SITE_PAGE")"; fail=1
  }
  printf '%s\n%s\n' "$MARKER_OPEN" "$MARKER_OPEN" >"$checkout/dup.md"
  (rewrite_spans write x "$checkout/dup.md" "$MARKER_OPEN" "$MARKER_CLOSE" 2>/dev/null) && { echo "FAIL rewrote a file with duplicate markers"; fail=1; }
  # All or nothing: a good file listed before a broken one stays as it was.
  printf '%s\n![hero](OLD)\n%s\n' "$MARKER_OPEN" "$MARKER_CLOSE" >"$checkout/good.md"
  (rewrite_spans write x "$checkout/good.md" "$MARKER_OPEN" "$MARKER_CLOSE" "$checkout/dup.md" "$MARKER_OPEN" "$MARKER_CLOSE" 2>/dev/null) &&
    { echo "FAIL a partial rewrite reported success"; fail=1; }
  grep -q "OLD" "$checkout/good.md" || { echo "FAIL a failed rewrite still edited an earlier file"; fail=1; }

  unset GIT_CONFIG_GLOBAL
  rm -rf "$work"
  if [ "$fail" -ne 0 ]; then
    echo "publish-readme-hero self-test: FAILED" >&2
    return 1
  fi
  echo "publish-readme-hero self-test: ok"
}

dry=false
png=""
while [ $# -gt 0 ]; do
  case "$1" in
    --dry-run) dry=true ;;
    --self-test) self_test; exit $? ;;
    -h | --help)
      sed -n '2,30p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    -*) die "unknown option: $1" ;;
    *)
      test -z "$png" || die "one PNG at a time"
      png="$1"
      ;;
  esac
  shift
done
test -n "$png" || die "usage: $0 [--dry-run] PNG | --self-test"
case "$png" in
  /*) ;;
  *) png="$PWD/$png" ;;
esac
publish "$script_repo" "$png" "$dry"
