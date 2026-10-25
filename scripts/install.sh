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
  # downloads the matching release from get.farhelm.io, verifies it against
  # the release's checksums, and puts the binaries in place. The README's "Install" chapter is the same text as
  # this script's behavior and is the place to look for the user-facing
  # story.
  #
  # POSIX sh on purpose, not bash: this is the file `curl | sh` runs on
  # whatever `/bin/sh` a fresh machine happens to have, so it can lean on
  # nothing bash-specific (arrays, `[[`, `local`, process substitution). It
  # is meant to be read before it is run — README says so — so it stays one
  # file, no sourcing, no helper scripts fetched separately.
  set -eu

  # Every release file comes from this one origin, in a fixed layout that
  # is a permanent contract with every client (SPEC_impl.md, "Verification
  # chain (D3)"): /latest names the latest stable tag, and /<tag>/ holds
  # that release's SHA256SUMS, its signature, its install.sh, and a
  # redirect to each archive. Nothing here asks GitHub anything.
  SITE_URL="https://get.farhelm.io"
  RELEASES_PAGE="https://github.com/scode/farhelm/releases"
  CURL_PROTOCOL_MODE=default

  # ---------------------------------------------------------------------
  # What an install changes, in one place
  #
  # The app bundle ~/Applications/Farhelm.app is the whole installation,
  # and ~/.local/bin/farhelm only a symlink into it (SPEC_impl.md,
  # "Side-by-side versions inside Farhelm.app"). This script resolves the
  # version, downloads and checks the release, and stages it; the release's
  # own farhelm then does every change under the installation lock (step 7
  # of main). A fresh install builds the bundle beside its place and renames
  # it in. An update changes it in place, possibly while Farhelm runs from
  # it, one complete file or version folder at a time in an order where
  # stopping after any step leaves an app that launches the old version or
  # the new one, and the ownership record says the install is unfinished
  # until it is. Running the installer again finishes the job.
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
# they are (the shell itself may add PWD, SHLVL and _; descriptor 8 is the
# forwarder's own, see below). Written by Farhelm's installer; replaced only
# by a newer one that forwards everything this one does the same way.
#
# Before starting a version, the forwarder claims it, so an install or an
# uninstall cannot delete it between the choice and the start: it opens the
# version's farhelm on descriptor 8 and has that farhelm, run once with no
# streams, take a shared lock on it. The lock belongs to the open file, so it
# stays after that run exits and is carried, through descriptor 8 and
# FARHELM_VERSION_LOCK_FD, into the program started next, which adopts it. A
# farhelm that predates version locks rejects the claim as an unknown command
# (exit status 2) and is started without one, as before.
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
unset FARHELM_VERSION_LOCK_FD
farhelm_forwarder_use_running=1
farhelm_forwarder_attempt=0
while [ "$farhelm_forwarder_attempt" -lt 3 ]; do
  farhelm_forwarder_attempt=$((farhelm_forwarder_attempt + 1))
  farhelm_forwarder_version=
  farhelm_forwarder_from_running=0
  if [ "$farhelm_forwarder_use_running" = 1 ] && [ -n "${FARHELM_SUPERVISOR_SOCK:-}" ] && [ -f "${FARHELM_SUPERVISOR_SOCK%/*}/running-version" ]; then
    IFS= read -r farhelm_forwarder_version <"${FARHELM_SUPERVISOR_SOCK%/*}/running-version" || :
    farhelm_forwarder_from_running=1
  fi
  if [ -z "$farhelm_forwarder_version" ] && [ -f "$farhelm_forwarder_versions/installed" ]; then
    IFS= read -r farhelm_forwarder_version <"$farhelm_forwarder_versions/installed" || :
    farhelm_forwarder_from_running=0
  fi
  case $farhelm_forwarder_version in
    '' | .* | */*)
      printf 'farhelm: no installed version is recorded in %s; reinstall Farhelm\n' "$farhelm_forwarder_versions" >&2
      exit 127
      ;;
  esac
  farhelm_forwarder_program=$farhelm_forwarder_versions/$farhelm_forwarder_version/farhelm
  if [ ! -x "$farhelm_forwarder_program" ]; then
    if [ "$farhelm_forwarder_from_running" = 1 ]; then
      # The Running record names a version that is gone, so the supervisor
      # that wrote it is not running any more: use the Installed version.
      farhelm_forwarder_use_running=0
      continue
    fi
    if [ "$farhelm_forwarder_attempt" -lt 3 ]; then
      # Installed may have moved on since it was read, and an install may
      # then have pruned the version it named: read Installed again rather
      # than fail on a choice that is already stale.
      continue
    fi
    printf 'farhelm: version %s is not installed in %s; reinstall Farhelm\n' "$farhelm_forwarder_version" "$farhelm_forwarder_versions" >&2
    exit 127
  fi
  # `command` keeps a failed open from ending the shell (POSIX makes a failed
  # redirection on the exec special built-in fatal); the braces send only the
  # open's own error to /dev/null, not the forwarder's later error output.
  if { command exec 8<"$farhelm_forwarder_program"; } 2>/dev/null; then
    "$farhelm_forwarder_program" internal version-lock 8 </dev/null >/dev/null 2>/dev/null
    case $? in
      0)
        FARHELM_VERSION_LOCK_FD=8
        export FARHELM_VERSION_LOCK_FD
        exec "$farhelm_forwarder_program" "$@"
        ;;
      2 | 69)
        # No version locks here: a farhelm from before them, or a
        # filesystem that cannot lock. Start it as before.
        exec 8<&-
        exec "$farhelm_forwarder_program" "$@"
        ;;
    esac
    exec 8<&-
  fi
done
printf 'farhelm: could not start version %s of Farhelm; it is being updated or uninstalled. Run the command again.\n' "$farhelm_forwarder_version" >&2
exit 127
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
  # validation job (sign-sums.yml) require of these archives.
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
  # The inode of folder $1 (not following a link), or nothing. `ls -di`
  # prints it first on macOS, GNU and BusyBox alike; anything that is not
  # all digits is treated as unreadable.
  folder_inode() {
    fi_out=$(LC_ALL=C ls -di -- "$1" 2>/dev/null) || return 0
    fi_out=${fi_out#"${fi_out%%[![:space:]]*}"}
    fi_out=${fi_out%%[[:space:]]*}
    case $fi_out in
      '' | *[!0-9]*) ;;
      *) printf '%s' "$fi_out" ;;
    esac
  }

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

  # The EXIT/INT/TERM/HUP handler: removes the ephemeral staging directory.
  #
  # What this does NOT do: survive SIGKILL or a power loss. Both skip trap
  # handlers entirely, so a `.farhelm-install.*` staging directory can be
  # left behind; it is inert and safe to delete by hand. Everything inside
  # and beside the app is the locked work's (farhelm internal install), whose
  # leftovers the next run clears under the lock, and whose installation lock
  # the kernel releases with the process.
  cleanup() {
    rm -rf "$STAGING_DIR" 2>/dev/null || true
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
    # suites can point it at a fixture server instead of get.farhelm.io. It
    # stands in for the WHOLE origin (latest, and every <tag>/ path below
    # it), never for one release's directory. It deliberately has its own
    # name: the helm's release mirror setting, FARHELM_RELEASE_BASE_URL, is
    # safe for the helm because it verifies the release signature, and this
    # script verifies none itself (SPEC.md, "Installation and updates"), so
    # the helm's variable left exported in a shell must never redirect an
    # install. Plain HTTP is accepted only for a loopback fixture (see
    # validate_release_base_url); an HTTPS value keeps the same
    # no-downgrade pinning as the default origin.
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
    ORIGIN=${FARHELM_INSTALL_TEST_BASE_URL:-$SITE_URL}
    ORIGIN=${ORIGIN%/}

    # FARHELM_INSTALL_SUMS_FILE names a SHA256SUMS already on this machine,
    # read INSTEAD of fetching the release's: the desktop app's updater
    # verifies the release's signed checksums itself and hands them over
    # here, so every archive this script downloads is checked against
    # signed hashes rather than against a second fetch over TLS. Its
    # interface (this variable and FARHELM_VERSION, and a checksum file that
    # may list more files than this script needs) is a permanent contract
    # with every shipped updater. Checked up front, so a bad path fails
    # before anything is downloaded.
    if [ -n "${FARHELM_INSTALL_SUMS_FILE:-}" ] &&
      { [ ! -f "$FARHELM_INSTALL_SUMS_FILE" ] || [ ! -r "$FARHELM_INSTALL_SUMS_FILE" ]; }; then
      error 'FARHELM_INSTALL_SUMS_FILE=%s is not a readable file; refusing it\n' "$FARHELM_INSTALL_SUMS_FILE"
      exit 1
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
    # (D15); otherwise the script reads the origin's /latest, one line naming
    # the latest STABLE tag. Either way, exactly one candidate string and one
    # error message are produced here, and normalize_version validates and
    # normalizes that single candidate the same way regardless of where it
    # came from.
    LATEST_LOOKUP=0
    if [ -n "${FARHELM_VERSION:-}" ]; then
      candidate=$FARHELM_VERSION
      version_error="FARHELM_VERSION='$FARHELM_VERSION' is not X.Y.Z, vX.Y.Z, or a -rc.N or -dev.N prerelease of one"
    else
      LATEST_LOOKUP=1
      # A small cap: the answer is one tag and a newline, so anything longer
      # is not an answer, and there is no reason to read it. `-w` appends the
      # final HTTP status on a line of its own (curl prints it even when `-f`
      # failed the request, and "000" when nothing answered), so a release
      # that is not published yet can be told apart from a network failure.
      # HTTP 200 can accompany a failed transfer. A shortened tag can still
      # name a real older release, so accepting its grammar is not enough:
      # curl must have received the complete answer before we select it.
      latest_rc=0
      latest_reply=$(curl_get -fsS --max-filesize 256 -w "$NEWLINE%{http_code}" "$ORIGIN/latest" 2>/dev/null) || latest_rc=$?
      latest_status=${latest_reply##*"$NEWLINE"}
      # The body, without the status line. Command substitution dropped the
      # body's own trailing newline only if it was the last thing printed, so
      # strip one here; a carriage return or any other stray byte still makes
      # the value fail the version grammar below rather than being trimmed.
      candidate=${latest_reply%"$NEWLINE"*}
      candidate=${candidate%"$NEWLINE"}
      if [ "$latest_status" = 404 ]; then
        version_error="no release is published on $ORIGIN yet ($ORIGIN/latest answered HTTP 404); set FARHELM_VERSION=vX.Y.Z to pick one"
      else
        version_error="could not determine the latest release from $ORIGIN/latest (HTTP ${latest_status:-000}); set FARHELM_VERSION=vX.Y.Z to pick one"
      fi
      if [ "$latest_status" != 200 ]; then
        candidate=""
      elif [ "$latest_rc" -ne 0 ]; then
        candidate=""
        version_error="the latest-release answer from $ORIGIN/latest did not complete (HTTP 200); set FARHELM_VERSION=vX.Y.Z to pick one"
      fi
      # One line, exactly: normalize_version greps line by line, so a body
      # with a second line could pass on its first and carry the rest along.
      case "$candidate" in
        *"$NEWLINE"* | *"$CR"*) candidate="" ;;
      esac
    fi
    # Builds of main carry the version 0.0.0-unreleased rather than a
    # release number (the root Cargo.toml explains why), and the installed
    # uninstall acceptance suite (scripts/test-uninstall.py) installs such a
    # build through this script as its "current" release. The sentinel names
    # no published release, so it is accepted ONLY together with the
    # test-only base URL; against the real origin it stays a version error
    # like any other non-release suffix.
    if [ -n "${FARHELM_INSTALL_TEST_BASE_URL:-}" ] &&
      { [ "$candidate" = "0.0.0-unreleased" ] || [ "$candidate" = "v0.0.0-unreleased" ]; }; then
      VERSION_TAG=v0.0.0-unreleased
    elif ! VERSION_TAG=$(normalize_version "$candidate"); then
      error '%s\n' "$version_error"
      exit 1
    fi
    # /latest never names a prerelease. One that did is a broken or
    # tampered answer, so it is refused rather than installed; a prerelease
    # is installed only when asked for by name.
    if [ "$LATEST_LOOKUP" -eq 1 ]; then
      case "$VERSION_TAG" in
        *-*)
          error '%s names %s, a prerelease; the latest release is always a stable one, so refusing it\n' "$ORIGIN/latest" "$VERSION_TAG"
          exit 1
          ;;
      esac
    fi
    VERSION_NUM=${VERSION_TAG#v}

    # Everything this release needs lives under its tag on the origin.
    BASE_URL="$ORIGIN/$VERSION_TAG"

    # SHA256SUMS first, and before anything is created on this machine:
    # every other download's integrity depends on it, and a release the
    # origin does not have must be refused with nothing changed (not even
    # an empty ~/.local/bin). It is read into SUMS_TEXT and written into the
    # staging directory once that exists. Its own failure modes get named
    # error messages instead of falling through to curl's generic one. `-L`
    # because the origin may serve it through a redirect; `-w` appends the
    # status of the FINAL response in the chain on a line of its own, which
    # curl prints even when `-f` failed the request, and as "000" when
    # nothing answered at all. The `|| true` sits inside the substitution so
    # that output survives curl's non-zero exit.
    #
    # With FARHELM_INSTALL_SUMS_FILE (see where it is checked above) nothing
    # is fetched: the caller's verified copy is the checksum file.
    if [ "$ERR_TERMINAL" -eq 1 ]; then
      printf '⏳ %sDownloading Farhelm %s%s\n' "$ERR_BOLD" "$VERSION_NUM" "$ERR_RESET" >&2
      printf '%s   If it looks stuck, it is safe to press Ctrl-C and run the same command again.%s\n\n' "$ERR_DIM" "$ERR_RESET" >&2
    fi
    if [ -n "${FARHELM_INSTALL_SUMS_FILE:-}" ]; then
      SUMS_TEXT=$(cat "$FARHELM_INSTALL_SUMS_FILE")
    else
      sums_url="$BASE_URL/SHA256SUMS"
      sums_reply=$(curl_get -fsSL --max-filesize 1048576 -w "$NEWLINE%{http_code}" "$sums_url" 2>/dev/null || true)
      sums_status=${sums_reply##*"$NEWLINE"}
      SUMS_TEXT=${sums_reply%"$NEWLINE"*}
      if [ "$sums_status" = 404 ]; then
        # No backfill (releases from before get.farhelm.io are not
        # published there) and no fallback to GitHub: a release the origin
        # does not have is refused, in one line, before anything else is
        # downloaded or changed.
        if [ "$LATEST_LOOKUP" -eq 1 ]; then
          error '%s is not published on %s (no %s), though %s/latest names it; retry in a few minutes\n' "$VERSION_TAG" "$ORIGIN" "$sums_url" "$ORIGIN"
        else
          error '%s is not published on %s (no %s); pick a release that is, or leave FARHELM_VERSION unset for the latest\n' "$VERSION_TAG" "$ORIGIN" "$sums_url"
        fi
        exit 1
      elif [ "$sums_status" != 200 ]; then
        error 'download failed (HTTP %s): %s\n' "${sums_status:-000}" "$sums_url"
        exit 1
      fi
    fi

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
    # from here into the installation directly: the locked work copies each
    # file next to its destination first and renames it from there, so the
    # final step is atomic whichever filesystem this directory is on.
    STAGING_DIR=$(mktemp -d "$INSTALL_DIR/.farhelm-install.XXXXXX")
    trap cleanup EXIT
    # Translate the catchable termination signals into a plain `exit`,
    # which runs the EXIT trap above — the same cleanup either way, without
    # running it twice. SIGKILL and power loss cannot be caught by any
    # trap; see cleanup's docstring for what that does and does not leave
    # behind.
    trap 'exit 129' HUP
    trap 'exit 130' INT
    trap 'exit 143' TERM

    printf '%s\n' "$SUMS_TEXT" >"$STAGING_DIR/SHA256SUMS"

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

      # Exactly one SHA256SUMS line must name this archive (the release's
      # signing asserts the same on the publishing side) — awk's
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

    # The locked work is the new release's own farhelm's to do, because a
    # shell cannot hold folders open or take kernel locks: it takes the
    # installation lock (a lock on ~/Applications itself, plus the lock folder
    # older installers know), changes the app only through folders it holds
    # open, writes in an order that leaves a working app wherever it is
    # stopped, prunes old versions nothing still runs from, and places the
    # Terminal link (farhelm_supervisor::installation::install). This script
    # writes nothing inside the app; it stages what that program installs,
    # hands over, and reads the outcome from two files it leaves behind.
    (umask 022; mkdir -p "$app_parent") || { error 'creating %s failed\n' "$app_parent"; exit 1; }

    # The record names the Terminal link this installation owns, by the
    # canonical path of its directory. The sentinel keeps a trailing newline
    # in the path through command substitution, and CDPATH cannot redirect
    # the cd or make it print.
    canonical_bin=$(
      CDPATH='' cd -P "$INSTALL_DIR" || exit 1
      pwd -P || exit 1
      printf '%s' '__FARHELM_CANONICAL_PATH_END__'
    ) || { error 'resolving %s failed\n' "$INSTALL_DIR"; exit 1; }
    canonical_bin=${canonical_bin%__FARHELM_CANONICAL_PATH_END__}
    canonical_bin=${canonical_bin%"$NEWLINE"}

    write_forwarder "$STAGING_DIR/forwarder" || { error 'writing the forwarder failed\n'; exit 1; }
    write_info_plist "$STAGING_DIR/Info.plist" || { error 'writing Info.plist failed\n'; exit 1; }

    # Hand-over arguments. An app from before the side-by-side layout is only
    # replaced when this script can show it built it (the old layout's record
    # naming this bin directory, or the recordless shape from before records);
    # the locked work checks again that it has no layout of its own. A run
    # with a checksum file is an older app's updater, which may only update
    # (design, section 7). Running records keep their versions from pruning.
    set -- --staging "$STAGING_DIR" --version "$VERSION_NUM" --applications "$app_parent" \
      --bin-dir "$INSTALL_DIR" --link "$canonical_bin/farhelm" \
      --running-record "$HOME/.local/state/farhelm/running-version"
    case "${XDG_STATE_HOME:-}" in
      /*) set -- "$@" --running-record "$XDG_STATE_HOME/farhelm/running-version" ;;
    esac
    if [ -n "${FARHELM_INSTALL_SUMS_FILE:-}" ]; then
      set -- "$@" --update-only
    fi
    # An app an older installer built without this layout's record may be
    # replaced whole. The check runs here, before the lock, so the approval
    # names the folder it was made for by its inode, read before and after
    # the check: the locked work replaces only that folder, and refuses one
    # put at the place in the meantime.
    if [ -d "$app_path" ] && [ ! -L "$app_path" ]; then
      legacy_inode=$(folder_inode "$app_path")
      if [ -n "$legacy_inode" ] &&
        { bundle_record_is_ours "$app_path" "$canonical_bin" || is_legacy_installer_bundle "$app_path"; } &&
        [ "$(folder_inode "$app_path")" = "$legacy_inode" ]; then
        set -- "$@" --replace-legacy-bundle "$legacy_inode"
      fi
    fi
    install_rc=0
    "$STAGING_DIR/farhelm" internal install "$@" </dev/null || install_rc=$?
    if [ -f "$STAGING_DIR/install-failure" ]; then
      error '%s\n' "$(sed -n 1p "$STAGING_DIR/install-failure")"
      sed '1d' "$STAGING_DIR/install-failure" >&2
      exit 1
    fi
    if [ "$install_rc" -ne 0 ] || [ ! -f "$STAGING_DIR/install-result" ]; then
      if [ "$install_rc" -eq 2 ]; then
        # A farhelm from before this installer rejects the subcommand.
        error 'Farhelm %s is older than this installer and cannot be installed by it; nothing was changed.\n' "$VERSION_NUM"
        printf '%s\n' '   Pick a newer release, or leave FARHELM_VERSION unset for the latest release.' >&2
      else
        error 'installing %s failed: the new farhelm stopped (status %s) before it reported an outcome\n' "$app_path" "$install_rc"
        printf 'Re-run the installer to retry.\n' >&2
      fi
      exit 1
    fi
    updated_installation=0
    if [ "$(sed -n 1p "$STAGING_DIR/install-result")" = updated ]; then
      updated_installation=1
    fi
    KEPT_NOTES=$(sed '1d' "$STAGING_DIR/install-result")

    # Spotlight and Launch Services keep showing the old version until the
    # bundle is touched or re-registered (observed on a real Mac). The
    # binary does not exist on the Linux CI host the installer tests run
    # on, hence the -x guard. The installation lock is released by now, so
    # an uninstall may already have removed the app: -c keeps touch from
    # creating an empty file in its place.
    touch -c "$app_path" 2>/dev/null || true
    LSREGISTER="/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"
    if [ -x "$LSREGISTER" ]; then
      "$LSREGISTER" -f "$app_path" >/dev/null 2>&1 || true
    fi

    # 8. Report. Choose the action from this run's outcome and the local
    # tmux prerequisite. Fresh-install launch advice follows its remedy. An
    # update keeps the approved restart/session-survival line and also ends
    # with the prescribed reminder after tmux advice. Uninstall stays visible.
    # Fresh installs invite feedback before the kept-file and uninstall notices.
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
      # Updates also run unattended inside the app; invite feedback only when
      # someone first installs it, before the kept-file and uninstall notices.
      printf "\n💬 I'd love to hear what you think, even a quick throwaway\n"
      printf '   comment. In Farhelm, click ? at the top of the sidebar and\n'
      printf '   choose Send feedback; it comes privately to me, the maintainer.\n'
      printf "   If you'd rather discuss it in the open, file a GitHub issue\n"
      printf '   at %shttps://github.com/scode/farhelm/issues%s instead.\n' "$OUT_CYAN" "$OUT_RESET"
    fi
    if [ -n "$KEPT_NOTES" ]; then
      printf '\n%s\n' "$KEPT_NOTES"
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
