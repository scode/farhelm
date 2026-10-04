#!/bin/sh
{
  # EVERYTHING in this file — every comment, every definition, and the final
  # call to main — is inside this ONE `{ ... }` compound command, opened on
  # the line right after the shebang and closed only on the file's physical
  # last line. That, not merely "main() is called last", is what makes a
  # truncated `curl | sh` transfer fail closed: a shell reading a byte
  # stream executes each syntactically COMPLETE command as it arrives, and a
  # brace group is not complete until its closing `}` token has been read —
  # so if the transfer is cut off ANYWHERE before that token, even one byte
  # before it, even while still inside a comment, the parser hits EOF still
  # inside an unterminated compound command, reports a syntax error, and the
  # shell exits having executed NOTHING. Opening the brace before even this
  # documentation, rather than after it, is deliberate: a `{ ... }` that
  # opened only later would leave every truncation point inside these
  # comments unprotected — executing zero commands either way (harmless),
  # but reporting success (misleading) instead of the clear failure every
  # other truncation point gets. A bare `main "$@"` on the last line WITHOUT
  # this wrapper does not have the fail-closed property at all: a stream cut
  # off right after the literal bytes `main` is itself already a complete,
  # executable command, and the shell runs it immediately.
  #
  # The macOS desktop installer: checks the platform,
  # downloads the matching release from GitHub, verifies it, and puts the
  # binaries in place. The README's "Install" chapter is the same text as
  # this script's behavior and is the place to look for the user-facing
  # story.
  #
  # POSIX sh on purpose, not bash: this is the file `curl | sh` runs on
  # whatever `/bin/sh` a fresh machine happens to have, so it can lean on
  # nothing bash-specific (arrays, `[[`, `local`, process substitution). It
  # is meant to be read before it is run — README says so — so it stays one
  # file, no sourcing, no helper scripts fetched separately.
  set -eu

  REPO_URL="https://github.com/scode/farhelm"
  RELEASES_PAGE="$REPO_URL/releases"
  LATEST_URL="$RELEASES_PAGE/latest"
  DOWNLOAD_PREFIX="$REPO_URL/releases/download"
  CURL_PROTOCOL_MODE=default

  # ---------------------------------------------------------------------
  # What an install changes, in one place
  #
  # The app bundle ~/Applications/Farhelm.app is the whole installation,
  # and ~/.local/bin/farhelm only a symlink into it (SPEC_impl.md,
  # "Side-by-side versions inside Farhelm.app"). A fresh install builds the
  # bundle in a private directory and renames it into place. An update
  # changes it in place, possibly while Farhelm runs from it, by renaming
  # one complete file or version folder at a time in an order where
  # stopping after any step leaves an app that launches the old version or
  # the new one; see step 7 of main. Nothing therefore needs a recovery
  # journal: every intermediate state is a working installation, and
  # running the installer again finishes the job.
  # ---------------------------------------------------------------------

  # TARGET|ARCHIVE|BINARY, one row per published archive. This is the single
  # copy of that fact in this file — every lookup below reads it rather than
  # hardcoding a name a second time — and it is also what `assets.rs`'s test
  # module parses back out of this file's source and diffs against
  # `RELEASE_ARCHIVES`, so the row set and the marker lines are load-bearing
  # for something other than this script. The two marker comments are
  # deliberately flush left (not indented like the rest of this block): the
  # Rust-side parser locates the block by finding these two literal strings
  # and reads whatever is textually between them, so indentation on the
  # marker LINES themselves would leak into that extracted text as a
  # trailing whitespace-only line and break the parity test.
# BEGIN ASSET TABLE
  ASSET_TABLE='
x86_64-unknown-linux-musl|farhelm-x86_64-unknown-linux-musl.tar.gz|farhelm
aarch64-unknown-linux-musl|farhelm-aarch64-unknown-linux-musl.tar.gz|farhelm
aarch64-apple-darwin|farhelm-aarch64-apple-darwin.tar.gz|farhelm
aarch64-apple-darwin|farhelm-desktop-aarch64-apple-darwin.tar.gz|farhelm-desktop
'
# END ASSET TABLE

  # D15's version shape: X.Y.Z or vX.Y.Z, with an optional -rc.N or -dev.N
  # prerelease suffix (the two kinds of prerelease this project cuts — see
  # releasing/AGENTS.md; a -dev.N is an rc under another name, so
  # the installer treats the two identically); no leading zeros anywhere.
  # Any other suffix stays rejected on purpose: a pinned version that does
  # not match a tag this project would ever publish is a typo, not a
  # request to try harder. See normalize_version below for why a plain
  # grep -E against this is not, by itself, enough.
  VERSION_PATTERN='^v?(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-(rc|dev)\.(0|[1-9][0-9]*))?$'

  # A literal newline and a literal carriage return, for the checks below
  # that need to test whether a value CONTAINS one. Built with printf rather
  # than embedded directly in the source: command substitution strips
  # TRAILING newlines, so the character has to come before a throwaway
  # sentinel (here "X") that gets stripped instead, or it would vanish along
  # with it.
  NEWLINE=$(printf '\nX')
  NEWLINE=${NEWLINE%X}
  CR=$(printf '\rX')
  CR=${CR%X}

  # The exact bytes of a minimal, valid gzip-compressed tar archive (one
  # regular file, "a/f", containing a single byte) — generated once, offline,
  # and pasted here as a portable printf octal-escape sequence (works in any
  # POSIX sh; needs no base64 or other extra tool to decode). Used only to
  # PROVE this machine's `tar` can actually read a gzip stream before this
  # script trusts it to unpack a real release archive later (see the
  # prerequisite check in main).
  # shellcheck disable=SC2016 # the literal backslash escapes are the payload; nothing here is meant to expand
  GZIP_TAR_PROBE='\037\213\010\000\000\000\000\000\000\003\355\316\261\015\203\060\024\004\120\217\342\015\142\154\364\231\207\046\003\100\220\030\077\026\145\112\044\047\051\336\153\116\272\346\156\175\074\323\150\245\213\230\257\354\076\263\233\322\324\242\324\045\312\174\365\321\152\113\271\014\177\326\035\373\153\335\162\376\306\324\077\072\177\175\000\000\000\000\000\000\000\000\000\200\133\336\272\161\326\206\000\050\000\000'

  # Validates $1 against $VERSION_PATTERN and echoes the normalized "vX.Y.Z"
  # (optionally "-rc.N" or "-dev.N") tag, or returns failure and prints
  # nothing.
  #
  # Rejects embedded newlines/carriage returns FIRST: grep -E's ^/$ anchors
  # match at the start/end of each LINE, not of the whole value, so a value
  # like "1.2.3\njunk" would otherwise pass validation on its first line and
  # carry the rest into a release URL completely unchecked.
  normalize_version() {
    case "$1" in
      *"$NEWLINE"* | *"$CR"*) return 1 ;;
    esac
    printf '%s' "$1" | grep -Eq "$VERSION_PATTERN" || return 1
    case "$1" in
      v*) printf '%s\n' "$1" ;;
      *) printf 'v%s\n' "$1" ;;
    esac
  }

  # Validate the optional mirror before any work that could make a failed
  # configuration look like a network or release failure. This deliberately
  # mirrors the helm's release-base parser: credentials, queries, and
  # fragments are refused because they are either unsafe to expose or do not
  # survive the asset URL joining contract faithfully.
  validate_release_base_url() {
    vrbu_value=$1
    case "$vrbu_value" in
      http://* | https://*) ;;
      *) return 1 ;;
    esac
    case "$vrbu_value" in
      *[\?\#]* | *"$NEWLINE"* | *"$CR"*) return 1 ;;
    esac
    vrbu_authority=${vrbu_value#*://}
    vrbu_host=${vrbu_authority%%/*}
    [ -n "$vrbu_host" ] || return 1
    case "$vrbu_host" in
      *@* | :*) return 1 ;;
    esac
    # Plain HTTP only reaches this machine. The fixture server the test
    # suites run is loopback HTTP; anything else must be HTTPS so the
    # download cannot be read or rewritten on the way.
    case "$vrbu_value" in
      http://*)
        case "$vrbu_host" in
          127.0.0.1 | 127.0.0.1:* | localhost | localhost:* | '[::1]' | '[::1]:'*) ;;
          *) return 1 ;;
        esac
        ;;
    esac
  }

  # Decide styling per output stream: redirecting a report must not put
  # terminal controls in it even when progress still goes to a terminal.
  # NO_COLOR suppresses color and emphasis. OSC 8 links are emitted whenever
  # stdout is a terminal, independently of NO_COLOR (the approved P2 behavior).
  init_output() {
    OUT_GREEN='' OUT_YELLOW='' OUT_CYAN='' OUT_DIM='' OUT_RESET=''
    ERR_BOLD='' ERR_RED='' ERR_CYAN='' ERR_DIM='' ERR_RESET=''
    ERR_TERMINAL=0
    BREW_LINK=https://brew.sh/
    if [ -t 1 ]; then
      BREW_LINK=$(printf '\033]8;;https://brew.sh/\033\134https://brew.sh/\033]8;;\033\134')
      if [ -z "${NO_COLOR:-}" ]; then
        OUT_GREEN=$(printf '\033[1;32m')
        OUT_YELLOW=$(printf '\033[1;33m')
        OUT_CYAN=$(printf '\033[36m')
        OUT_DIM=$(printf '\033[2m')
        OUT_RESET=$(printf '\033[0m')
      fi
    fi
    if [ -t 2 ]; then
      ERR_TERMINAL=1
      if [ -z "${NO_COLOR:-}" ]; then
        ERR_BOLD=$(printf '\033[1m')
        ERR_RED=$(printf '\033[1;31m')
        ERR_CYAN=$(printf '\033[36m')
        ERR_DIM=$(printf '\033[2m')
        ERR_RESET=$(printf '\033[0m')
      fi
    fi
  }

  # Fatal diagnostics go to stderr with an error marker and, on a styled
  # terminal, red text. Preserve their wording even when stdout is redirected.
  error() {
    printf '❌ %s' "$ERR_RED" >&2
    # shellcheck disable=SC2059 # Callers supply a literal printf format and its arguments.
    printf "$@" >&2
    printf '%s' "$ERR_RESET" >&2
  }

  # Every network request in this script goes through here. `-q` disables
  # the user's ~/.curlrc — it only takes effect as curl's very first
  # argument, which is why it is hardcoded ahead of anything the caller
  # passes, since a stray .curlrc could otherwise silently change redirect
  # or output behavior. The timeouts bound how long an unreachable or
  # stalled release host can delay failure: a little over 10 minutes per
  # request, from --max-time 600.
  #
  # The `loopback` mode serves only FARHELM_INSTALL_TEST_BASE_URL's plain-HTTP
  # fixture. HTTP must stay allowed for redirects there (the fixture's own
  # 302 test serves the next hop over HTTP), so instead of a protocol pin it
  # uses --connect-to to send EVERY connection, redirects included, to the
  # validated loopback host. A redirect to some other host therefore still
  # only ever talks to this machine; plain HTTP never leaves it.
  curl_get() {
    if [ "$CURL_PROTOCOL_MODE" = default ]; then
      curl -q --connect-timeout 15 --max-time 600 \
        --proto '=https' --proto-redir '=https' "$@"
    else
      curl -q --connect-timeout 15 --max-time 600 \
        --proto '=http,https' --proto-redir '=http,https' \
        --connect-to "::$CURL_LOOPBACK_HOST:" "$@"
    fi
  }

  # Computes the SHA-256 of $1 as a lowercase hex string, using whichever
  # tool prerequisite detection selected once, into $CHECKSUM_TOOL (set in
  # main). Checked for ITS OWN exit status rather than trusting whatever a
  # downstream `awk` happens to print from a truncated pipe: a failing
  # checksum tool must be reported as a checksum-tool failure, not smoothed
  # into a false "mismatch" diagnostic.
  #
  # Reads the file as STDIN rather than passing it as a filename argument:
  # GNU sha256sum escapes a filename containing a backslash and prefixes
  # that line with `\`, which would otherwise land in $1 of the awk call
  # below and corrupt the parsed digest for any install directory whose
  # path contains one. Stdin mode never prints a filename at all, so there
  # is nothing to escape.
  sha256_of() {
    case "$CHECKSUM_TOOL" in
      sha256sum)
        out=$(sha256sum <"$1") || return 1
        printf '%s\n' "$out" | awk '{print $1}'
        ;;
      shasum)
        out=$(shasum -a 256 <"$1") || return 1
        printf '%s\n' "$out" | awk '{print $1}'
        ;;
      openssl)
        out=$(openssl dgst -sha256 <"$1") || return 1
        printf '%s\n' "$out" | awk '{print $NF}'
        ;;
    esac
  }

  # Writes the forwarder, Contents/MacOS/farhelm, to $1. Everything that
  # outlives a Farhelm session's supervisor names this file (hook command
  # lines, the reporters' executable variables, the session PATH entry, and
  # ~/.local/bin/farhelm), so its text is a contract with every session a
  # Farhelm ever started: a later installer may replace it only by rename,
  # and only with a script that forwards every invocation this one accepts
  # the same way (SPEC_impl.md, "What running sessions hold across
  # versions"). It is a shell script because scripts need no code signature
  # on Apple silicon, so it needs no release packaging.
  #
  # Inside a session ($FARHELM_SUPERVISOR_SOCK set, with a running-version
  # record beside that socket) it runs the version the session's supervisor
  # runs; anywhere else, the Installed one. It execs, so the chosen program
  # takes its place in the process tree (hook attribution walks that tree
  # and treats any extra process as an intermediary), and it passes
  # arguments, environment and standard streams through untouched. Its own
  # variables carry a prefix nothing exports, because assigning to a name
  # that came in exported would change the child's environment.
  write_forwarder() {
    cat >"$1" <<'FORWARDER_EOF'
#!/bin/sh
# Farhelm's forwarder: runs one of the farhelm versions kept side by side in
# this app bundle (SPEC_impl.md, "Side-by-side versions inside Farhelm.app").
# Inside a Farhelm session it runs the version that session's supervisor runs,
# named by running-version beside $FARHELM_SUPERVISOR_SOCK; anywhere else it
# runs the installed one, named by Contents/Versions/installed. Arguments,
# the caller's environment variables and standard streams pass through as
# they are (the shell itself may add PWD, SHLVL and _). Written by Farhelm's
# installer; replaced only by a newer one that forwards everything this one
# does the same way.
farhelm_forwarder_self=$0
case $farhelm_forwarder_self in
  */*) ;;
  *) farhelm_forwarder_self=./$farhelm_forwarder_self ;;
esac
while [ -h "$farhelm_forwarder_self" ]; do
  farhelm_forwarder_link=$(readlink "$farhelm_forwarder_self") || break
  case $farhelm_forwarder_link in
    /*) farhelm_forwarder_self=$farhelm_forwarder_link ;;
    *) farhelm_forwarder_self=${farhelm_forwarder_self%/*}/$farhelm_forwarder_link ;;
  esac
done
farhelm_forwarder_versions=$(CDPATH='' cd -P -- "${farhelm_forwarder_self%/*}/../Versions" 2>/dev/null && pwd -P) || {
  printf 'farhelm: cannot find the Versions folder beside %s\n' "$farhelm_forwarder_self" >&2
  exit 127
}
farhelm_forwarder_version=
if [ -n "${FARHELM_SUPERVISOR_SOCK:-}" ] && [ -f "${FARHELM_SUPERVISOR_SOCK%/*}/running-version" ]; then
  IFS= read -r farhelm_forwarder_version <"${FARHELM_SUPERVISOR_SOCK%/*}/running-version" || :
fi
if [ -z "$farhelm_forwarder_version" ] && [ -f "$farhelm_forwarder_versions/installed" ]; then
  IFS= read -r farhelm_forwarder_version <"$farhelm_forwarder_versions/installed" || :
fi
case $farhelm_forwarder_version in
  '' | .* | */*)
    printf 'farhelm: no installed version is recorded in %s; reinstall Farhelm\n' "$farhelm_forwarder_versions" >&2
    exit 127
    ;;
