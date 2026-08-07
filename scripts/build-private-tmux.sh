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

download() {
  local url=$1 name=$2 checksum=$3
  curl -fsSL "$url" -o "$work/$name"
  if command -v sha256sum >/dev/null 2>&1; then
    echo "$checksum  $work/$name" | sha256sum -c -
  else
    test "$(shasum -a 256 "$work/$name" | awk '{print $1}')" = "$checksum"
  fi
  tar xzf "$work/$name" -C "$work"
}

download "https://github.com/tmux/tmux/releases/download/${TMUX_VERSION}/tmux-${TMUX_VERSION}.tar.gz" "tmux.tar.gz" "$TMUX_SHA256"
download "https://github.com/libevent/libevent/releases/download/release-${LIBEVENT_VERSION}/libevent-${LIBEVENT_VERSION}.tar.gz" "libevent.tar.gz" "$LIBEVENT_SHA256"
download "https://ftp.gnu.org/gnu/ncurses/ncurses-${NCURSES_VERSION}.tar.gz" "ncurses.tar.gz" "$NCURSES_SHA256"

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
