#!/bin/sh
# Serve the docs site from this checkout at a fixed local address, for reviewing drafts while they are written.
#
# Usage:
#   website/scripts/preview.sh [start]      start the server if needed, wait until it answers, print its URL
#   website/scripts/preview.sh takeover     stop another checkout's preview server on the port (or one whose checkout
#                                           was deleted), then start this one
#   website/scripts/preview.sh stop         stop this checkout's server
#   website/scripts/preview.sh status       say whether the port is served, and from which checkout
#   website/scripts/preview.sh url <file>   print the preview URL of a page source file under src/content/docs/
#
# Exit status: 0 on success. `start` exits 1 when the port is held by something other than this checkout's server, and
# prints nothing on stdout then, so no link to the wrong checkout's pages can come out of it. `status` exits 0 when
# this checkout is serving, 1 when something else holds the port, and 3 when nothing does. Usage errors exit 2.
#
# The server is Astro's dev server in its background mode, not `vercel dev`: the site is plain static Astro, so `vercel
# dev` would run the same thing underneath while also needing a Vercel login and a linked project. The dev server
# rebuilds as soon as a file is saved, new pages, sidebar changes, and astro.config.mjs included, so every edit shows
# up on a browser reload. A dependency change is the exception: a running server keeps the modules it loaded, so it
# needs `stop` and a fresh `start`. Astro keeps the background server's lock file (`.astro/dev.json`) and log in this
# checkout (`astro dev logs` shows the log); that lock is how `astro dev stop` finds the server.
#
# NOTE: The port is fixed on purpose, unlike every test harness in this repository. The maintainer reads the preview
# through a port forward they set up once, and a port that changed per run would need a new forward every time. The
# cost is that only one checkout on the machine can serve a preview at a time. `start` refuses rather than taking the
# port from another checkout's server, which belongs to whoever started it; `takeover` exists for when the maintainer
# says to take it anyway.
#
# Which checkout a process belongs to is decided by comparing its working directory with the website directory by
# device and inode (ss and /proc, so Linux only), never by matching process names or path strings. Strings would fail
# for a checkout reached through a symlink, and name matching is how a stale lock gets the wrong process killed: Astro's
# own `astro dev stop` trusts any pid in the lock whose command line looks like astro, and pids get reused. Astro's lock
# also cannot say who holds the port, because it is per checkout and knows nothing about another checkout's server.

port=14000
base_url="http://127.0.0.1:$port"

website=$(cd "$(dirname "$0")/.." && pwd -P) || exit 1
astro="$website/node_modules/.bin/astro"

usage() {
  echo "usage: $0 [start|takeover|stop|status|url <file>]" >&2
  exit 2
}

# Print the pid of whatever listens on the port, or nothing. ss only reports pids for processes this user may inspect,
# which on this machine is everything the agents start. When Astro restarts its server in place (it does on every
# astro.config.mjs edit, which is where the sidebar lives), ss briefly lists the listening socket with no process at
# all; without the bounded wait below, `start` right after a sidebar edit refuses this checkout's own server.
listener_pid() {
  tries=0
  while :; do
    found=$(ss -ltnpH "sport = :$port")
    lpid=$(printf '%s\n' "$found" | sed -n 's/.*pid=\([0-9][0-9]*\).*/\1/p' | head -n 1)
    if [ -n "$lpid" ] || [ -z "$found" ] || [ $tries -ge 50 ]; then
      echo "$lpid"
      return 0
    fi
    tries=$((tries + 1))
    sleep 0.1
  done
}

port_in_use() {
  [ -n "$(ss -ltnH "sport = :$port")" ]
}

# True when process $1 runs in directory $2, compared by device and inode.
runs_in() {
  [ -n "$1" ] || return 1
  a=$(stat -L -c %d:%i "/proc/$1/cwd" 2>/dev/null) || return 1
  b=$(stat -L -c %d:%i "$2" 2>/dev/null) || return 1
  [ "$a" = "$b" ]
}

ours_is_serving() {
  runs_in "$(listener_pid)" "$website"
}

# Describe whoever holds the port, via the working directory of the listening process.
describe_listener() {
  pid=$(listener_pid)
  if [ -n "$pid" ]; then
    dir=$(readlink "/proc/$pid/cwd" 2>/dev/null) || dir="an unknown directory"
    case "$dir" in
      *" (deleted)")
        echo "port $port is held by pid $pid, whose directory ${dir% (deleted)} no longer exists"
        ;;
      *) echo "port $port is held by pid $pid, running in $dir" ;;
    esac
  else
    echo "port $port is in use by a process this user cannot inspect"
  fi
}

# Print the pid recorded in the Astro lock file $1 when it describes a background server, or nothing. Astro writes the
# same lock for a foreground `astro dev` (a maintainer's own `bun run dev`, say), and this script never stops one of
# those: it is not a preview server, and its owner is watching its terminal.
lock_pid() {
  [ -f "$1" ] || return 0
  node -e '
    try {
      const l = require(process.argv[1]);
      if (l.background === true && Number.isInteger(l.pid)) console.log(l.pid);
    } catch {}' "$1"
}

# Stop the Astro background server of the website directory $1, but only if the pid its lock names really runs there.
# A lock whose pid is dead or belongs to some other process is stale and is deleted instead (see the header for why
# `astro dev stop` must not be trusted with it). A lock for a foreground server is left alone entirely (see lock_pid).
stop_server_in() {
  dir=$1
  dir_lock="$dir/.astro/dev.json"
  [ -f "$dir_lock" ] || return 0
  pid=$(lock_pid "$dir_lock")
  if [ -z "$pid" ]; then
    return 0
  elif runs_in "$pid" "$dir"; then
    (cd "$dir" && ./node_modules/.bin/astro dev stop >&2)
  else
    rm -f "$dir_lock"
  fi
}

