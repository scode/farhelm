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
  # The one supported way to install farhelm: detects your platform,
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

  # ---------------------------------------------------------------------
  # The install transaction, in one place
  #
  # Replacing binaries has to survive being interrupted at any instant,
  # including instants no trap handler ever sees (SIGKILL, power loss), so
  # the replacement phase writes a RECOVERY JOURNAL before each move and a
  # later run finishes what an interrupted one started. Three properties
  # shape how that journal is stored and spelled; each one is a bug this
  # script has already had.
  #
  # It lives INSIDE the lock directory, at $LOCK_DIR/journal. The lock
  # directory is the one path here created by an exclusive `mkdir`, so a
  # journal underneath it cannot be a pathname someone else got to first —
  # whereas a journal beside the binaries, at a predictable name nothing
  # validated, could be pre-planted as a symlink and turn this script's
  # own `>>` into an append to a file it never meant to touch. Storing the
  # two together also makes their lifetimes one thing: the lock is only
  # ever removed once the journal is gone (see remove_owned_lock), which
  # is what "recovery state is preserved for the next run" reduces to.
  #
  # A record NAMES NO PATHS. The vocabulary is exactly `PARK cli`,
  # `PARK desktop`, `INSTALL cli`, `INSTALL desktop`, and `UNDONE <n>`;
  # every path rollback touches is derived at recovery time from
  # $INSTALL_DIR plus the two fixed binary names. That keeps a legal but
  # awkward FARHELM_INSTALL_DIR (one containing the field separator, or a
  # newline, both of which a pathname may legally contain) from splitting
  # into extra fields or extra apparent records, and it bounds what a
  # journal can ever ask for: at most `farhelm`, `farhelm-desktop`, their
  # `.old` backups, and nothing else, inside the install directory the
  # caller selected. Anything outside the vocabulary makes rollback refuse
  # rather than guess.
  #
  # Rollback is REPLAY-SAFE. Undo steps are not idempotent as a pair — an
  # `INSTALL` undo removes the destination and the `PARK` undo before it
  # puts the old binary back at that same destination — so a second pass
  # over an unchanged journal would delete exactly what the first pass
  # restored. Each completed undo step is therefore recorded as
  # `UNDONE <n>`, naming the 1-based line number of the record it
  # finished, and any later pass skips those. Line numbers stay valid
  # because appending is the only write the journal ever takes.
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

  # D15's version shape: X.Y.Z or vX.Y.Z, with an optional -rc.N prerelease
  # suffix; no leading zeros anywhere. See normalize_version below for why a
  # plain grep -E against this is not, by itself, enough.
  VERSION_PATTERN='^v?(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-rc\.(0|[1-9][0-9]*))?$'

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
  # (optionally "-rc.N") tag, or returns failure and prints nothing.
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

  # Emits $1 as one single-quoted POSIX shell word (embedded single quotes
  # escaped the standard '\'' way), so a printed `export PATH=...` line stays
  # safe to paste even when the install directory contains spaces or shell
  # metacharacters. Returns failure if $1 contains a newline: no single-line
  # quoting can represent that safely, and the caller falls back to non-
  # pasteable guidance instead.
  shquote() {
    case "$1" in
      *"$NEWLINE"*) return 1 ;;
    esac
    printf "'%s'" "$(printf '%s' "$1" | sed "s/'/'\\\\''/g")"
  }

  # Every network request in this script goes through here. `-q` disables
  # the user's ~/.curlrc — it only takes effect as curl's very first
  # argument, which is why it is hardcoded ahead of anything the caller
  # passes, since a stray .curlrc could otherwise silently change redirect
  # or output behavior. The timeouts bound how long an unreachable or
  # stalled release host can delay failure: a little over 10 minutes per
  # request, from --max-time 600.
  curl_get() {
    curl -q --connect-timeout 15 --max-time 600 "$@"
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
      printf '%s has %s members named %s, expected exactly 1\n' "$esm_label" "$esm_hits" "$esm_base" >&2
      exit 1
    fi
    esm_member=$(tar tzf "$esm_archive" | awk -F/ -v b="$esm_base" '{n=split($0,p,"/"); if (p[n]==b) print}')

    case "$esm_member" in
      -*)
        printf '%s: member name %s looks like a tar option; refusing\n' "$esm_label" "$esm_member" >&2
        exit 1
        ;;
    esac
    if printf '%s' "$esm_member" | LC_ALL=C grep -q '[[:cntrl:]]'; then
      printf '%s: member name contains a control character; refusing\n' "$esm_label" >&2
      exit 1
    fi

    esm_type_lines=$(tar tvzf "$esm_archive" -- "$esm_member" 2>/dev/null)
    esm_type_count=$(printf '%s\n' "$esm_type_lines" | grep -c .)
    if [ "$esm_type_count" -ne 1 ]; then
      printf '%s: %s reports %s metadata records for %s, expected exactly 1\n' "$esm_label" "tar tv" "$esm_type_count" "$esm_member" >&2
      exit 1
    fi
    esm_type_char=$(printf '%s' "$esm_type_lines" | cut -c1)
    if [ "$esm_type_char" != "-" ]; then
      printf '%s: %s is not a regular file (tar reports type '"'"'%s'"'"'); refusing to install it\n' "$esm_label" "$esm_member" "$esm_type_char" >&2
      exit 1
    fi

    tar -xOzf "$esm_archive" -- "$esm_member" >"$esm_dest"
    chmod "$esm_mode" "$esm_dest"
  }

  # Refuses (exit 1) unless PATH is either absent or a plain regular file.
  # install.sh only ever creates and moves plain regular files at the paths
  # it owns, so anything else there — a directory, a device, a symlink
  # (dangling or not, hence the explicit -L check: -e alone is silently
  # false for a dangling symlink and would let one slip through as
  # "absent") — is either a user collision or a way a later `mv` could be
  # redirected outside the install directory, and must stop the whole run
  # before any mutation happens.
  # NOTE on variable names throughout this file's helper functions: POSIX sh
  # has no `local` — every assignment here is a GLOBAL, visible to (and
  # overwritable by) any caller. Every helper below therefore uses names
  # prefixed by its own initials rather than a generic name like `dest` or
  # `path` a caller might also be using; an earlier revision of this script
  # shipped exactly that bug (`journal_append`'s `dest` silently clobbering
  # the replace loop's own `dest` mid-transaction) before it was caught.
  refuse_unless_absent_or_regular() {
    rufu_target=$1
    if [ -L "$rufu_target" ] || { [ -e "$rufu_target" ] && [ ! -f "$rufu_target" ]; }; then
      printf '%s exists and is not a regular file (or is a symlink); refusing to touch it\n' "$rufu_target" >&2
      exit 1
    fi
  }

  # Refuses (exit 1) unless PATH is completely absent. Used only for the
  # reserved backup paths (.farhelm.old, .farhelm-desktop.old) at the START
  # of a FRESH transaction: acquire_lock already resolves any recovery state
  # a previous crashed run left behind before a new transaction is allowed
  # to begin, so seeing anything at these paths here means something else
  # (a user file, a race, external interference) put it there, and this
  # script must not silently overwrite what might be someone's data or an
  # unrecovered backup.
  refuse_unless_absent() {
    rua_target=$1
    if [ -e "$rua_target" ] || [ -L "$rua_target" ]; then
      printf '%s already exists; refusing to start a new install/update while it does (if a previous run left recovery state behind, re-run this script once more first to let it finish repairing)\n' "$rua_target" >&2
      exit 1
    fi
  }

  # True iff DIR has one of the only three shapes this script's own lock
  # ever takes: empty (the brief window right after `mkdir` but before the
  # pid file is written), just "pid", or "pid" plus the recovery "journal"
  # a transaction in its replacement phase has created. Anything else —
  # unrelated files, subdirectories, a wrongly-named entry — means DIR is
  # not our lock, and every caller must refuse to touch it rather than
  # guess.
  #
  # Both the listing and the per-name file tests are load-bearing. The
  # listing is what catches an EXTRA entry; the `-f` tests are what stop a
  # single crafted entry whose name embeds a newline from impersonating
  # the two-line listing a real lock produces.
  is_our_lock() {
    [ -d "$1" ] || return 1
    iol_entries=$(ls -A "$1" 2>/dev/null)
    case "$iol_entries" in
      '') return 0 ;;
      'pid') [ -f "$1/pid" ] ;;
      "journal${NEWLINE}pid") [ -f "$1/pid" ] && [ -f "$1/journal" ] ;;
      *) return 1 ;;
    esac
  }

  # Removes $LOCK_DIR, but only when it is safe to: the journal must
  # already be gone, and the directory must still be shaped like our own
  # lock. Never a blind `rm -rf` (F6) — a validated `rmdir` on an emptied,
  # known-shape directory cannot silently destroy something else that
  # happens to occupy the reserved pathname.
  #
  # The journal check makes "unfinished recovery state survives" an
  # invariant of this one function rather than a rule every call site has
  # to remember: releasing the lock while a journal still records
  # un-undone moves would strand the next run with wreckage it is no
  # longer allowed to touch. Callers that legitimately finish a
  # transaction (commit, or a rollback that fully succeeded) remove the
  # journal first, and only then does this release the slot.
  remove_owned_lock() {
    if [ -e "$LOCK_DIR/journal" ]; then
      return 0
    fi
    if is_our_lock "$LOCK_DIR"; then
      rm -f "$LOCK_DIR/pid"
      rmdir "$LOCK_DIR" 2>/dev/null || true
    fi
  }

  # Appends one record describing a move about to be attempted, creating
  # $JOURNAL on the first call of a transaction. Called immediately BEFORE
  # the move it describes, so that if the move (or anything else) is
  # interrupted right after, the journal already contains a record telling
  # a later rollback that this specific move needs reversing.
  #
  # TYPE is "PARK" (an existing destination being moved aside to its
  # backup path) or "INSTALL" (a staged binary being moved into its
  # destination); WHICH is "cli" or "desktop", naming which binary — never
  # a path, for the reasons in the transaction section above.
  #
  # The umask dance gives the journal owner-only permissions without
  # disturbing the mode of anything else this run creates: it is recovery
  # instructions this script will later act on, so nobody else's account
  # gets to append to it, and the window between "exists" and "is
  # private" that a create-then-chmod would open is closed by not
  # existing.
  journal_append() {
    ja_type=$1
    ja_which=$2
    if [ ! -e "$JOURNAL" ]; then
      ja_prior_umask=$(umask)
      umask 077
      : >"$JOURNAL"
      umask "$ja_prior_umask"
    fi
    printf '%s %s\n' "$ja_type" "$ja_which" >>"$JOURNAL"
  }

  # Records that the undo of the record at 1-based line N is COMPLETE, so
  # no later pass repeats it. Appending is deliberately the only write the
  # journal ever takes: rewriting it in place would need a scratch file and
  # a rename to be crash-safe, and would invalidate the line numbers these
  # markers refer to.
  journal_mark_undone() {
    printf 'UNDONE %s\n' "$1" >>"$JOURNAL"
  }

  # Undoes every not-yet-undone move recorded in $JOURNAL, in REVERSE
  # order, restoring $INSTALL_DIR to its pre-transaction state regardless
  # of exactly how far the transaction got before it was interrupted (F2,
  # F3): a crash after only the first PARK, after a PARK+INSTALL pair, or
  # after both binaries' PARK moves but before the second INSTALL, all
  # correctly unwind to "both binaries are back to what they were before
  # this run started" — because undoing strictly in reverse naturally
  # composes: undoing a later INSTALL first clears the way for undoing the
  # PARK that preceded it.
  #
  # Every individual undo step is written to be a safe no-op if the move it
  # describes never actually completed (e.g. the journal record was written
  # but the process died before the `mv` itself ran) — existence-guarded, so
  # a conservative "journal it before attempting" write policy never causes
  # rollback to consume or clobber something that was never touched.
  #
  # Safe to call again after a partial pass, and it WILL be: an explicit
  # rollback that fails exits, and the EXIT handler immediately replays.
  # Each finished step is marked `UNDONE <n>` before the next is attempted,
  # and a replay skips those — without which the replay's `INSTALL` undo
  # would delete the very binary the first pass's `PARK` undo had just put
  # back, and then report success.
  #
  # Its own scaffolding gets the same treatment as the moves it performs.
  # Reading the journal, refusing a record it does not recognise, and
  # recording progress are each checked, because a rollback that quietly
  # skipped work and returned success would be worse than one that failed:
  # callers read success as "the old installation is back" and delete the
  # lock and journal on the strength of it. Nothing here needs a scratch
  # file — the journal is read once into a variable and walked twice — so
  # there is no temporary to create, reverse into, reopen, or remove, and
  # no failure of any of those to mistake for having nothing to do.
  #
  # Returns success only if EVERY recorded move was fully undone. On
  # partial failure (most plausibly: the original destination has since
  # become a directory or other non-regular path, so the restore has
  # nowhere safe to land) it prints exactly what could not be restored and
  # returns failure; the caller is responsible for leaving the lock and
  # journal in place rather than declaring victory (F8).
  rollback_from_journal() {
    if ! rfj_content=$(cat "$JOURNAL" 2>/dev/null); then
      printf 'could not read the recovery journal %s; nothing was rolled back and %s is LEFT IN PLACE for inspection\n' "$JOURNAL" "$LOCK_DIR" >&2
      return 1
    fi

    # First pass: turn the journal into a REVERSED list of pending undo
    # steps (each "TYPE:WHICH:LINENO", prepended so the list comes out
    # newest-first) plus the set of line numbers already marked done. One
    # pass suffices for both because an `UNDONE <n>` marker is always
    # appended after the record it refers to.
    rfj_lineno=0
    rfj_pending=""
    rfj_undone=" "
    rfj_bad=""
    while IFS= read -r rfj_line; do
      rfj_lineno=$((rfj_lineno + 1))
      [ -n "$rfj_line" ] || continue
      case "$rfj_line" in
        "PARK cli" | "PARK desktop" | "INSTALL cli" | "INSTALL desktop")
          rfj_pending="${rfj_line%% *}:${rfj_line#* }:$rfj_lineno $rfj_pending"
          ;;
        "UNDONE "*)
          rfj_marked=${rfj_line#UNDONE }
          case "$rfj_marked" in
            '' | *[!0-9]*) rfj_bad=$rfj_line ;;
            *) rfj_undone="$rfj_undone$rfj_marked " ;;
          esac
          ;;
        *) rfj_bad=$rfj_line ;;
      esac
      [ -z "$rfj_bad" ] || break
    done <<EOF