esac
if [ ! -x "$farhelm_forwarder_versions/$farhelm_forwarder_version/farhelm" ]; then
  printf 'farhelm: version %s is not installed in %s; reinstall Farhelm\n' "$farhelm_forwarder_version" "$farhelm_forwarder_versions" >&2
  exit 127
fi
exec "$farhelm_forwarder_versions/$farhelm_forwarder_version/farhelm" "$@"
FORWARDER_EOF
  }

  # Writes the app's Info.plist to $1, naming $VERSION_NUM.
  #
  # Info.plist is launch configuration: its LSEnvironment key can set
  # DYLD_INSERT_LIBRARIES or FARHELM_* for the app, so whoever can write
  # it can run code as the user the next time Farhelm is opened. The
  # callers give it an explicit owner-only-writable mode for that reason.
  write_info_plist() {
    cat >"$1" <<PLIST_EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleExecutable</key>
	<string>farhelm-desktop</string>
	<key>CFBundleIdentifier</key>
	<string>org.scode.farhelm.desktop</string>
	<key>CFBundleName</key>
	<string>Farhelm</string>
	<key>CFBundleDisplayName</key>
	<string>Farhelm</string>
	<key>CFBundleIconFile</key>
	<string>Farhelm</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleInfoDictionaryVersion</key>
	<string>6.0</string>
	<key>CFBundleShortVersionString</key>
	<string>$VERSION_NUM</string>
	<key>CFBundleVersion</key>
	<string>$VERSION_NUM</string>
	<key>LSMinimumSystemVersion</key>
	<string>11.0</string>
	<key>NSHighResolutionCapable</key>
	<true/>
	<key>LSApplicationCategoryType</key>
	<string>public.app-category.developer-tools</string>
</dict>
</plist>
PLIST_EOF
  }

  # Writes this installation's ownership record to $1: the identifier
  # `farhelm-app-v2` and the absolute path of the Terminal link it owns ($2),
  # each followed by a NUL. It records no checksums, because every other
  # file in the bundle changes on every update; uninstall verifies the
  # bundle's layout and file ownership instead.
  write_current_record() {
    (umask 077; printf 'farhelm-app-v2\000%s\000' "$2" >"$1") || return 1
    chmod 0600 "$1"
  }

  # True iff $1 is a regular, non-symlink file holding exactly the record
  # write_current_record would write for link path $2. POSIX sh cannot hold
  # NUL in a variable, so the expected bytes are written to a file and the
  # two are compared.
  current_record_is_ours() {
    if [ -L "$1" ] || [ ! -f "$1" ]; then
      return 1
    fi
    write_current_record "$STAGING_DIR/current-record-expected" "$2" || return 1
    same_bytes "$STAGING_DIR/current-record-expected" "$1"
  }

  # True iff the regular files $1 and $2 hold the same bytes, compared by
  # checksum with the tool chosen in main rather than with cmp, which the
  # installer does not otherwise need.
  same_bytes() {
    [ -f "$1" ] && [ -f "$2" ] || return 1
    sb_first=$(sha256_of "$1") || return 1
    sb_second=$(sha256_of "$2") || return 1
    [ "$sb_first" = "$sb_second" ]
  }

  # True iff $1 is a well-formed record of this layout that names some other
  # Terminal link: an absolute path to a farhelm (~/.local/bin was a
  # symlink when it was written and is a real folder now, the home folder
  # was renamed, and so on). The app's location is fixed and the installer
  # only ever records this home's link, so such a record can only be this
  # installation's; the update rewrites it.
  current_record_moved_here() {
    if [ -L "$1" ] || [ ! -f "$1" ]; then
      return 1
    fi
    crm_magic=$(tr '\000' '\n' <"$1" | sed -n 1p) || return 1
    crm_link=$(tr '\000' '\n' <"$1" | sed -n 2p) || return 1
    [ "$crm_magic" = farhelm-app-v2 ] || return 1
    case "$crm_link" in
      /*/farhelm) return 0 ;;
      *) return 1 ;;
    esac
  }


  # True iff $1 is a version folder name exactly as this installer writes
  # one: a normalized release version (X.Y.Z, optionally -rc.N or -dev.N,
  # no leading "v") or 0.0.0-unreleased. Only such folders are ever pruned.
  # Names holding a newline or carriage return are refused first, because
  # grep matches line by line and would otherwise accept a name with one
  # valid line in it (the same hazard normalize_version guards against).
  is_version_folder_name() {
    case "$1" in
      *"$NEWLINE"* | *"$CR"*) return 1 ;;
      0.0.0-unreleased) return 0 ;;
    esac
    printf '%s\n' "$1" | grep -Eq '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-(rc|dev)\.(0|[1-9][0-9]*))?$'
  }

  # Stages the new version's folder, holding a complete copy of the staged
  # farhelm, at $UPDATE_WORK/version, ready to be renamed into Versions/.
  stage_version_dir() {
    (umask 022; mkdir "$UPDATE_WORK/version") || bundle_fail "creating the $VERSION_NUM folder"
    cp "$STAGING_DIR/farhelm" "$UPDATE_WORK/version/farhelm" || bundle_fail "copying farhelm"
    chmod 0755 "$UPDATE_WORK/version/farhelm" || bundle_fail "setting the farhelm mode"
  }

  # Replaces the file $2 inside the bundle with a copy of $1, mode $3, by
  # renaming a complete copy over it. The copy is made beside $2, on the
  # bundle's own filesystem, so the rename is atomic: a running Farhelm and
  # anything about to start it see the old file or the new one, never a
  # partial one, and a program file's code signature is never rewritten in
  # place. $4 names the file in a failure.
  replace_file() {
    REPLACE_TMP="${2%/*}/.farhelm-new.$$.${2##*/}"
    rm -f "$REPLACE_TMP"
    cp "$1" "$REPLACE_TMP" || bundle_fail "copying $4"
    chmod "$3" "$REPLACE_TMP" || bundle_fail "setting the mode of $4"
    mv -f "$REPLACE_TMP" "$2" || bundle_fail "replacing $4"
    REPLACE_TMP=""
  }

  # How many members of the archive at $1 have $2 as their BASENAME. The
  # basename match, not a full path comparison, is the deliberate contract
  # with dist's layout: dist nests each member under <package>-<target>/, and
  # locating by basename means this script never has to hardcode that prefix.
  member_count_named() {
    tar tzf "$1" | awk -F/ -v b="$2" '{n=split($0,p,"/"); if (p[n]==b) c++} END{print c+0}'
  }

  # Stream the ONE member of archive $1 whose basename is $3 to the path $4
  # (a name THIS SCRIPT chose — no path from inside an archive ever touches
  # the filesystem), with final mode $5 regardless of what the archive or the
  # caller's umask would produce. $2 is the archive's release name, used only
  # in messages. Refuses, with exit 1, an archive where the basename matches
  # zero or several members — callers that treat a member as OPTIONAL must
  # probe with member_count_named first and only call this on a hit.
  #
  # The refusals between selection and extraction are the point of this
  # function existing at all, and they run in a fixed order:
  #
  # - a member whose full in-archive path starts with "-" could be misread as
  #   a tar OPTION rather than an operand — some tar implementations keep
  #   parsing options after the archive operand — so a crafted path could
  #   redirect what gets listed or extracted;
  # - control characters are refused for the same "do not trust this string
  #   with a shell/tool boundary" reason;
  # - the basename match only proves ONE entry has this basename SOMEWHERE in
  #   the archive, not that IT is a regular file: a regular decoy entry named
  #   "<member>.extra" sorting earlier in a naive substring search could
  #   supply a false "-" type character for a symlink or hardlink actually AT
  #   the selected path. So metadata is queried for the EXACT member only
  #   (`--` first, so the name can never be misread as another option), must
  #   yield exactly one record, and that record must be a regular file.
  #
  # This is the same shape the helm's own extractor and the release's
  # sign-sums job require of these archives.
  extract_sole_member() {
    esm_archive=$1
    esm_label=$2
    esm_base=$3
    esm_dest=$4
    esm_mode=$5

    esm_hits=$(member_count_named "$esm_archive" "$esm_base")
    if [ "$esm_hits" -ne 1 ]; then
      error '%s has %s members named %s, expected exactly 1\n' "$esm_label" "$esm_hits" "$esm_base"
      exit 1
    fi
    esm_member=$(tar tzf "$esm_archive" | awk -F/ -v b="$esm_base" '{n=split($0,p,"/"); if (p[n]==b) print}')

    case "$esm_member" in
      -*)
        error '%s: member name %s looks like a tar option; refusing\n' "$esm_label" "$esm_member"
        exit 1
        ;;
    esac
    if printf '%s' "$esm_member" | LC_ALL=C grep -q '[[:cntrl:]]'; then
      error '%s: member name contains a control character; refusing\n' "$esm_label"
      exit 1
    fi

    # Each substitution below has its own failure path. The script runs
    # under `set -e`, so an assignment whose command fails would otherwise
    # end the run right here, silently (tar's own error is discarded), before
    # any of this function's named refusals could print. `grep -c` exits 1
    # when it counts zero lines, so `|| true` keeps its "0" for the check
    # below instead of ending the run.
    esm_type_lines=$(tar tvzf "$esm_archive" -- "$esm_member" 2>/dev/null) || {
      error '%s: tar could not list %s in the archive; refusing\n' "$esm_label" "$esm_member"
      exit 1
    }
    esm_type_count=$(printf '%s\n' "$esm_type_lines" | grep -c . || true)
    if [ "$esm_type_count" -ne 1 ]; then
      error '%s: %s reports %s metadata records for %s, expected exactly 1\n' "$esm_label" "tar tv" "$esm_type_count" "$esm_member"
      exit 1
    fi
    esm_type_char=$(printf '%s' "$esm_type_lines" | cut -c1)
    if [ "$esm_type_char" != "-" ]; then
      error '%s: %s is not a regular file (tar reports type '"'"'%s'"'"'); refusing to install it\n' "$esm_label" "$esm_member" "$esm_type_char"
      exit 1
    fi

    tar -xOzf "$esm_archive" -- "$esm_member" >"$esm_dest"
    chmod "$esm_mode" "$esm_dest"
  }

  # Prints the SHA-256 the layout before this one recorded for the copy of
  # binary $1 ("farhelm" or "farhelm-desktop") it put in the bin directory,
  # or nothing when there is no usable record or it names no digest for that
  # binary. That record is NUL-separated (magic, canonical directory, CLI
  # digest, desktop digest, then a final NUL). The digests are read from the
  # END: the directory field may legally contain newlines, so counting
  # forward after `tr` would misalign on such a path, while the two digests
  # never do. A symlinked or non-regular record is treated as absent.
  recorded_digest_of() {
    rdo_record="$INSTALL_DIR/.farhelm-installation"
    if [ -L "$rdo_record" ] || [ ! -f "$rdo_record" ]; then
      return 0
    fi
    rdo_magic=$(tr '\000' '\n' <"$rdo_record" | head -n 1) || return 0
    [ "$rdo_magic" = farhelm-standalone ] || return 0
    # The last line is the desktop digest (possibly empty), the one before
    # it the CLI digest.
    case "$1" in
      farhelm) rdo_back=1 ;;
      farhelm-desktop) rdo_back=0 ;;
      *) return 0 ;;
    esac
    tr '\000' '\n' <"$rdo_record" | awk -v back="$rdo_back" '{ line[NR] = $0 } END { if (NR > back) print line[NR - back] }'
    return 0
  }

  # True iff the app bundle at $1 carries the bundle record the layout
  # before this one wrote for this installation: a regular, non-symlink
  # Contents/.farhelm-installation that starts with the `farhelm-app`
  # identifier followed by this bin directory's canonical path ($2). It is
  # the ownership test for the one-time move from that layout, which
  # replaces the bundle wholesale; the record's checksums are not verified,
  # because a bundle an interrupted uninstall has half-emptied must still be
  # rebuilt, not refused.
  #
  # The comparison is byte-exact on the record's leading fields. POSIX sh
  # cannot hold NUL in a variable, so the expected prefix is written to a
  # file and both prefixes are compared by checksum; `${#}` under LC_ALL=C
  # gives the canonical path's length in bytes.
  bundle_record_is_ours() {
    brio_record="$1/Contents/.farhelm-installation"
    if [ -L "$brio_record" ] || [ ! -f "$brio_record" ]; then
      return 1
    fi
    brio_len=$(LC_ALL=C; export LC_ALL; printf '%s' "$((13 + ${#2}))") || return 1
    printf 'farhelm-app\000%s\000' "$2" >"$STAGING_DIR/bundle-record-expected" || return 1
    head -c "$brio_len" <"$brio_record" >"$STAGING_DIR/bundle-record-actual" || return 1
    brio_expected=$(sha256_of "$STAGING_DIR/bundle-record-expected") || return 1
    brio_actual=$(sha256_of "$STAGING_DIR/bundle-record-actual") || return 1
    [ "$brio_expected" = "$brio_actual" ]
  }

  # True iff every entry of directory $1 is one of the names that follow,
  # and each of those names is present. Hidden names count; a shell glob
  # rather than `ls`: inherited QUOTING_STYLE and locale settings must not
  # change what counts as present.
  dir_has_exactly() {
    dhe_dir=$1
    shift
    for dhe_entry in "$dhe_dir"/* "$dhe_dir"/.[!.]* "$dhe_dir"/..?*; do
      [ -e "$dhe_entry" ] || [ -L "$dhe_entry" ] || continue
      dhe_known=0
      for dhe_name in "$@"; do
        [ "${dhe_entry##*/}" = "$dhe_name" ] && dhe_known=1
      done
      [ "$dhe_known" -eq 1 ] || return 1
    done
    for dhe_name in "$@"; do
      [ -e "$dhe_dir/$dhe_name" ] || return 1
    done
    return 0
  }

  # True iff $1 is exactly what this script built between the bundle's
  # introduction (#310) and its ownership record (#673): the fixed layout
  # with no record, regular non-symlink files throughout, and an Info.plist
  # naming the bundle identifier this script has always written. Those
  # bundles have nothing else to prove ownership with, and leaving one in
  # place would leave a stale second Farhelm in Launchpad and Spotlight.
  is_legacy_installer_bundle() {
    ilib_app=$1
    dir_has_exactly "$ilib_app" Contents || return 1
    dir_has_exactly "$ilib_app/Contents" Info.plist MacOS Resources || return 1
    dir_has_exactly "$ilib_app/Contents/MacOS" farhelm farhelm-desktop || return 1
    dir_has_exactly "$ilib_app/Contents/Resources" Farhelm.icns || return 1
    for ilib_file in Contents/Info.plist Contents/MacOS/farhelm Contents/MacOS/farhelm-desktop Contents/Resources/Farhelm.icns; do
      if [ -L "$ilib_app/$ilib_file" ] || [ ! -f "$ilib_app/$ilib_file" ]; then
        return 1
      fi
    done
    for ilib_dir in "" /Contents /Contents/MacOS /Contents/Resources; do
      [ -L "$ilib_app$ilib_dir" ] && return 1
    done
    # The identifier test must see the identifier macOS will actually use,
    # not a copy of this script's pair left inside an XML comment by someone
    # who customised the bundle. So comments are stripped first, the plist
    # must mention CFBundleIdentifier exactly once after that, and that one
    # occurrence must be this script's own key line followed by its own
    # string line. Anything else (a second key, a one-line pair, a parse
    # this cannot follow) is not provably the installer's and is refused.
    awk '
      {
        line = $0; out = ""
        while (line != "") {
          if (in_comment) {
            end = index(line, "-->")
            if (end == 0) { line = ""; continue }
            line = substr(line, end + 3); in_comment = 0
          } else {
            start = index(line, "<!--")
            if (start == 0) { out = out line; line = ""; continue }
            out = out substr(line, 1, start - 1); line = substr(line, start + 4); in_comment = 1
          }
        }
        rest = out
        while ((at = index(rest, "CFBundleIdentifier")) > 0) { mentions++; rest = substr(rest, at + 18) }
        if (found) {
          if (out ~ /^[[:space:]]*<string>org\.scode\.farhelm\.desktop<\/string>[[:space:]]*$/) ok = 1
          found = 0
        }
        if (out ~ /^[[:space:]]*<key>CFBundleIdentifier<\/key>[[:space:]]*$/) found = 1
      }
      END { exit (ok && mentions == 1 && !in_comment) ? 0 : 1 }
    ' "$ilib_app/Contents/Info.plist"
  }

  # Undo whatever the bundle step (step 7 of main) left half done, then drop
  # its lock. A private build directory still holding the previous bundle
  # means the swap did not finish: the previous bundle goes back if the
  # public name is free, and the directory is deleted only once it no
  # longer holds it, so a failed restore never deletes the user's app. An
  # in-place update leaves only its private work directory and at most one
  # half-copied file beside its destination, both its own. Safe to call
  # when the bundle step never ran, and more than once.
  release_bundle_lock() {
    if [ -n "${BUNDLE_WORK:-}" ] && [ -d "$BUNDLE_WORK" ]; then
      if [ -e "$BUNDLE_WORK/previous" ] && [ ! -e "$app_path" ]; then
        mv "$BUNDLE_WORK/previous" "$app_path" 2>/dev/null || true
      fi
      if [ -e "$BUNDLE_WORK/previous" ]; then
        error 'the previous %s could not be put back; it is at %s\n' "$app_path" "$BUNDLE_WORK/previous"
      else
        rm -rf "$BUNDLE_WORK" 2>/dev/null || true
      fi
    fi
    BUNDLE_WORK=""
    if [ -n "${REPLACE_TMP:-}" ]; then
      rm -f "$REPLACE_TMP" 2>/dev/null || true
      REPLACE_TMP=""
    fi
    if [ -n "${UPDATE_WORK:-}" ]; then
      rm -rf "$UPDATE_WORK" 2>/dev/null || true
      UPDATE_WORK=""
    fi
    if [ "${BUNDLE_LOCK_HELD:-0}" -eq 1 ]; then
      BUNDLE_LOCK_HELD=0
      rmdir "$BUNDLE_LOCK" 2>/dev/null || true
    fi
  }

  # The EXIT/INT/TERM/HUP handler: removes the ephemeral staging directory
  # and undoes the bundle step's own leftovers (see release_bundle_lock).
  #
  # What this does NOT do: survive SIGKILL or a power loss. Both skip trap
  # handlers entirely, so a `.farhelm-install.*` staging directory, a
  # private build or work directory beside the bundle, or the bundle lock
  # can be left behind. The staging and work directories are inert and safe
  # to delete by hand; a leftover lock makes the next run refuse and say to
  # remove it. The installation itself is a working one at every point an
  # update can stop (see the header).
  cleanup() {
    rm -rf "$STAGING_DIR" 2>/dev/null || true
    release_bundle_lock
  }

  main() {
    init_output

    # 1. Environment and platform. The fixed install directory comes from HOME.
    if [ -z "${HOME:-}" ]; then
      error 'HOME is not set; refusing to install\n'
      exit 1
    fi
    case "$HOME" in
      /*) ;;
      *) error 'HOME must be an absolute path; refusing to install\n'; exit 1 ;;
    esac
    # Only the installer is macOS-only. The helm provisions Linux hosts
    # itself; refusing here must happen before downloads or filesystem writes.
    os=$(uname -s)
    if [ "$os" != Darwin ]; then
      error 'This installer only supports macOS for now.\n'
      printf '%s\n' \
        '   Linux is supported for running a helm and session hosts; only this installer is' \
        '   limited, and that will be fixed. If you want to install on Linux, please open an' \
        "   issue and it will be prioritized: ${ERR_CYAN}https://github.com/scode/farhelm/issues${ERR_RESET}" >&2
      exit 1
    fi
    # Rosetta reports x86_64 on Apple silicon, which can run the native build.
    machine=$(uname -m)
    case "$machine" in
      arm64) TARGET=aarch64-apple-darwin ;;
      x86_64)
        if [ "$(sysctl -n hw.optional.arm64 2>/dev/null || true)" = "1" ]; then
          TARGET=aarch64-apple-darwin
        else
          error 'farhelm has no release build for %s %s; see %s\n' "$os" "$machine" "$RELEASES_PAGE"
          exit 1
        fi
        ;;
      *)
        error 'farhelm has no release build for %s %s; see %s\n' "$os" "$machine" "$RELEASES_PAGE"
        exit 1
        ;;
    esac

    # FARHELM_INSTALL_TEST_BASE_URL exists only so this script's own test
    # suites can point it at a fixture server instead of GitHub. It
    # deliberately has its own name: the helm's release mirror setting,
    # FARHELM_RELEASE_BASE_URL, is safe for the helm because it verifies the
    # release signature, and this script verifies none (SPEC.md,
    # "Installation and updates"), so the helm's variable left exported in a
    # shell must never redirect an install. Plain HTTP is accepted only for
    # a loopback fixture (see validate_release_base_url); an HTTPS value
    # keeps the same no-downgrade pinning as the default GitHub channel.
    if [ -n "${FARHELM_INSTALL_TEST_BASE_URL:-}" ]; then
      if ! validate_release_base_url "$FARHELM_INSTALL_TEST_BASE_URL"; then
        error 'FARHELM_INSTALL_TEST_BASE_URL must be an https URL, or an http URL on this machine (127.0.0.1, localhost or [::1]), with a host and no userinfo, query, or fragment; refusing it\n'
        exit 1
      fi
      case "$FARHELM_INSTALL_TEST_BASE_URL" in
        http://*)
          CURL_PROTOCOL_MODE=loopback
          # The validator accepted only 127.0.0.1, localhost or [::1],
          # optionally with a port; keep just the host for --connect-to.
          CURL_LOOPBACK_HOST=${FARHELM_INSTALL_TEST_BASE_URL#http://}
          CURL_LOOPBACK_HOST=${CURL_LOOPBACK_HOST%%/*}
          case "$CURL_LOOPBACK_HOST" in
            '['*) CURL_LOOPBACK_HOST="${CURL_LOOPBACK_HOST%%]*}]" ;;
            *) CURL_LOOPBACK_HOST=${CURL_LOOPBACK_HOST%%:*} ;;
          esac
          ;;
      esac
      printf 'using FARHELM_INSTALL_TEST_BASE_URL=%s\n' "$FARHELM_INSTALL_TEST_BASE_URL" >&2
    fi

    # 2. Prerequisites.
    #
    # Checked before anything network-bound so a missing tool is reported
    # immediately, by name, rather than surfacing as a confusing failure
    # three steps in. The checksum tool is picked ONCE, here, rather than
    # re-probed every time sha256_of runs.
    missing=""
    command -v curl >/dev/null 2>&1 || missing="$missing curl"
    if command -v tar >/dev/null 2>&1; then
      # `tar` existing is not enough: every release archive is gzip, and
      # some tar builds shell out to a separate `gzip` binary to read one
      # rather than decompressing it themselves. Prove actual capability by
      # listing a tiny embedded fixture archive rather than just checking
      # whether `gzip` happens to be on PATH — a tar with BUILT-IN gzip
      # support (BusyBox, libarchive-based bsdtar) needs no such thing and
      # must not be penalized for lacking it.
      # shellcheck disable=SC2059 # $GZIP_TAR_PROBE's backslash escapes ARE the format string; nothing here is user data
      if ! printf "$GZIP_TAR_PROBE" | tar tzf - >/dev/null 2>&1; then
        missing="$missing tar-with-gzip-support"
      fi
    else
      missing="$missing tar"
    fi
    if command -v sha256sum >/dev/null 2>&1; then
      CHECKSUM_TOOL=sha256sum
    elif command -v shasum >/dev/null 2>&1; then
      CHECKSUM_TOOL=shasum
    elif command -v openssl >/dev/null 2>&1; then
      CHECKSUM_TOOL=openssl
    else
      CHECKSUM_TOOL=""
      missing="$missing sha256sum-or-shasum-or-openssl"
    fi
    if [ -n "$missing" ]; then
      error 'farhelm'"'"'s installer needs:%s\n' "$missing"
      exit 1
    fi

    # 3. Version.
    #
    # FARHELM_VERSION pins a release, including a -rc.N or -dev.N prerelease
    # (D15); otherwise the script asks GitHub which tag "latest" currently
    # means. Either way, exactly one candidate string and one error message
    # are produced here, and normalize_version validates and normalizes that
    # single candidate the same way regardless of where it came from.
    if [ -n "${FARHELM_VERSION:-}" ]; then
      candidate=$FARHELM_VERSION
      version_error="FARHELM_VERSION='$FARHELM_VERSION' is not X.Y.Z, vX.Y.Z, or a -rc.N or -dev.N prerelease of one"
    else
      # No -L: the redirect itself is the answer (a 302 whose Location
      # names the release tag), not something to chase. `-I` sends HEAD, so
      # this costs one round trip and no body.
      latest_raw=$(curl_get -sI "$LATEST_URL") || latest_raw=""
      latest_raw=$(printf '%s' "$latest_raw" | tr -d '\r')
      # Through an HTTP(S) proxy, curl -I can print the CONNECT tunnel's
      # own "200 Connection established" response ahead of the target's
      # real one, as two blank-line-separated header blocks; without -L
      # there is still exactly one block from the target itself, and it is
      # always the LAST one.
      latest_block=$(printf '%s\n' "$latest_raw" | awk 'BEGIN{RS=""} {block=$0} END{print block}')
      latest_status=$(printf '%s\n' "$latest_block" | awk 'NR==1{print $2}')
      # Header names are case-insensitive per RFC 9110; lowercase both
      # sides before matching rather than trusting GitHub to always spell
      # it "Location", and trim trailing header-value whitespace before
      # using it.
      latest_location=$(printf '%s\n' "$latest_block" | awk '
        {
          line = $0
          if (tolower(line) ~ /^location:[ \t]*/) {
            sub(/^[^:]*:[ \t]*/, "", line)
            sub(/[ \t]+$/, "", line)
            print line
            exit
          }
        }
      ')
      # The Location value may be absolute or host-relative; either way
      # the tag is whatever follows the final slash.
      candidate=${latest_location##*/}
      version_error="could not determine the latest release from GitHub (HTTP ${latest_status:-000}); set FARHELM_VERSION=vX.Y.Z or check $RELEASES_PAGE"
      if [ "$latest_status" != "302" ]; then
        # Force the normalize_version call below to fail with
        # version_error above, rather than duplicating this branch's
        # error handling.
        candidate=""
      fi
    fi
    # Builds of main carry the version 0.0.0-unreleased rather than a
    # release number (the root Cargo.toml explains why), and the installed
    # uninstall acceptance suite (scripts/test-uninstall.py) installs such a
    # build through this script as its "current" release. The sentinel names
    # no published release, so it is accepted ONLY together with the
    # test-only base URL; pinned against GitHub it stays a version error like
    # any other non-release suffix.
    if [ -n "${FARHELM_INSTALL_TEST_BASE_URL:-}" ] &&
      { [ "$candidate" = "0.0.0-unreleased" ] || [ "$candidate" = "v0.0.0-unreleased" ]; }; then
      VERSION_TAG=v0.0.0-unreleased
    elif ! VERSION_TAG=$(normalize_version "$candidate"); then
      error '%s\n' "$version_error"
      exit 1
    fi
    VERSION_NUM=${VERSION_TAG#v}

    # A real install always uses the release's normal download URL; the
    # test-only override is described where it is validated above.
    BASE_URL=${FARHELM_INSTALL_TEST_BASE_URL:-$DOWNLOAD_PREFIX/$VERSION_TAG}
    BASE_URL=${BASE_URL%/}

    INSTALL_DIR="$HOME/.local/bin"
    # Mask group/world write bits on any directory COMPONENT this specific
    # call creates (umask 000 would otherwise leave a brand-new directory
    # mode 0777, and another account could replace the Terminal link this
    # run puts there). A directory that already existed is left exactly
    # as it was: this is not a general permission-hardening pass over
    # someone's chosen install location, only over what this run itself
    # creates. The subshell is important because mkdir -p can create
    # intermediate components that the leaf chmod below cannot protect.
    install_dir_existed=1
    [ -d "$INSTALL_DIR" ] || install_dir_existed=0
    (umask 022; mkdir -p "$INSTALL_DIR")
    if [ "$install_dir_existed" -eq 0 ]; then
      chmod 0755 "$INSTALL_DIR"
    fi

    # 4. Stage everything in a private scratch directory. Nothing is moved
    # from here into the installation directly: the bundle step copies each
    # file next to its destination first and renames it from there, so the
    # final step is atomic whichever filesystem this directory is on.
    STAGING_DIR=$(mktemp -d "$INSTALL_DIR/.farhelm-install.XXXXXX")
    # Set by the bundle step; see release_bundle_lock.
    BUNDLE_LOCK_HELD=0
    BUNDLE_WORK=""
    UPDATE_WORK=""
    REPLACE_TMP=""
    trap cleanup EXIT
    # Translate the catchable termination signals into a plain `exit`,
    # which runs the EXIT trap above — the same cleanup either way, without
    # running it twice. SIGKILL and power loss cannot be caught by any
    # trap; see cleanup's docstring for what that does and does not leave
    # behind.
    trap 'exit 129' HUP
    trap 'exit 130' INT
    trap 'exit 143' TERM

    # SHA256SUMS first: every other download's integrity depends on it, so
    # its own failure modes get named error messages instead of falling
    # through to curl's generic one. `-L` here matters: GitHub serves
    # release assets — SHA256SUMS included — through a redirect to object
    # storage, so without it this request never reaches the manifest at
    # all and looks like a permanent failure. `-w '%{http_code}'` reports
    # the status of the FINAL response in the chain, which is what the
    # branches below need.
    #
    # `-f` makes curl itself exit non-zero on a 4xx/5xx response — but it
    # still writes the real code to `-w` first, so the `|| true` here MUST
    # sit inside the substitution (letting curl's already-captured output
    # stand) rather than after it as a separate fallback assignment, which
    # would silently replace a perfectly good "404" with the wrong "000"
    # every time `-f` made curl's own exit status non-zero — precisely the
    # case this whole block exists to detect. `-w` itself already prints
    # "000" on a total connection failure (no response received at all),
    # so nothing else is needed to cover that case.
    if [ "$ERR_TERMINAL" -eq 1 ]; then
      printf '⏳ %sDownloading Farhelm %s%s\n' "$ERR_BOLD" "$VERSION_NUM" "$ERR_RESET" >&2
      printf '%s   If it looks stuck, it is safe to press Ctrl-C and run the same command again.%s\n\n' "$ERR_DIM" "$ERR_RESET" >&2
    fi
    sums_url="$BASE_URL/SHA256SUMS"
    sums_status=$(curl_get -fsSL -w '%{http_code}' -o "$STAGING_DIR/SHA256SUMS" "$sums_url" 2>/dev/null || true)
    if [ "$sums_status" = 404 ]; then
      # D17: a 404 cannot tell "no such release" from "still publishing"
      # apart, so both this script and the helm say exactly the same
      # thing rather than guessing. Deviation from D17's own wording: the
      # helm's version of this message ends "...or pass --payload-dir", a
      # flag this script does not have; here it ends by pointing at the
      # releases page instead.
      error 'no SHA256SUMS for %s at %s (HTTP 404): the release is not published or is still publishing; retry in a few minutes, or check %s\n' "$VERSION_TAG" "$BASE_URL" "$RELEASES_PAGE"
      exit 1
    elif [ "$sums_status" != 200 ]; then
      error 'download failed (HTTP %s): %s\n' "$sums_status" "$sums_url"
      exit 1
    fi

    # 5. Download, verify, and unpack the CLI and desktop archives. The
    # table keeps Linux rows for release-asset parity, not installation.
    # Reading $ASSET_TABLE through a heredoc rather than a pipe is
    # deliberate: a `command | while read` loop runs the loop body in a
    # subshell under POSIX sh, and every variable this loop sets needs to
    # outlive it.
    #
    # ICNS_STATE is one of those outliving variables: `staged` once the
    # desktop archive yielded a Farhelm.icns, `absent` for old releases.
    # Refuse those before taking the install lock or replacing any file.
    ICNS_STATE=absent
    while IFS='|' read -r row_target row_archive row_binary; do
      [ "$row_target" = "$TARGET" ] || continue

      archive_path="$STAGING_DIR/$row_archive"
      if [ "$ERR_TERMINAL" -eq 1 ]; then
        # Approved numbering follows the two Darwin rows in the asset table.
        case "$row_binary" in
          farhelm) download_label='[1/2]'; download_name='farhelm command-line tool' ;;
          farhelm-desktop) download_label='[2/2]'; download_name='Farhelm app' ;;
        esac
        printf '   %s%s%s %s\n' "$ERR_BOLD" "$download_label" "$ERR_RESET" "$download_name" >&2
        curl_get -fL --show-error --progress-bar --retry 3 -o "$archive_path" "$BASE_URL/$row_archive"
      else
        curl_get -fsSL --retry 3 -o "$archive_path" "$BASE_URL/$row_archive"
      fi

      # Exactly one SHA256SUMS line must name this archive (D3's
      # "sign-sums" job asserts the same on the publishing side) — awk's
      # exact field comparison, not a substring grep, so one archive's
      # name being a prefix of another's can never cross-match.
      sums_hits=$(awk -v f="$row_archive" '$2==f{c++} END{print c+0}' "$STAGING_DIR/SHA256SUMS")
      if [ "$sums_hits" -ne 1 ]; then
        error 'SHA256SUMS has %s entries for %s, expected exactly 1\n' "$sums_hits" "$row_archive"
        exit 1
      fi
      expected_sha256=$(awk -v f="$row_archive" '$2==f{print $1; exit}' "$STAGING_DIR/SHA256SUMS")
      if ! actual_sha256=$(sha256_of "$archive_path"); then
        error '%s: could not compute a checksum (%s failed reading %s)\n' "$row_archive" "$CHECKSUM_TOOL" "$archive_path"
        exit 1
      fi
      if [ "$actual_sha256" != "$expected_sha256" ]; then
        error '%s: checksum mismatch (expected %s, got %s)\n' "$row_archive" "$expected_sha256" "$actual_sha256"
        exit 1
      fi

      # Selection, refusal rules, and -O streaming all live in
      # extract_sole_member; see its own comment for why each refusal
      # exists.
      extract_sole_member "$archive_path" "$row_archive" "$row_binary" "$STAGING_DIR/$row_binary" 0755

      # The desktop archive also carries the app icon the bundle step below
      # builds Farhelm.app around. Releases published before the icon
      # existed do not have it; the pre-commit gate below refuses them.
      # A present icon gets the same extraction
      # discipline as a binary. More than one match is the one shape that
      # is never legitimate.
      if [ "$row_binary" = "farhelm-desktop" ]; then
        icns_hits=$(member_count_named "$archive_path" "Farhelm.icns")
        if [ "$icns_hits" -gt 1 ]; then
          error '%s has %s members named Farhelm.icns, expected at most 1\n' "$row_archive" "$icns_hits"
          exit 1
        fi
        if [ "$icns_hits" -eq 1 ]; then
          extract_sole_member "$archive_path" "$row_archive" "Farhelm.icns" "$STAGING_DIR/Farhelm.icns" 0644
          ICNS_STATE=staged
        fi
      fi
    done <<EOF