# True when process $1 is an Astro dev server whose working directory has been deleted: a preview left behind by a
# checkout or scratch copy that is gone. Nothing can reach its lock any more, so nothing but a signal stops it, and no
# one can be using it, since every request to it hangs.
orphaned_astro() {
  [ -n "$1" ] || return 1
  case "$(readlink "/proc/$1/cwd" 2>/dev/null)" in
    *" (deleted)") ;;
    *) return 1 ;;
  esac
  tr '\0' ' ' <"/proc/$1/cmdline" 2>/dev/null | grep -q 'astro/bin/astro\.mjs dev '
}

start() {
  if ours_is_serving; then
    echo "$base_url/docs/"
    return 0
  fi
  if port_in_use; then
    echo "preview: $(describe_listener)." >&2
    echo "preview: that is not this checkout's server, so its pages are not your draft; hand out no preview links." >&2
    if orphaned_astro "$(listener_pid)"; then
      echo "preview: its checkout is gone, so it is no one's; '$0 takeover' clears it without asking." >&2
    else
      echo "preview: tell the maintainer, and run '$0 takeover' only if they say to." >&2
    fi
    exit 1
  fi

  cd "$website" || exit 1
  # Cheap when nothing changed, and it keeps a rebase or a dependency bump from serving stale modules. Its output goes
  # to stderr so stdout stays exactly the URL.
  bun install --frozen-lockfile >&2 || exit 1

  # A background server this checkout started earlier on some other port (a plain `bun run dev` from an agent, say)
  # holds Astro's per-checkout lock, and Astro would refuse to start a second one. It is this checkout's, so replacing
  # it is ours to do.
  stop_server_in "$website"

  # --background returns once the server answers. Astro's output is only interesting when something went wrong.
  if ! out=$("$astro" dev --background --host 127.0.0.1 --port "$port" 2>&1); then
    echo "preview: astro dev failed to start:" >&2
    echo "$out" >&2
    exit 1
  fi

  # Vite moves to the next free port when the requested one is taken, so success above is not proof the server is on
  # this port: something else may have bound it between the check and Astro's bind.
  if ! ours_is_serving; then
    echo "preview: another process took port $port while this server started; stopping this one. Astro said:" >&2
    echo "$out" >&2
    stop_server_in "$website"
    echo "preview: $(describe_listener). Hand out no preview links; tell the maintainer." >&2
    exit 1
  fi
  echo "$base_url/docs/"
}

# Take the port from another checkout's preview server. Only an Astro background server whose own checkout's lock names
# it, or an Astro server whose checkout no longer exists, is stopped; anything else on the port is left alone, because
# there is no telling what it is.
takeover() {
  if port_in_use && ! ours_is_serving; then
    tpid=$(listener_pid)
    if orphaned_astro "$tpid"; then
      echo "preview: stopping pid $tpid, a preview server whose checkout no longer exists" >&2
      kill -TERM "$tpid" 2>/dev/null
      tries=0
      while kill -0 "$tpid" 2>/dev/null && [ $tries -lt 50 ]; do
        tries=$((tries + 1))
        sleep 0.1
      done
    else
      other=$(readlink "/proc/$tpid/cwd" 2>/dev/null)
      if [ -z "$tpid" ] || [ -z "$other" ] || [ "$(lock_pid "$other/.astro/dev.json")" != "$tpid" ] \
        || ! runs_in "$tpid" "$other"; then
        echo "preview: $(describe_listener), and it is not another checkout's preview server; not touching it." >&2
        exit 1
      fi
      echo "preview: stopping the preview server of $other" >&2
      stop_server_in "$other"
    fi
  fi
  start
}

stop() {
  stop_server_in "$website"
}

status() {
  if ours_is_serving; then
    echo "serving this checkout at $base_url/docs/"
  elif port_in_use; then
    echo "not serving this checkout; $(describe_listener)"
    return 1
  else
    echo "not running"
    return 3
  fi
}

# Map a page source file to its URL. Starlight's route is the file's path under src/content/docs/ without the
# extension, with a trailing `index` dropped, so `docs/using/manage-hosts.md` serves at /docs/using/manage-hosts/ and
# `docs/index.mdx` at /docs/. Starlight also slugifies each path segment; every page here is already named in lowercase
# kebab case, which slugifies to itself, so a name outside that shape is refused rather than guessed at. No page
# overrides its route with a `slug:` frontmatter field; one that did would break this mapping.
url() {
  [ -f "$1" ] || { echo "preview: no such file: $1" >&2; exit 1; }
  file=$(realpath "$1") || exit 1
  content="$website/src/content/docs/"
  case "$file" in
    "$content"*.md | "$content"*.mdx) ;;
    *)
      echo "preview: $1 is not a page under $content" >&2
      exit 1
      ;;
  esac
  route=${file#"$content"}
  route=${route%.mdx}
  route=${route%.md}
  case "$route" in
    *[!a-z0-9/-]* | -* | */-*)
      echo "preview: $1 is not named in lowercase kebab case; work out its URL from the site instead" >&2
      exit 1
      ;;
    index) route= ;;
    */index) route=${route%/index} ;;
  esac
  if [ -n "$route" ]; then
    echo "$base_url/$route/"
  else
    echo "$base_url/"
  fi
}

case "${1:-start}" in
  start) [ $# -le 1 ] || usage; start ;;
  takeover) [ $# -eq 1 ] || usage; takeover ;;
  stop) [ $# -eq 1 ] || usage; stop ;;
  status) [ $# -eq 1 ] || usage; status ;;
  url) [ $# -eq 2 ] || usage; url "$2" ;;
  *) usage ;;
esac
