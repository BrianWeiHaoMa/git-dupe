#!/bin/sh
# Installs the latest release of git-dupe on this machine: the executable into
# ~/.local/bin and its manual page into ~/.local/share/man/man1, the two copies the
# release's INSTALL note makes. Run it as
#
#   curl -fsSL https://raw.githubusercontent.com/BrianWeiHaoMa/git-dupe/main/install.sh | sh
#
# It reads the latest release's tag from the releases pages, downloads the archive of
# this machine's platform and architecture and its .sha256, checks the archive's
# SHA-256 digest, unpacks it, and runs the executable once; only then does it place each
# file, by a rename, so that an interrupted run leaves each file as it was or as
# installed, and running it again completes the install. Every failure before that
# installs nothing. It runs under any POSIX sh on x86-64 Linux and on macOS and needs
# curl, tar, uname, mktemp, and shasum or sha256sum. It never reads standard input,
# which holds this script when it is piped, and writes nothing else outside a temporary
# directory it removes.
#
# GIT_DUPE_RELEASES_URL, when set, replaces the base of the releases pages,
# https://github.com/BrianWeiHaoMa/git-dupe/releases, for the repository's checks, which
# point it at a stand-in on the loopback interface, and for a mirror serving the same
# paths: <base>/latest redirecting to <base>/tag/<tag>, and each file of a release under
# <base>/download/<tag>/. Only this script reads it.
#
# Every command stands in a function and the last line calls the first, because sh runs
# a piped script as it arrives: a download cut short defines functions and runs nothing.

say() {
  printf '%s\n' "$*"
}

warn() {
  printf 'install.sh: warning: %s\n' "$*" >&2
}

fail() {
  printf 'install.sh: %s\n' "$*" >&2
  exit 1
}

# What an unfinished run leaves: the temporary directory, and a copy not yet renamed
# into place.
clean_up() {
  if [ -n "$work" ]; then rm -rf "$work" || :; fi
  if [ -n "$staged" ]; then rm -f "$staged" || :; fi
}

need() {
  for tool in "$@"; do
    command -v "$tool" >/dev/null 2>&1 ||
      fail "$tool is not on PATH, and installing git-dupe needs it; nothing was installed."
  done
  if ! command -v shasum >/dev/null 2>&1 && ! command -v sha256sum >/dev/null 2>&1; then
    fail "neither shasum nor sha256sum is on PATH, and checking the download needs one; nothing was installed."
  fi
}

fetch() {
  curl --proto "$protocols" -fsSL "$@"
}

# The suffix of this machine's archive, as a release names its archives: the platform
# and the architecture as `uname -m` prints it.
archive_suffix() {
  system=$(uname -s) || fail "uname -s failed; nothing was installed."
  machine=$(uname -m) || fail "uname -m failed; nothing was installed."
  case "$system $machine" in
    'Linux x86_64') suffix=linux-x86_64 ;;
    'Darwin arm64') suffix=macos-arm64 ;;
    'Darwin x86_64') suffix=macos-x86_64 ;;
    *) fail "no release of git-dupe is built for $system $machine, as uname -s and uname -m name this machine; install it from source instead, as $from_source says. Nothing was installed." ;;
  esac
}

# The latest release's tag, v<version>: the last part of the page <base>/latest leads to.
latest_release() {
  final=$(fetch -I -o /dev/null -w '%{url_effective}' "$releases/latest") ||
    fail "cannot find the latest release at $releases/latest; nothing was installed."
  tag=${final##*/}
  version=${tag#v}
  case ${final%/*} in
    */tag) ;;
    *) fail "$releases/latest led to $final, which is no release's page; nothing was installed." ;;
  esac
  case $tag in
    v[0-9]*) ;;
    *) fail "the latest release's tag, $tag, is not v<version>; nothing was installed." ;;
  esac
  case $version in
    *[!0-9A-Za-z.-]*) fail "the latest release's tag, $tag, is not v<version>; nothing was installed." ;;
  esac
}

download() {
  archive=git-dupe-$version-$suffix.tar
  say "Downloading $archive from $releases/download/$tag/"
  for name in "$archive" "$archive.sha256"; do
    fetch -o "$work/$name" "$releases/download/$tag/$name" ||
      fail "cannot download $releases/download/$tag/$name; nothing was installed."
  done
}

# The SHA-256 digest of the file $1, as 64 lowercase hexadecimal digits.
digest_of() {
  if command -v shasum >/dev/null 2>&1; then
    summed=$(shasum -a 256 <"$1") || return 1
  else
    summed=$(sha256sum <"$1") || return 1
  fi
  printf '%s\n' "${summed%% *}"
}