$ASSET_TABLE
EOF

    # The asset table gives every one of the three targets a farhelm row
    # (and gives macOS's aarch64-apple-darwin one further farhelm-desktop
    # row), so a missing $STAGING_DIR/farhelm here means the table and the
    # platform switch above have drifted apart, not a normal failure a
    # user can act on. Reading this fixed path directly, rather than
    # through a separate "did we stage one" variable, is enough: the loop
    # above can only ever write it at this one path.
    if [ ! -e "$STAGING_DIR/farhelm" ]; then
      error 'internal error: no farhelm archive matched target %s\n' "$TARGET"
      exit 1
    fi

    # Belt-and-braces beyond the checksum: prove the binary we just staged
    # actually runs and claims to be the version we asked for, before it
    # ever touches the real install directory.
    reported_version=$("$STAGING_DIR/farhelm" --version)
    expected_version_line="farhelm $VERSION_NUM"
    if [ "$reported_version" != "$expected_version_line" ]; then
      error 'downloaded farhelm reports '"'"'%s'"'"', expected '"'"'%s'"'"'; refusing to install\n' "$reported_version" "$expected_version_line"
      exit 1
    fi

    # Old archives cannot supply the app this installer promises. Staging
    # may have created the bin directory, but no installed file has changed.
    if [ "$ICNS_STATE" != staged ]; then
      error 'Farhelm %s is too old for this installer: it has no Mac app.\n' "$VERSION_NUM"
      printf '%s\n' '   Pick 0.2.1 or newer, or leave FARHELM_VERSION unset for the latest release.' >&2
      exit 1
    fi

    # 6. Older releases. A release from before the side-by-side version
    # layout would install a desktop app that starts its supervisor from its
    # sibling, which in this layout is the forwarder, so it is refused here,
    # before anything installed changes. The marker is a fixed piece of text
    # that every desktop build since the layout carries (the refusal it
    # prints when its own version folder is missing); a Rust test in the
    # desktop crate keeps the two in step.
    if ! LC_ALL=C grep -qF 'needs its own version of the farhelm binary at' "$STAGING_DIR/farhelm-desktop"; then
      error 'Farhelm %s is too old for this installer: it cannot be updated while it runs.\n' "$VERSION_NUM"
      printf '%s\n' '   Pick a newer release, or leave FARHELM_VERSION unset for the latest release.' >&2
      exit 1
    fi

    # 7. Install the app. ~/Applications/Farhelm.app is the whole
    # installation (SPEC_impl.md, "Side-by-side versions inside
    # Farhelm.app"): each version's farhelm in its own Contents/Versions/<v>/
    # folder, the Installed record Contents/Versions/installed naming the one
    # the next start uses, the forwarder at Contents/MacOS/farhelm, the app's
    # main program Contents/MacOS/farhelm-desktop, and an ownership record.
    # ~/.local/bin/farhelm is only a symlink to the forwarder, for Terminal.
    #
    # The bundle is what makes the app reachable by name from
    # Spotlight/Alfred, gives it a Dock icon and a Cmd-Tab name, and makes a
    # second launch activate the running instance instead of racing it for
    # the embedded helm's state. The executable KEEPS the name
    # farhelm-desktop inside it: the default APFS is case-insensitive, so an
    # executable named "Farhelm" would be the same directory entry as the
    # forwarder "farhelm". The pretty name comes from CFBundleName.
    app_parent="$HOME/Applications"
    app_path="$app_parent/Farhelm.app"
    contents="$app_path/Contents"
    forwarder="$contents/MacOS/farhelm"

    bundle_fail() {
      error 'installing %s failed at: %s\n' "$app_path" "$1"
      printf 'Re-run the installer to retry.\n' >&2
      exit 1
    }

    # One run at a time, through the lock beside the bundle that every
    # installer since the bundle existed takes. Contention refuses, as does
    # a lock an interrupted run left behind, which nothing can tell from a
    # live one.
    (umask 022; mkdir -p "$app_parent") || bundle_fail "creating $app_parent"
    BUNDLE_LOCK="$app_parent/.farhelm-app.lock"
    if ! (umask 077; mkdir "$BUNDLE_LOCK") 2>/dev/null; then
      error 'another farhelm install is changing %s right now, or one was interrupted while doing so; wait a moment and re-run -- if this persists, remove %s by hand\n' "$app_path" "$BUNDLE_LOCK"
      exit 1
    fi
    BUNDLE_LOCK_HELD=1

    # The record names the Terminal link this installation owns, by the
    # canonical path of its directory. The sentinel keeps a trailing newline
    # in the path through command substitution, and CDPATH cannot redirect
    # the cd or make it print.
    canonical_bin=$(
      CDPATH='' cd -P "$INSTALL_DIR" || exit 1
      pwd -P || exit 1
      printf '%s' '__FARHELM_CANONICAL_PATH_END__'
    ) || bundle_fail "resolving $INSTALL_DIR"
    canonical_bin=${canonical_bin%__FARHELM_CANONICAL_PATH_END__}
    canonical_bin=${canonical_bin%"$NEWLINE"}

    # Which kind of run this is. Only a bundle this installer can show it
    # built is ever changed: one with this installation's current record,
    # or, for the one-time move from the layout that copied both binaries
    # into the bundle, an old bundle whose old record names this
    # installation's bin directory, or the recordless shape from before
    # records existed. Old installations elsewhere (custom or moved
    # directories) are refused rather than recognized; that layout's own
    # uninstall, or removing the bundle by hand, comes first.
    updated_installation=0
    if current_record_is_ours "$contents/.farhelm-installation" "$canonical_bin/farhelm" ||
      current_record_moved_here "$contents/.farhelm-installation"; then
      updated_installation=1
      if [ -d "$contents/Versions" ] && [ ! -L "$contents/Versions" ]; then
        install_mode=update
      else
        install_mode=rebuild
      fi
    elif [ -e "$app_path" ] || [ -L "$app_path" ]; then
      if bundle_record_is_ours "$app_path" "$canonical_bin" || is_legacy_installer_bundle "$app_path"; then
        updated_installation=1
        install_mode=rebuild
      else
        error '%s exists and is not an app bundle this installer built; refusing to replace it.\n' "$app_path"
        printf 'Remove or rename that bundle and re-run.\n' >&2
        exit 1
      fi
    else
      install_mode=rebuild
    fi

    previous_installed=""
    if [ "$install_mode" = update ]; then
      # In place, while an older Farhelm may be running from this very
      # bundle. Each step renames one complete, signed file (or a complete
      # version folder) into place, in an order where stopping after any of
      # them leaves an app that launches the old version or the new one: the
      # new version's folder first, then the main program that starts it,
      # then the Installed record, then Info.plist. The folder is never
      # replaced as a whole and nothing is re-signed (a running app survives
      # this, and survives nothing else; see SPEC_impl.md).
      UPDATE_WORK=$(mktemp -d "$app_parent/.farhelm-update.XXXXXX") || bundle_fail "creating a private work directory in $app_parent"
      # An interrupted uninstall can leave the app without some of its
      # folders, and an installer killed between copying a file beside its
      # destination and renaming it leaves that copy behind, which
      # uninstall would refuse as foreign. Both are repaired here, under
      # the lock, before anything is replaced.
      (umask 022; mkdir -p "$contents/MacOS" "$contents/Resources") || bundle_fail "recreating the app's folders"
      for leftover in "$contents"/.farhelm-new.* "$contents"/MacOS/.farhelm-new.* \
        "$contents"/Resources/.farhelm-new.* "$contents"/Versions/.farhelm-new.*; do
        if [ -f "$leftover" ] && [ ! -L "$leftover" ]; then
          rm -f "$leftover"
        fi
      done
      if [ -f "$contents/Versions/installed" ]; then
        IFS= read -r previous_installed <"$contents/Versions/installed" || :
      fi
      # A version folder that already holds its farhelm is kept as it is:
      # a running Farhelm may be using it, and a release's bytes do not
      # change. Only a folder without one is replaced, and it is put back
      # if the replacement fails, so a failure never loses a version.
      new_version_dir="$contents/Versions/$VERSION_NUM"
      if [ ! -f "$new_version_dir/farhelm" ] || [ -L "$new_version_dir/farhelm" ] || [ -L "$new_version_dir" ]; then
        stage_version_dir
        if [ -e "$new_version_dir" ] || [ -L "$new_version_dir" ]; then
          mv "$new_version_dir" "$UPDATE_WORK/replaced-version" || bundle_fail "moving the incomplete $VERSION_NUM folder aside"
        fi
        if ! mv "$UPDATE_WORK/version" "$new_version_dir"; then
          if [ -e "$UPDATE_WORK/replaced-version" ] || [ -L "$UPDATE_WORK/replaced-version" ]; then
            mv "$UPDATE_WORK/replaced-version" "$new_version_dir" || :
          fi
          bundle_fail "moving the $VERSION_NUM folder into place"
        fi
      fi
      replace_file "$STAGING_DIR/farhelm-desktop" "$contents/MacOS/farhelm-desktop" 0755 "the app's main program"
      replace_file "$STAGING_DIR/Farhelm.icns" "$contents/Resources/Farhelm.icns" 0644 "the icon"
      write_forwarder "$UPDATE_WORK/forwarder" || bundle_fail "writing the forwarder"
      if ! same_bytes "$UPDATE_WORK/forwarder" "$forwarder"; then
        replace_file "$UPDATE_WORK/forwarder" "$forwarder" 0755 "the forwarder"
      fi
      printf '%s\n' "$VERSION_NUM" >"$UPDATE_WORK/installed" || bundle_fail "writing the Installed record"
      replace_file "$UPDATE_WORK/installed" "$contents/Versions/installed" 0644 "the Installed record"
      write_info_plist "$UPDATE_WORK/Info.plist" || bundle_fail "writing Info.plist"
      replace_file "$UPDATE_WORK/Info.plist" "$contents/Info.plist" 0644 "Info.plist"
      write_current_record "$UPDATE_WORK/record" "$canonical_bin/farhelm" || bundle_fail "writing installer ownership metadata"
      replace_file "$UPDATE_WORK/record" "$contents/.farhelm-installation" 0600 "installer ownership metadata"
    else
      # Built whole in a private directory beside the bundle, so the final
      # move is a rename on the same filesystem. A previous bundle is moved
      # into the same directory before the swap, so the public name only
      # ever holds the old bundle, nothing, or the new one, and everything
      # deleted afterwards is this run's own. This path is a fresh install,
      # the one-time move from the old layout, and a repair; the old layout
      # is replaced with Farhelm quit (docs/install_uninstall.md says so).
      BUNDLE_WORK=$(mktemp -d "$app_parent/.farhelm-app-build.XXXXXX") || bundle_fail "creating a private build directory in $app_parent"
      bundle_stage="$BUNDLE_WORK/Farhelm.app"
      (umask 022; mkdir -p "$bundle_stage/Contents/MacOS" "$bundle_stage/Contents/Resources" "$bundle_stage/Contents/Versions/$VERSION_NUM") || bundle_fail "creating the staging layout"
      cp "$STAGING_DIR/farhelm" "$bundle_stage/Contents/Versions/$VERSION_NUM/farhelm" || bundle_fail "copying farhelm"
      cp "$STAGING_DIR/farhelm-desktop" "$bundle_stage/Contents/MacOS/farhelm-desktop" || bundle_fail "copying farhelm-desktop"
      write_forwarder "$bundle_stage/Contents/MacOS/farhelm" || bundle_fail "writing the forwarder"
      chmod 0755 "$bundle_stage/Contents/Versions/$VERSION_NUM/farhelm" "$bundle_stage/Contents/MacOS/farhelm-desktop" "$bundle_stage/Contents/MacOS/farhelm" || bundle_fail "setting program modes"
      cp "$STAGING_DIR/Farhelm.icns" "$bundle_stage/Contents/Resources/Farhelm.icns" || bundle_fail "copying the icon"
      printf '%s\n' "$VERSION_NUM" >"$bundle_stage/Contents/Versions/installed" || bundle_fail "writing the Installed record"
      chmod 0644 "$bundle_stage/Contents/Resources/Farhelm.icns" "$bundle_stage/Contents/Versions/installed" || bundle_fail "setting file modes"
      write_info_plist "$bundle_stage/Contents/Info.plist" || bundle_fail "writing Info.plist"
      chmod 0644 "$bundle_stage/Contents/Info.plist" || bundle_fail "setting the Info.plist mode"
      write_current_record "$bundle_stage/Contents/.farhelm-installation" "$canonical_bin/farhelm" || bundle_fail "writing installer ownership metadata"
      if [ -e "$app_path" ] || [ -L "$app_path" ]; then
        mv "$app_path" "$BUNDLE_WORK/previous" || bundle_fail "moving the previous bundle aside (grant your terminal App Management in System Settings > Privacy & Security if this said 'Operation not permitted')"
      fi
      mv "$bundle_stage" "$app_path" || bundle_fail "moving the staged bundle into place"
      rm -rf "$BUNDLE_WORK" || printf 'note: could not delete %s, which held the previous bundle; it is safe to delete\n' "$BUNDLE_WORK" >&2
      BUNDLE_WORK=""
    fi

    # Spotlight and Launch Services keep showing the old version until the
    # bundle is touched or re-registered (observed on a real Mac). The
    # binary does not exist on the Linux CI host the installer tests run
    # on, hence the -x guard.
    touch "$app_path" 2>/dev/null || true
    LSREGISTER="/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"
    if [ -x "$LSREGISTER" ]; then
      "$LSREGISTER" -f "$app_path" >/dev/null 2>&1 || true
    fi

    # The Terminal link, and the copies the layout before this one kept in
    # the bin directory. A copy is removed only when its checksum matches
    # what that layout's record says the installer last put there; a
    # farhelm of the user's own is renamed to a visible name instead, since
    # its name is needed for the link. A farhelm-desktop of the user's own
    # is left where it is.
    KEPT_NOTES=""
    kept_stamp=$(date -u +%Y%m%dT%H%M%SZ)
    for name in farhelm farhelm-desktop; do
      dest="$INSTALL_DIR/$name"
      if [ -L "$dest" ]; then
        [ "$name" = farhelm ] && [ "$(readlink "$dest")" = "$forwarder" ] && continue
      elif [ ! -e "$dest" ]; then
        continue
      elif [ -f "$dest" ]; then
        recorded_sha=$(recorded_digest_of "$name")
        if [ -n "$recorded_sha" ] && existing_sha=$(sha256_of "$dest" 2>/dev/null) \
          && [ "$existing_sha" = "$recorded_sha" ]; then
          if [ "$name" = farhelm-desktop ]; then
            rm -f "$dest" || error 'note: could not remove the old %s; it is safe to delete\n' "$dest"
          fi
          continue
        fi
      fi
      [ "$name" = farhelm ] || continue
      if [ -d "$dest" ] && [ ! -L "$dest" ]; then
        error '%s is a directory; the app is installed, but the Terminal link was not created; move it aside and re-run\n' "$dest"
        exit 1
      fi
      kept="$INSTALL_DIR/$name.replaced-$kept_stamp"
      if [ -e "$kept" ] || [ -L "$kept" ]; then
        kept="$kept-$$"
      fi
      mv "$dest" "$kept" || { error 'could not keep the existing %s as %s; move it aside and re-run the installer\n' "$dest" "$kept"; exit 1; }
      KEPT_NOTES="${KEPT_NOTES}ℹ️  ~/.local/bin/$name was not installed by this installer, so it was renamed
   to ~/.local/bin/${kept##*/} - farhelm installation still
   proceeded.$NEWLINE"
    done
    if [ ! -L "$INSTALL_DIR/farhelm" ] || [ "$(readlink "$INSTALL_DIR/farhelm")" != "$forwarder" ]; then
      link_stage="$INSTALL_DIR/.farhelm-link.$$"
      rm -f "$link_stage"
      ln -s "$forwarder" "$link_stage" || bundle_fail "creating the Terminal link"
      mv -f "$link_stage" "$INSTALL_DIR/farhelm" || { rm -f "$link_stage"; bundle_fail "installing the Terminal link $INSTALL_DIR/farhelm"; }
    fi
    # The old layout's record of the bin-directory copies goes once they
    # are gone; the app's record is the installation's only record now.
    if [ -f "$INSTALL_DIR/.farhelm-installation" ] && [ ! -L "$INSTALL_DIR/.farhelm-installation" ]; then
      rm -f "$INSTALL_DIR/.farhelm-installation" || error 'note: could not remove %s, the previous layout'"'"'s record; it is safe to delete\n' "$INSTALL_DIR/.farhelm-installation"
    fi

    # Version folders other than the one just installed, the one it
    # replaced, and the one a running Farhelm started from (its supervisor's
    # Running record in the default state directory, or in the one this
    # shell's XDG_STATE_HOME names) are removed. Anything that is not named
    # like a version is not this installer's and stays. A Farhelm running
    # with another overridden state directory keeps its version only
    # through the "just replaced" rule, that is, for one update.
    if [ "$install_mode" = update ]; then
      # Both the default state directory, which a Farhelm opened from
      # Finder or the Dock always uses whatever this shell says, and the
      # one this shell's absolute XDG_STATE_HOME names.
      running_version=""
      running_version_xdg=""
      if [ -f "$HOME/.local/state/farhelm/running-version" ]; then
        IFS= read -r running_version <"$HOME/.local/state/farhelm/running-version" || :
      fi
      case "${XDG_STATE_HOME:-}" in
        /*)
          if [ -f "$XDG_STATE_HOME/farhelm/running-version" ]; then
            IFS= read -r running_version_xdg <"$XDG_STATE_HOME/farhelm/running-version" || :
          fi
          ;;
      esac
      for version_dir in "$contents/Versions"/*; do
        if [ ! -d "$version_dir" ] || [ -L "$version_dir" ]; then
          continue
        fi
        version_name=${version_dir##*/}
        is_version_folder_name "$version_name" || continue
        [ "$version_name" = "$VERSION_NUM" ] && continue
        [ "$version_name" = "$previous_installed" ] && continue
        [ "$version_name" = "$running_version" ] && continue
        [ "$version_name" = "$running_version_xdg" ] && continue
        rm -rf "$version_dir" || error 'note: could not remove the old version folder %s; it is safe to delete\n' "$version_dir"
      done
    fi
    release_bundle_lock

    # 8. Report. Choose the action from this run's outcome and the local
    # tmux prerequisite. Fresh-install launch advice follows its remedy. An
    # update keeps the approved restart/session-survival line and also ends
    # with the prescribed reminder after tmux advice. Uninstall stays visible.
    # Parse the whole tmux version output, not a matching line inside a banner.
    tmux_have="none"
    meets_floor=0
    tmux_present=0
    if command -v tmux >/dev/null 2>&1; then
      tmux_present=1
      tmux_version_output=$(tmux -V 2>/dev/null || true)
      tmux_have=${tmux_version_output#tmux }
    else
      tmux_version_output=""
    fi
    case "$tmux_version_output" in
      *"$NEWLINE"* | *"$CR"*) parsed="" ;;
      *) parsed=$(printf '%s' "$tmux_version_output" | sed -n 's/^tmux \([0-9][0-9]*\)\.\([0-9][0-9]*\)\([a-z]\{0,1\}\)$/\1 \2 \3/p') ;;
    esac
    if [ -n "$parsed" ]; then
      # shellcheck disable=SC2086 # word-splitting $parsed into its 2-3 fields is the point
      set -- $parsed
      tmux_major=$1
      tmux_minor=$2
      tmux_letter=${3:-}
      if [ "$tmux_major" -gt 3 ]; then
        meets_floor=1
      elif [ "$tmux_major" -eq 3 ]; then
        if [ "$tmux_minor" -gt 7 ]; then
          meets_floor=1
        elif [ "$tmux_minor" -eq 7 ]; then
          case "$tmux_letter" in
            "" | a | b) meets_floor=0 ;;
            *) meets_floor=1 ;;
          esac
        fi
      fi
    fi
    if [ "$updated_installation" -eq 1 ]; then
      printf '✅ %sFarhelm %s is ready.%s\n' "$OUT_GREEN" "$VERSION_NUM" "$OUT_RESET"
      printf '\n   Quit and reopen Farhelm to finish updating. Your sessions keep running.\n'
    else
      printf '✅ %sFarhelm %s is installed.%s\n' "$OUT_GREEN" "$VERSION_NUM" "$OUT_RESET"
      if [ "$meets_floor" -eq 1 ]; then
        printf '\n   Open Farhelm from Spotlight or ~/Applications.\n'
      fi
    fi
    if [ -n "$KEPT_NOTES" ]; then
      printf '\n%s' "$KEPT_NOTES"
    fi
    printf '\n%s   To uninstall later, run:%s %s~/.local/bin/farhelm uninstall%s\n' "$OUT_DIM" "$OUT_RESET" "$OUT_CYAN" "$OUT_RESET"
    if [ "$meets_floor" -ne 1 ]; then
      printf '\n⚠️  %sFarhelm needs tmux 3.7c or newer before it can start.%s\n' "$OUT_YELLOW" "$OUT_RESET"
      if [ "$tmux_present" -eq 0 ]; then
        printf '   This Mac has none. Install it with Homebrew: %sbrew install tmux%s\n' "$OUT_CYAN" "$OUT_RESET"
      else
        printf '   This Mac has tmux %s. Upgrade it with Homebrew: %sbrew upgrade tmux%s\n' "$tmux_have" "$OUT_CYAN" "$OUT_RESET"
      fi
      printf '   No Homebrew yet? Install it first: %s%s%s\n' "$OUT_CYAN" "$BREW_LINK" "$OUT_RESET"
      if [ "$updated_installation" -eq 1 ]; then
        printf '\n   Then quit and reopen Farhelm to finish updating.\n'
      else
        printf '\n   Then open Farhelm from Spotlight or ~/Applications.\n'
      fi
    fi
  }

  main "$@"
}
