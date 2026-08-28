#!/usr/bin/env bash
# Build Farhelm's private tmux against pinned static libevent and ncurses.
# Linux outputs are fully static musl binaries. Apple arm64 keeps only the
# platform libraries dynamic; libevent and ncurses are the private archives
# built here because macOS does not support fully static executables.

set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
pins="$repo/.github/release/source-pins.env"
source "$pins"

target=${1:?usage: build-private-tmux.sh TARGET OUTPUT}
output=${2:?usage: build-private-tmux.sh TARGET OUTPUT}
zig=${ZIG:-zig}
work=$(mktemp -d "${TMPDIR:-/tmp}/farhelm-tmux.XXXXXX")
trap 'rm -rf "$work"' EXIT

# Fetch one pinned source archive from the first URL that answers, then
# verify it against the pinned SHA-256 and unpack it.
#
# Several URLs, not one, because every build here — CI's pinned tmux, the
# release gate, the release's own tmux payloads — runs on a cache miss, and
# a single origin that is down takes all of them down with it: ftp.gnu.org
# refused connections for hours on 2026-08-28 and every job that needed
# ncurses failed at this line. The checksum is what makes a mirror safe to
# trust — a mirror can only hand back the exact bytes the pin names or fail
# the check — so the alternates only widen availability, never trust.
#
# A short connect timeout is part of the same fix: an origin that drops
# SYNs rather than refusing them would otherwise hold each attempt for
# curl's two-minute default before the next URL got its turn.
download() {
  local name=$1 checksum=$2 url
  shift 2
  for url in "$@"; do
    if curl -fsSL --connect-timeout 15 --retry 2 "$url" -o "$work/$name"; then
      break
    fi
    echo "download of $name from $url failed; trying the next source" >&2
    rm -f "$work/$name"
  done
  test -s "$work/$name" || {
    echo "could not download $name from any source" >&2
    exit 1
  }
  if command -v sha256sum >/dev/null 2>&1; then
    echo "$checksum  $work/$name" | sha256sum -c -
  else
    test "$(shasum -a 256 "$work/$name" | awk '{print $1}')" = "$checksum"
  fi
  tar xzf "$work/$name" -C "$work"
}

download "tmux.tar.gz" "$TMUX_SHA256" \
  "https://github.com/tmux/tmux/releases/download/${TMUX_VERSION}/tmux-${TMUX_VERSION}.tar.gz"
download "libevent.tar.gz" "$LIBEVENT_SHA256" \
  "https://github.com/libevent/libevent/releases/download/release-${LIBEVENT_VERSION}/libevent-${LIBEVENT_VERSION}.tar.gz"
# ftpmirror.gnu.org redirects to a nearby GNU mirror; the canonical host is
# still tried first so a mirror that lags a fresh ncurses release does not
# become the reason a build fails while the origin is fine.
download "ncurses.tar.gz" "$NCURSES_SHA256" \
  "https://ftp.gnu.org/gnu/ncurses/ncurses-${NCURSES_VERSION}.tar.gz" \
  "https://ftpmirror.gnu.org/ncurses/ncurses-${NCURSES_VERSION}.tar.gz"

prefix="$work/prefix"
configure_host=()
static_link=()
tmux_configure=()
ncurses_configure=()
# musl provides forkpty in libc. tmux's old autoconf probe redeclares it with
# a pre-const prototype and can misclassify modern musl headers, so the Linux
# targets supply the known library result instead of compiling that probe.
case "$target" in
  x86_64-unknown-linux-musl|aarch64-unknown-linux-musl)
    configure_triple=${target/-unknown/}
    export CC="$zig cc -target $configure_triple"
    export AR="$zig ar"
    export RANLIB="$zig ranlib"
    export ac_cv_search_forkpty="none required"
    configure_host=(--host="$configure_triple")
    static_link=(-static)
    tmux_configure=(--enable-static)
    ;;
  aarch64-apple-darwin)
    test "$(uname -s)" = Darwin
    test "$(uname -m)" = arm64
    export CC=cc AR=ar RANLIB=ranlib
    # tmux 3.7c's darwin configure refuses to guess about jemalloc (as it
    # already does about utf8proc): an explicit choice is required, and
    # the private build links nothing but its own prefix.
    tmux_configure=(--disable-jemalloc)
    # A case-insensitive APFS volume cannot represent every upstream terminfo
    # key. macOS owns the runtime database, so install only the private
    # archives and compile the system lookup path into ncurses instead.
    ncurses_configure=(
      --disable-db-install
      --with-default-terminfo-dir=/usr/share/terminfo
      --with-terminfo-dirs=/usr/share/terminfo
      --enable-pc-files
      --with-pkg-config-libdir="$prefix/lib/pkgconfig"
    )
    ;;
  *)
    echo "unsupported tmux target: $target" >&2
    exit 2
    ;;
esac

(cd "$work/libevent-$LIBEVENT_VERSION" && ./configure "${configure_host[@]}" --prefix="$prefix" --disable-shared --enable-static --disable-openssl --disable-thread-support --disable-libevent-regress --disable-samples && make -j"$(getconf _NPROCESSORS_ONLN)" && make install)
(cd "$work/ncurses-$NCURSES_VERSION" && BUILD_CC=cc BUILD_CFLAGS=-O2 ./configure "${configure_host[@]}" --prefix="$prefix" --without-shared --with-normal --with-termlib --without-debug --without-ada --without-cxx --without-manpages --without-progs --without-tests --enable-widec "${ncurses_configure[@]}" && make -j"$(getconf _NPROCESSORS_ONLN)" && make install)
# tmux 3.7b's configure appends -lncurses after finding ncursesw whenever the
# wide header is named ncurses.h. Both names must resolve to the same private
# archive or the final link falls through to a host library.
ln -s libncursesw.a "$prefix/lib/libncurses.a"

export PKG_CONFIG_PATH="$prefix/lib/pkgconfig"
# tmux probes the standalone tinfow/tinfo modules before ncursesw. Restricting
# pkg-config to the prefix keeps a host tinfo.pc from making configure record a
# library that the target linker cannot use; --with-termlib supplies that
# module and archive inside the prefix instead.
export PKG_CONFIG_LIBDIR="$prefix/lib/pkgconfig"
export CPPFLAGS="-I$prefix/include -I$prefix/include/ncursesw"
export LDFLAGS="-L$prefix/lib ${static_link[*]}"
tmux_source="$work/tmux-$TMUX_VERSION"
# Release tarballs carry the generated parser newer than its grammar. Using it
# keeps the reproducible build independent of whichever yacc happens to be on
# a runner while still refusing a source tree that would need regeneration.
test "$tmux_source/cmd-parse.c" -nt "$tmux_source/cmd-parse.y"
mkdir "$work/build-tools"
# Autoconf insists on discovering a yacc even when the generated parser is
# current. The shim is never invoked by make; it only lets that obsolete
# configure-time probe accept the release tarball's own generated source.
ln -s /usr/bin/true "$work/build-tools/bison"
(cd "$tmux_source" && PATH="$work/build-tools:$PATH" ./configure "${configure_host[@]}" "${tmux_configure[@]}" && make -j"$(getconf _NPROCESSORS_ONLN)")
install -m 0755 "$work/tmux-$TMUX_VERSION/tmux" "$output"