# The .sha256 is read here rather than given to `shasum -c`, which checks whatever files
# it names: it must be the one line a release writes, the digest and the archive's bare
# name, and that digest the archive's own.
check_digest() {
  published=$(cat "$work/$archive.sha256") ||
    fail "cannot read $archive.sha256; nothing was installed."
  digest=${published%% *}
  case $digest in
    *[!0-9a-f]* | '') digest= ;;
  esac
  case $published in
    "$digest  $archive" | "$digest *$archive") ;;
    *) digest= ;;
  esac
  if [ "${#digest}" -ne 64 ]; then
    fail "$archive.sha256 is not the SHA-256 digest of $archive followed by its name; nothing was installed."
  fi
  actual=$(digest_of "$work/$archive") ||
    fail "cannot compute the SHA-256 digest of $archive; nothing was installed."
  if [ "$actual" != "$digest" ]; then
    fail "$archive does not have the SHA-256 digest its $archive.sha256 names; nothing was installed."
  fi
}

# Unpacks the archive and runs its executable before anything is installed, so that one
# built for a newer C library or another architecture fails here and not at the first
# `git dupe`.
unpack() {
  tar -xf "$work/$archive" -C "$work" ||
    fail "cannot unpack $archive; nothing was installed."
  unpacked=$work/git-dupe-$version
  for member in git-dupe git-dupe.1; do
    if [ ! -f "$unpacked/$member" ]; then
      fail "$archive holds no git-dupe-$version/$member; nothing was installed."
    fi
  done
  if ! said=$("$unpacked/git-dupe" --version 2>&1 </dev/null); then
    if [ -n "$said" ]; then printf '%s\n' "$said" >&2; fi
    fail "the git-dupe of $archive does not run on this machine; install it from source instead, as $from_source says. Nothing was installed."
  fi
}

# Copies $1 into the directory $2 as $3, with mode $4, through a fresh file mktemp makes
# in that directory, so that no file already there is touched, and a rename onto $3, so
# that $2/$3 is at every moment the old file or the new one, never part of either.
place() {
  staged=$(mktemp "$2/.$3.XXXXXX") || fail "cannot make a file in $2."
  cp "$1" "$staged" || fail "cannot copy $3 into $2."
  # The archive's mode, whatever the umask gave the copy.
  chmod "$4" "$staged" || fail "cannot set the mode of $staged."
  mv -f "$staged" "$2/$3" || fail "cannot rename $staged onto $2/$3."
  staged=
}

install_files() {
  bin=$HOME/.local/bin
  man1=$HOME/.local/share/man/man1
  # mv onto a directory would move the file into it.
  for target in "$bin/git-dupe" "$man1/git-dupe.1"; do
    if [ -d "$target" ]; then
      fail "a directory stands at $target, where the file belongs; nothing was installed."
    fi
  done
  mkdir -p "$bin" "$man1" || fail "cannot make $bin and $man1; nothing was installed."
  place "$unpacked/git-dupe" "$bin" git-dupe 755
  place "$unpacked/git-dupe.1" "$man1" git-dupe.1 644
}

# A warning only: the executable installs the same whatever Git is on PATH.
check_git() {
  wanted="git-dupe needs Git 2.43.0 or newer, below 3.0.0"
  if ! command -v git >/dev/null 2>&1; then
    warn "no git is on PATH; $wanted."
    return 0
  fi
  git_version=$(git --version 2>/dev/null </dev/null) || git_version=
  git_version=${git_version#git version }
  git_version=${git_version%% *}
  major=${git_version%%.*}
  minor=${git_version#*.}
  minor=${minor%%.*}
  case $major.$minor in
    .* | *. | *[!0-9.]*) warn "cannot read the version of the git on PATH; $wanted." ;;
    *)
      if [ "$major" -ne 2 ] || [ "$minor" -lt 43 ]; then
        warn "the git on PATH is $git_version; $wanted."
      fi
      ;;
  esac
}

report() {
  say "Installed $bin/git-dupe and $man1/git-dupe.1:"
  "$bin/git-dupe" --version </dev/null
  case ":$PATH:" in
    *":$bin:"* | *":$bin/:"*) ;;
    *)
      say ""
      say "Add ~/.local/bin to PATH, where Git looks for git-dupe and man for its page: it is not on PATH now. This line in your shell's startup file, ~/.zshrc for zsh or ~/.bashrc for bash, adds it for every new shell:"
      say "    export PATH=\"\$HOME/.local/bin:\$PATH\""
      ;;
  esac
  check_git
}

main() {
  set -eu
  releases=${GIT_DUPE_RELEASES_URL:-https://github.com/BrianWeiHaoMa/git-dupe/releases}
  releases=${releases%/}
  from_source=https://github.com/BrianWeiHaoMa/git-dupe/blob/main/docs/install.md#from-source
  work=
  staged=
  need uname curl tar mktemp
  if [ -z "${HOME:-}" ]; then fail "HOME is not set; nothing was installed."; fi
  archive_suffix
  # Over HTTPS, no redirect may lead to plain HTTP.
  case $releases in
    https://*) protocols='=https' ;;
    *) protocols='=http,https' ;;
  esac
  trap clean_up EXIT
  trap 'exit 1' HUP INT TERM
  work=$(mktemp -d "${TMPDIR:-/tmp}/git-dupe-install.XXXXXX") ||
    fail "cannot make a temporary directory; nothing was installed."
  latest_release
  download
  check_digest
  unpack
  install_files
  report
}

main