$rfj_content
EOF

    # An unrecognized record is not something to skip past: this format has
    # exactly one writer, so anything else means the journal is not the one
    # this script wrote, and acting on part of it would be acting on a
    # stranger's instructions. Refuse, and change nothing.
    if [ -n "$rfj_bad" ]; then
      printf 'the recovery journal %s contains a record this installer does not recognise; refusing to act on it -- %s and its journal are LEFT IN PLACE for inspection\n' "$JOURNAL" "$LOCK_DIR" >&2
      return 1
    fi

    rfj_ok=1
    for rfj_step in $rfj_pending; do
      rfj_type=${rfj_step%%:*}
      rfj_tail=${rfj_step#*:}
      rfj_which=${rfj_tail%%:*}
      rfj_at=${rfj_tail##*:}
      case "$rfj_undone" in
        *" $rfj_at "*) continue ;;
      esac

      # Every path this function touches is derived here, from
      # $INSTALL_DIR and a fixed binary name — never read out of the
      # journal.
      if [ "$rfj_which" = cli ]; then
        rfj_name=farhelm
      else
        rfj_name=farhelm-desktop
      fi
      rfj_dest="$INSTALL_DIR/$rfj_name"
      rfj_backup="$INSTALL_DIR/.$rfj_name.old"

      rfj_did=0
      case "$rfj_type" in
        INSTALL)
          # The move was staged->dest; undoing it just means dest must stop
          # holding the new file. There is nowhere meaningful to move it
          # BACK to (its staging source may no longer exist at all, from a
          # different process's ephemeral $STAGING_DIR), so removal is the
          # correct and sufficient undo — any prior PARK for the same
          # binary, undone next in this reverse pass, is what restores the
          # real content.
          if rm -f "$rfj_dest"; then
            rfj_did=1
          else
            rfj_ok=0
            printf 'could not remove %s while rolling back; manual cleanup needed\n' "$rfj_dest" >&2
          fi
          ;;
        PARK)
          # The move was dest->backup. Undo only if the backup still
          # exists (a no-op otherwise: either this PARK never actually
          # completed, or a previous pass already restored it and died
          # before it could say so).
          if [ ! -e "$rfj_backup" ]; then
            rfj_did=1
          elif [ -L "$rfj_dest" ] || { [ -e "$rfj_dest" ] && [ ! -f "$rfj_dest" ]; }; then
            rfj_ok=0
            printf 'cannot restore %s to %s: %s exists and is not a regular file; leaving %s in place for manual recovery\n' "$rfj_backup" "$rfj_dest" "$rfj_dest" "$rfj_backup" >&2
          elif mv "$rfj_backup" "$rfj_dest"; then
            rfj_did=1
          else
            rfj_ok=0
            printf 'could not restore %s to %s while rolling back\n' "$rfj_backup" "$rfj_dest" >&2
          fi
          ;;
      esac

      if [ "$rfj_did" -eq 1 ]; then
        # Losing the ability to record progress is fatal to the whole
        # pass, not just to this step: every step after it would be
        # unrepeatable-but-unmarked, which is exactly the state a replay
        # cannot tell apart from "never done".
        if ! journal_mark_undone "$rfj_at"; then
          rfj_ok=0
          printf 'could not record rollback progress in %s; stopping before a later attempt could undo an already-restored binary\n' "$JOURNAL" >&2
          break
        fi
      fi
    done
    [ "$rfj_ok" -eq 1 ]
  }

  # Acquires the install-directory lock ($LOCK_DIR, a plain `mkdir` — atomic
  # create-if-absent on every POSIX filesystem) so two installers cannot
  # interleave their writes to the same $INSTALL_DIR.
  #
  # A lock that already exists is one of three things, distinguished in
  # order: (1) shaped like something OTHER than our own lock (F6) — refuse
  # outright, touch nothing; (2) shaped like ours but with no readable
  # single positive-integer pid yet — treated as OCCUPIED, not stale (F5):
  # the pid write happens just after `mkdir`, not atomically with it, so a
  # lock with no readable pid might belong to a brand-new LIVE owner this
  # process simply raced against, and guessing "stale" wrongly (deleting a
  # live lock) is far worse than guessing "live" wrongly (one retryable
  # refusal); (3) a readable pid that `kill -0` finds alive — genuinely
  # live, refuse and ask to wait. Only past all three checks is a lock
  # treated as stale wreckage from a crash, at which point any recorded
  # transaction journal is rolled back (restoring the prior installation)
  # before the slot is reused.
  #
  # Known limitation: `kill -0` can be fooled by pid reuse on a long-uptime
  # machine (a stale lock's pid happening to be reassigned to something else
  # by the time this runs). That is a narrow race on top of an already-rare
  # crash-at-exactly-the-wrong-instant, and is not solved here. A lock stuck
  # in state (2) forever (crashed between `mkdir` and the pid write) also has
  # no automatic recovery, by the same safety-over-self-healing tradeoff;
  # the refusal message says so.
  acquire_lock() {
    if mkdir "$LOCK_DIR" 2>/dev/null; then
      printf '%s\n' "$$" >"$LOCK_DIR/pid"
      LOCK_ACQUIRED=1
      return 0
    fi

    if ! is_our_lock "$LOCK_DIR"; then
      printf '%s exists and is not a farhelm install lock (unexpected contents); refusing to touch it -- remove it by hand if you are sure nothing owns it, then retry\n' "$LOCK_DIR" >&2
      exit 1
    fi

    other_pid=$(cat "$LOCK_DIR/pid" 2>/dev/null || true)
    case "$other_pid" in
      '' | *[!0-9]*)
        printf 'another farhelm install/update appears to be starting against %s (its pid is not readable yet); wait a moment and retry -- if this persists, a previous run may have crashed between creating the lock and recording its pid, which requires removing %s by hand\n' "$INSTALL_DIR" "$LOCK_DIR" >&2
        exit 1
        ;;
    esac
    if kill -0 "$other_pid" 2>/dev/null; then
      printf 'another farhelm install/update (pid %s) is already running against %s; wait for it to finish, then retry\n' "$other_pid" "$INSTALL_DIR" >&2
      exit 1
    fi

    if [ -e "$JOURNAL" ]; then
      if rollback_from_journal; then
        rm -f "$JOURNAL"
        remove_owned_lock
        printf 'recovered from an interrupted install/update (stale lock, pid %s no longer running): restored the previous installation; re-run this script to retry\n' "$other_pid" >&2
      else
        printf 'found an interrupted install/update (stale lock, pid %s no longer running) and could not fully roll it back; %s and %s are LEFT IN PLACE for inspection -- see the lines above for what could not be restored, then re-run this script\n' "$other_pid" "$LOCK_DIR" "$JOURNAL" >&2
      fi
      exit 1
    fi

    # No journal: whatever crashed did so before any replacement mutation
    # began (still downloading/verifying, or between acquiring the lock and
    # the first move). Nothing to roll back -- clear the stale lock and
    # continue within THIS invocation; there is nothing to report beyond
    # "the slot was free".
    remove_owned_lock
    mkdir "$LOCK_DIR"
    printf '%s\n' "$$" >"$LOCK_DIR/pid"
    LOCK_ACQUIRED=1
  }

  # The EXIT/INT/TERM/HUP handler. Always removes the ephemeral staging
  # directory. If this process holds the lock, it also checks for an
  # in-progress transaction journal: if one exists, the replacement never
  # reached its commit point (see main's "Committed:" comment), so cleanup
  # rolls it back BEFORE removing anything else (F3) — this is what makes a
  # signal or an unhandled failure anywhere during replacement behave the
  # same as an explicitly handled one, rather than silently deleting the
  # only record of what needs restoring. Only once rollback succeeds (or
  # there was no journal to begin with) does the lock itself get removed;
  # a rollback that cannot fully complete leaves the lock and journal in
  # place on purpose, for the next run (or a human) to find.
  #
  # What this does NOT do: survive SIGKILL or a power loss. Both skip trap
  # handlers entirely, so a `.farhelm-install.*` staging directory can be
  # left behind by one of those; it is inert and safe to delete by hand.
  # The lock and journal are designed to survive that instead of relying on
  # a trap: see acquire_lock's stale-recovery path, which performs exactly
  # this same rollback from the NEXT invocation when a trap never got the
  # chance to run at all.
  cleanup() {
    rm -rf "$STAGING_DIR" 2>/dev/null || true
    if [ "${LOCK_ACQUIRED:-0}" -eq 1 ]; then
      if [ -e "$JOURNAL" ]; then
        if rollback_from_journal; then
          rm -f "$JOURNAL"
          remove_owned_lock
        else
          printf 'interrupted while updating; automatic rollback could not fully complete -- %s and %s are LEFT IN PLACE for inspection; re-run this script once you have looked, or ask for help\n' "$LOCK_DIR" "$JOURNAL" >&2
        fi
      else
        remove_owned_lock
      fi
    fi
  }

  main() {
    # 1. Platform.
    #
    # Only the three targets a release actually ships (D4) are recognized;
    # everything else — Windows, real Intel Macs, 32-bit anything — is out
    # of scope rather than guessed at. Apple-silicon Macs running under
    # Rosetta report `x86_64` from `uname -m` even though the hardware (and
    # therefore this script) can run the native arm64 build directly, so
    # that combination gets one extra check instead of being rejected
    # outright.
    os=$(uname -s)
    machine=$(uname -m)
    case "$os $machine" in
      "Linux x86_64") TARGET=x86_64-unknown-linux-musl ;;
      "Linux aarch64") TARGET=aarch64-unknown-linux-musl ;;
      "Darwin arm64") TARGET=aarch64-apple-darwin ;;
      "Darwin x86_64")
        if [ "$(sysctl -n hw.optional.arm64 2>/dev/null || true)" = "1" ]; then
          TARGET=aarch64-apple-darwin
        else
          printf 'farhelm has no release build for %s %s; see %s\n' "$os" "$machine" "$RELEASES_PAGE" >&2
          exit 1
        fi
        ;;
      *)
        printf 'farhelm has no release build for %s %s; see %s\n' "$os" "$machine" "$RELEASES_PAGE" >&2
        exit 1
        ;;
    esac
    # Whether this platform's row(s) in $ASSET_TABLE include farhelm-desktop
    # — a property of TARGET alone, decided here once and used for every
    # later decision that depends on it, instead of re-deriving it from
    # filesystem state (staged files, or what happens to already exist at
    # the destination) after the fact.
    HAS_DESKTOP=0
    [ "$TARGET" = "aarch64-apple-darwin" ] && HAS_DESKTOP=1

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
      printf 'farhelm'"'"'s installer needs:%s\n' "$missing" >&2
      exit 1
    fi

    # 3. Version.
    #
    # FARHELM_VERSION pins a release, including a -rc.N prerelease (D15);
    # otherwise the script asks GitHub which tag "latest" currently means.
    # Either way, exactly one candidate string and one error message are
    # produced here, and normalize_version validates and normalizes that
    # single candidate the same way regardless of where it came from.
    if [ -n "${FARHELM_VERSION:-}" ]; then
      candidate=$FARHELM_VERSION
      version_error="FARHELM_VERSION='$FARHELM_VERSION' is not X.Y.Z, vX.Y.Z, or a -rc.N prerelease of one"
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
    if ! VERSION_TAG=$(normalize_version "$candidate"); then
      printf '%s\n' "$version_error" >&2
      exit 1
    fi
    VERSION_NUM=${VERSION_TAG#v}

    # FARHELM_RELEASE_BASE_URL is deliberately undocumented in the README:
    # it exists so this script's own tests (and nothing else) can point it
    # at a fixture server instead of github.com. A real install always
    # uses the release's normal download URL.
    BASE_URL=${FARHELM_RELEASE_BASE_URL:-$DOWNLOAD_PREFIX/$VERSION_TAG}
    BASE_URL=${BASE_URL%/}

    INSTALL_DIR=${FARHELM_INSTALL_DIR:-$HOME/.local/bin}
    # Mask group/world write bits on any directory COMPONENT this specific
    # call creates (umask 000 would otherwise leave a brand-new directory
    # mode 0777, which would undermine the installed binaries' own
    # hardcoded 0755 below — another account could just replace the
    # directory entry). A directory that already existed is left exactly
    # as it was: this is not a general permission-hardening pass over
    # someone's chosen install location, only over what this run itself
    # creates.
    install_dir_existed=1
    [ -d "$INSTALL_DIR" ] || install_dir_existed=0
    mkdir -p "$INSTALL_DIR"
    if [ "$install_dir_existed" -eq 0 ]; then
      chmod 0755 "$INSTALL_DIR"
    fi

    # 4. Stage everything in a scratch directory on the same filesystem as
    # the destination, so the "put the binary in place" step below is a
    # same-filesystem mv (atomic, no half-written binary a concurrent
    # launch could exec) rather than a copy.
    STAGING_DIR=$(mktemp -d "$INSTALL_DIR/.farhelm-install.XXXXXX")
    LOCK_DIR="$INSTALL_DIR/.farhelm-install.lock"
    # Both are needed before acquire_lock (which rolls back a previous
    # run's journal) and before the EXIT trap below can fire, so they are
    # settled here rather than at the replacement phase that uses them.
    JOURNAL="$LOCK_DIR/journal"
    LOCK_ACQUIRED=0
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
    sums_url="$BASE_URL/SHA256SUMS"
    sums_status=$(curl_get -fsSL -w '%{http_code}' -o "$STAGING_DIR/SHA256SUMS" "$sums_url" 2>/dev/null || true)
    if [ "$sums_status" = 404 ]; then
      # D17: a 404 cannot tell "no such release" from "still publishing"
      # apart, so both this script and the helm say exactly the same
      # thing rather than guessing. Deviation from D17's own wording: the
      # helm's version of this message ends "...or pass --payload-dir", a
      # flag this script does not have; here it ends by pointing at the
      # releases page instead.
      printf 'no SHA256SUMS for %s at %s (HTTP 404): the release is not published or is still publishing; retry in a few minutes, or check %s\n' "$VERSION_TAG" "$BASE_URL" "$RELEASES_PAGE" >&2
      exit 1
    elif [ "$sums_status" != 200 ]; then
      printf 'download failed (HTTP %s): %s\n' "$sums_status" "$sums_url" >&2
      exit 1
    fi

    # 5. Download, verify, and unpack exactly the archives this platform
    # needs — one row for Linux, two (farhelm and farhelm-desktop) for
    # macOS. Reading $ASSET_TABLE through a heredoc rather than a pipe is
    # deliberate: a `command | while read` loop runs the loop body in a
    # subshell under POSIX sh, and every variable this loop sets needs to
    # outlive it.
    #
    # ICNS_STATE is one of those outliving variables: `staged` once the
    # desktop archive yielded a Farhelm.icns, `absent` otherwise (Linux
    # runs, and macOS installs of releases that predate the icon). The
    # bundle step after the commit reads it.
    ICNS_STATE=absent
    while IFS='|' read -r row_target row_archive row_binary; do
      [ "$row_target" = "$TARGET" ] || continue

      archive_path="$STAGING_DIR/$row_archive"
      curl_get -fsSL --retry 3 -o "$archive_path" "$BASE_URL/$row_archive"

      # Exactly one SHA256SUMS line must name this archive (D3's
      # "sign-sums" job asserts the same on the publishing side) — awk's
      # exact field comparison, not a substring grep, so one archive's
      # name being a prefix of another's can never cross-match.
      sums_hits=$(awk -v f="$row_archive" '$2==f{c++} END{print c+0}' "$STAGING_DIR/SHA256SUMS")
      if [ "$sums_hits" -ne 1 ]; then
        printf 'SHA256SUMS has %s entries for %s, expected exactly 1\n' "$sums_hits" "$row_archive" >&2
        exit 1
      fi
      expected_sha256=$(awk -v f="$row_archive" '$2==f{print $1; exit}' "$STAGING_DIR/SHA256SUMS")
      if ! actual_sha256=$(sha256_of "$archive_path"); then
        printf '%s: could not compute a checksum (%s failed reading %s)\n' "$row_archive" "$CHECKSUM_TOOL" "$archive_path" >&2
        exit 1
      fi
      if [ "$actual_sha256" != "$expected_sha256" ]; then
        printf '%s: checksum mismatch (expected %s, got %s)\n' "$row_archive" "$expected_sha256" "$actual_sha256" >&2
        exit 1
      fi

      # Selection, refusal rules, and -O streaming all live in
      # extract_sole_member; see its own comment for why each refusal
      # exists.
      extract_sole_member "$archive_path" "$row_archive" "$row_binary" "$STAGING_DIR/$row_binary" 0755

      # The desktop archive also carries the app icon the bundle step below
      # builds Farhelm.app around. Releases published before the icon
      # existed do not have it, and this script installs pinned old
      # versions too — so ABSENCE is a skip the closing report explains,
      # not an error, while a PRESENT icon gets the same extraction
      # discipline as a binary. More than one match is the one shape that
      # is never legitimate.
      if [ "$row_binary" = "farhelm-desktop" ]; then
        icns_hits=$(member_count_named "$archive_path" "Farhelm.icns")
        if [ "$icns_hits" -gt 1 ]; then
          printf '%s has %s members named Farhelm.icns, expected at most 1\n' "$row_archive" "$icns_hits" >&2
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
      printf 'internal error: no farhelm archive matched target %s\n' "$TARGET" >&2
      exit 1
    fi

    # Belt-and-braces beyond the checksum: prove the binary we just staged
    # actually runs and claims to be the version we asked for, before it
    # ever touches the real install directory.
    reported_version=$("$STAGING_DIR/farhelm" --version)
    expected_version_line="farhelm $VERSION_NUM"
    if [ "$reported_version" != "$expected_version_line" ]; then
      printf 'downloaded farhelm reports '"'"'%s'"'"', expected '"'"'%s'"'"'; refusing to install\n' "$reported_version" "$expected_version_line" >&2
      exit 1
    fi

    # 6. Replace. Both binaries this run needs are fully staged and
    # verified above; nothing past this point downloads anything.
    # Everything from here is local filesystem work, guarded by a lock so
    # two installers cannot interleave their writes to the same
    # $INSTALL_DIR, and journaled move-by-move so that ANY failure — ours,
    # or one this process never gets to react to (a signal, an unrelated
    # unhandled error) — leaves either the complete old pair or the
    # complete new pair in place, never a partial mix.
    acquire_lock

    binaries="farhelm"
    [ "$HAS_DESKTOP" -eq 1 ] && binaries="farhelm farhelm-desktop"

    # Refuse before touching anything if a public destination is neither
    # absent nor a regular file, or if either reserved backup path already
    # has something at it (a fresh transaction must never find one:
    # acquire_lock already resolved any recovery state a previous crash
    # left behind, so anything here now is a collision this script must
    # not overwrite).
    for name in $binaries; do
      refuse_unless_absent_or_regular "$INSTALL_DIR/$name"
      refuse_unless_absent "$INSTALL_DIR/.$name.old"
    done

    replaced_something=0

    for name in $binaries; do
      # The journal's own name for this binary (see the transaction
      # section): records carry this, never $dest or $backup.
      if [ "$name" = farhelm ]; then
        binary_id=cli
        fail_msg="install failed while replacing farhelm; the previous farhelm (if any) was restored"
      else
        binary_id=desktop
        fail_msg="update failed while replacing farhelm-desktop; the previous farhelm was restored"
      fi

      dest="$INSTALL_DIR/$name"
      backup="$INSTALL_DIR/.$name.old"

      if [ -e "$dest" ]; then
        replaced_something=1
        journal_append PARK "$binary_id"
        if ! mv "$dest" "$backup"; then
          if rollback_from_journal; then
            rm -f "$JOURNAL"
            printf '%s\n' "$fail_msg" >&2
          else
            printf '%s -- but automatic rollback could not fully complete; %s and %s are LEFT IN PLACE for inspection, see the lines above for what could not be restored\n' "$fail_msg" "$LOCK_DIR" "$JOURNAL" >&2
          fi
          exit 1
        fi
      fi

      journal_append INSTALL "$binary_id"
      if ! mv "$STAGING_DIR/$name" "$dest"; then
        if rollback_from_journal; then
          rm -f "$JOURNAL"
          printf '%s\n' "$fail_msg" >&2
        else
          printf '%s -- but automatic rollback could not fully complete; %s and %s are LEFT IN PLACE for inspection, see the lines above for what could not be restored\n' "$fail_msg" "$LOCK_DIR" "$JOURNAL" >&2
        fi
        exit 1
      fi
    done

    # Committed: every binary this run touched is now the new one, and
    # removing the journal is the single atomic point that says so — from
    # here on, ANY interruption (including one right after this line) finds
    # no journal and correctly treats leftover backup files as harmless
    # already-committed cleanup debris, never as something to restore from.
    # The backups and the lock are removed next, in that order, but their
    # removal is not itself atomic with the commit; that is fine, because
    # nothing after this point depends on them being gone quickly, only on
    # the journal being gone first — which is also the order
    # remove_owned_lock insists on before it will release the slot at all.
    rm -f "$JOURNAL"
    for name in $binaries; do
      rm -f "$INSTALL_DIR/.$name.old"
    done
    remove_owned_lock

    # 7. macOS launcher identity: assemble ~/Applications/Farhelm.app around
    # COPIES of the binaries just committed, plus the icon staged from the
    # desktop archive. The bundle is what makes the app reachable by name
    # from Spotlight/Alfred, gives it a Dock icon and a Cmd-Tab name, and
    # makes a second launch activate the running instance instead of racing
    # it for the embedded helm's state.
    #
    # The bundle is a DERIVED artifact, deliberately outside the journaled
    # transaction above: the journal's vocabulary is exactly the two flat
    # binaries, and the bundle is reconstructible from any committed pair,
    # so every run that gets this far simply rebuilds it wholesale — staged
    # next to the binaries, then swapped in with rm -rf + mv rather than
    # edited in place (in-place modification of an existing .app is what
    # trips macOS's App Management privacy prompt). A failure here exits 1,
    # but the messages say what is still true: the binaries in
    # FARHELM_INSTALL_DIR are committed and usable.
    #
    # The executable KEEPS the name farhelm-desktop inside the bundle: the
    # default APFS is case-insensitive, so an executable named "Farhelm"
    # would be the same directory entry as the required CLI sibling
    # "farhelm" and the second copy would clobber the first. The pretty
    # name comes from CFBundleName. Set FARHELM_NO_APP_BUNDLE=1 to skip
    # bundle assembly entirely and get the pre-bundle behavior.
    BUNDLE_NOTE=""
    if [ "$HAS_DESKTOP" -eq 1 ]; then
      if [ -n "${FARHELM_NO_APP_BUNDLE:-}" ]; then
        BUNDLE_NOTE="Skipped the Farhelm.app bundle (FARHELM_NO_APP_BUNDLE is set)."
      elif [ "$ICNS_STATE" != staged ]; then
        BUNDLE_NOTE="This release's desktop archive carries no Farhelm.icns (releases before the bundle existed); skipped assembling the Farhelm.app bundle."
      elif [ -z "${HOME:-}" ]; then
        BUNDLE_NOTE="HOME is not set; skipped assembling the Farhelm.app bundle."
      else
        app_parent="$HOME/Applications"
        app_path="$app_parent/Farhelm.app"

        # Replace only something that plausibly IS a farhelm bundle (this
        # script's, or the hand-rolled trial bundle that preceded it, both
        # of which say "farhelm" in their Info.plist). Anything else at
        # this name belongs to the user and is not this script's to
        # delete.
        if [ -e "$app_path" ]; then
          if [ ! -f "$app_path/Contents/Info.plist" ] || ! LC_ALL=C grep -qi farhelm "$app_path/Contents/Info.plist" 2>/dev/null; then
            printf '%s exists and does not look like a farhelm app bundle; refusing to replace it.\n' "$app_path" >&2
            printf 'The binaries in %s are installed and usable; remove or rename that bundle and re-run to get Farhelm.app.\n' "$INSTALL_DIR" >&2
            exit 1
          fi
        fi

        bundle_fail() {
          printf 'assembling %s failed at: %s\n' "$app_path" "$1" >&2
          printf 'The binaries in %s are installed and usable; re-run the installer to retry the bundle.\n' "$INSTALL_DIR" >&2
          exit 1
        }

        bundle_stage="$STAGING_DIR/Farhelm.app"
        mkdir -p "$bundle_stage/Contents/MacOS" "$bundle_stage/Contents/Resources" || bundle_fail "creating the staging layout"
        cp "$INSTALL_DIR/farhelm-desktop" "$bundle_stage/Contents/MacOS/farhelm-desktop" || bundle_fail "copying farhelm-desktop"
        cp "$INSTALL_DIR/farhelm" "$bundle_stage/Contents/MacOS/farhelm" || bundle_fail "copying farhelm"
        chmod 0755 "$bundle_stage/Contents/MacOS/farhelm-desktop" "$bundle_stage/Contents/MacOS/farhelm" || bundle_fail "setting binary modes"
        cp "$STAGING_DIR/Farhelm.icns" "$bundle_stage/Contents/Resources/Farhelm.icns" || bundle_fail "copying the icon"
        chmod 0644 "$bundle_stage/Contents/Resources/Farhelm.icns" || bundle_fail "setting the icon mode"

        cat >"$bundle_stage/Contents/Info.plist" <<PLIST_EOF || bundle_fail "writing Info.plist"
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

        mkdir -p "$app_parent" || bundle_fail "creating $app_parent"
        rm -rf "$app_path" || bundle_fail "removing the previous bundle (grant your terminal App Management in System Settings > Privacy & Security if this said 'Operation not permitted')"
        mv "$bundle_stage" "$app_path" || bundle_fail "moving the staged bundle into place"

        # Registration is best-effort tidiness: Launch Services discovers
        # ~/Applications on its own, this just shortens the wait. The
        # binary does not exist on the Linux CI host the installer tests
        # run on, hence the -x guard.
        LSREGISTER="/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"
        if [ -x "$LSREGISTER" ]; then
          "$LSREGISTER" -f "$app_path" >/dev/null 2>&1 || true
        fi
        BUNDLE_NOTE="Assembled $app_path (Spotlight, Dock, and Cmd-Tab identity)."
      fi
    fi

    # 8. Report. PATH warning and restart reminder first (only when they
    # apply), then the standing closing message every run prints.
    case "$INSTALL_DIR" in
      *:*)
        # A colon can never be represented as one PATH entry (POSIX PATH
        # uses it as the separator, with no escape mechanism), and the
        # membership test below can be fooled into a false match by two
        # UNRELATED PATH entries that happen to concatenate into this
        # directory's name around a colon — so this case is handled first,
        # unconditionally, rather than trusted to that test at all.
        printf '\n%s contains a colon, which cannot be represented in PATH; add it to PATH by hand.\n' "$INSTALL_DIR"
        ;;
      *"$NEWLINE"*)
        printf '\n%s is not on your PATH; its name cannot be printed as a safe one-line command. Add it to PATH by hand.\n' "$INSTALL_DIR"
        ;;
      *)
        case ":$PATH:" in
          *":$INSTALL_DIR:"*) ;;
          *)
            printf '\n%s is not on your PATH.' "$INSTALL_DIR"
            if quoted=$(shquote "$INSTALL_DIR" 2>/dev/null); then
              # shellcheck disable=SC2016 # the literal text "$PATH" is what should print, not this shell's PATH
              printf ' Add it with:\n  export PATH=%s:$PATH\n' "$quoted"
            else
              printf ' Add it to PATH by hand.\n'
            fi
            ;;
        esac
        ;;
    esac

    if [ "$replaced_something" -eq 1 ]; then
      echo ""
      echo "Updated. Restart what is running:"
      echo "  Linux: systemctl --user restart farhelm-supervisor farhelm-helm"
      echo "  macOS: quit and reopen Farhelm (the desktop app owns the embedded helm and"
      echo "  any supervisor it started as child processes; a supervisor you started by"
      echo "  hand with 'farhelm supervisor run' is reused as-is and needs restarting"
      echo "  yourself)."
      echo "  Running sessions survive either way — they live in tmux, which neither"
      echo "  restart touches."
    fi

    echo ""
    if [ "$HAS_DESKTOP" -eq 1 ]; then
      printf 'Installed farhelm %s (and farhelm-desktop) to %s.\n' "$VERSION_NUM" "$INSTALL_DIR"
    else
      printf 'Installed farhelm %s to %s.\n' "$VERSION_NUM" "$INSTALL_DIR"
    fi
    if [ -n "$BUNDLE_NOTE" ]; then
      printf '%s\n' "$BUNDLE_NOTE"
    fi
    echo ""
    echo "If this machine should run your helm (the web UI on 127.0.0.1:7433) and host"
    echo "agent sessions itself, run 'farhelm helm setup' — it writes and starts the helm"
    echo "and supervisor user units."
    echo ""
    echo "Do NOT run it if this machine runs the desktop app (farhelm-desktop starts its"
    echo "own helm and local supervisor), or if it is a Linux session host you will add"
    echo "from another helm's hosts panel (that helm installs the supervisor here over"
    echo "SSH), or if you only want a browser tab against a helm elsewhere (nothing to"
    echo "set up)."

    # tmux hint: parsed as "tmux <major>.<minor><letter?>" (tmux's own
    # release spelling, e.g. "3.7c"); anything that does not match that
    # shape, or no tmux at all, counts as "none". The WHOLE output must be
    # exactly one line matching that shape: `sed`'s anchors apply per
    # LINE, not per whole value, so a banner or warning printed alongside
    # a real version line (e.g. "tmux 3.7c\nsome vendor banner") would
    # otherwise still parse as that valid line — even though it is
    # multi-line output the supervisor itself treats as unparseable, so
    # this parser must not be more lenient than that. tmux_have stays
    # "none" until parsing SUCCEEDS: a string like "tmux next-3.8" is
    # correctly rejected for the floor comparison below, and must not be
    # printed as though it were a recognized installed version.
    tmux_have="none"
    meets_floor=0
    if command -v tmux >/dev/null 2>&1; then
      tmux_version_output=$(tmux -V 2>/dev/null || true)
    else
      tmux_version_output=""
    fi
    case "$tmux_version_output" in
      *"$NEWLINE"* | *"$CR"*) parsed="" ;;
      *) parsed=$(printf '%s' "$tmux_version_output" | sed -n 's/^tmux \([0-9][0-9]*\)\.\([0-9][0-9]*\)\([a-z]\{0,1\}\)$/\1 \2 \3/p') ;;
    esac
    if [ -n "$parsed" ]; then
      tmux_have="$tmux_version_output"
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
    if [ "$meets_floor" -ne 1 ]; then
      echo ""
      printf 'tmux 3.7c or newer is required wherever sessions run; this machine has %s. Linuxbrew/Homebrew: brew install tmux.\n' "$tmux_have"
    fi
  }

  main "$@"
}
