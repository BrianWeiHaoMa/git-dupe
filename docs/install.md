# Installing git-dupe

git-dupe is one executable, `git-dupe`, which Git runs for `git dupe …` when it stands
in a directory on `PATH`, and a manual page, `git-dupe.1`, which `man git-dupe` and
`git dupe --help` show when it stands in the `man1` directory of a manual path. It runs
on Linux and on macOS and needs Git 2.43.0 or newer, below 3.0.0, a release that is not
out yet and may change what git-dupe relies on.

## With the install script

On x86-64 Linux and on macOS, paste:

```sh
curl -fsSL https://raw.githubusercontent.com/BrianWeiHaoMa/git-dupe/main/install.sh | sh
```

The script, [`install.sh`](../install.sh) in this repository, finds the latest release,
downloads the archive for your machine and its `.sha256`, checks the archive's SHA-256
digest, unpacks it, and runs the executable once. Only then does it install the
executable as `~/.local/bin/git-dupe` and the page as
`~/.local/share/man/man1/git-dupe.1`, the two copies the archive's `INSTALL` note makes.
It asks nothing, needs no `sudo`, and writes nothing else outside a temporary directory
it removes. It needs `curl`, `tar`, and `shasum` or `sha256sum`, which macOS and the
usual Linux distributions have.

A failure before the two copies installs nothing, and each copy replaces its file by a
rename, so an interrupted run leaves each file as it was or installed, and running the
line again completes it. Run later, the same line installs the newest release over the
one installed.

When `~/.local/bin` is not on `PATH`, as on macOS by default, the script ends with the
line to add to your shell's startup file; `man git-dupe` finds the page through the same
entry. On a machine no archive is built for, or where the downloaded executable does not
run, as on a Linux whose C library is older than the archive's, the script says so and
installs nothing: install [from source](#from-source) there. It also warns, and installs
all the same, when the Git on `PATH` is missing or outside the versions git-dupe
supports.

## From a release, by hand

Download the archive for your machine,
`git-dupe-<version>-<platform>-<architecture>.tar`, from the
[releases](https://github.com/BrianWeiHaoMa/git-dupe/releases) and follow its `INSTALL`
note, which installs the executable and the manual page, with `~/.local/bin` on `PATH`
and `~/.local/share/man` on the manual path:

```sh
tar -xf git-dupe-<version>-<platform>-<architecture>.tar
cd git-dupe-<version>
mkdir -p ~/.local/bin ~/.local/share/man/man1
cp git-dupe ~/.local/bin/
cp git-dupe.1 ~/.local/share/man/man1/
```

Any other directory on `PATH`, and the `man1` directory of any other manual path, serve
the same. Each archive has a `.sha256` beside it that names the archive by its bare
name, so that `shasum -a 256 -c <archive>.sha256`, or `sha256sum -c`, checks a download
in its own directory.

A release has one archive for each kind of machine, its executable built and checked on
that kind of machine, the Macs' on macOS 26:

| Archive | For |
|---|---|
| `git-dupe-<version>-linux-x86_64.tar` | x86-64 Linux with the GNU C library 2.39 or newer |
| `git-dupe-<version>-macos-arm64.tar` | a Mac with Apple silicon, on macOS 11.0 or newer |
| `git-dupe-<version>-macos-x86_64.tar` | an Intel Mac, on macOS 10.12 or newer |

On a Mac, a browser marks the archive it downloads as quarantined, the mark passes to the
files unpacked from it, and macOS holds the executable, which Apple has not notarized,
instead of running it: remove the mark once, after installing and before its first run,
with `xattr -d com.apple.quarantine ~/.local/bin/git-dupe`, or download with
`curl -LO`, which sets none, as the install script's `curl` sets none.

## From source

On another machine, or where the executable does not start, install from source. In a
clone of this repository, with Rust installed through rustup, which installs the
toolchain the repository pins on first use:

```sh
cargo install --path .
```

That installs the executable alone. `git dupe help` and `git dupe help <command>` cover
daily use from the executable; the manual page behind `man git-dupe` and
`git dupe --help` comes with a release archive, installed as its `INSTALL` says.

## Git

However git-dupe is installed, the Git on `PATH` decides: `git --version` must say
2.43.0 or newer, below 3.0.0. Ubuntu 24.04 ships 2.43.0 and Debian 13 ships 2.47;
Ubuntu 22.04 ships 2.34 and Debian 12 ships 2.39, so there a newer Git comes first, from
the `git-core` PPA, a backport, or a build from source. On a Mac, Apple's Git, which the
Command Line Tools for Xcode install, will do, 2.50.1 in their version 26.6, and so will
Homebrew's.

## Uninstalling

Run `git dupe detach` in each project that uses it, then delete
`~/.local/bin/git-dupe` and `~/.local/share/man/man1/git-dupe.1`, or run
`cargo uninstall git-dupe` after an install from source.
