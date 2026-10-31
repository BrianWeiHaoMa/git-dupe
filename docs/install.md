# Installing git-dupe

git-dupe is one executable, `git-dupe`, which Git runs for `git dupe …` when it stands
in a directory on `PATH`, and a manual page, `git-dupe.1`, which `man git-dupe` and
`git dupe --help` show when it stands in the `man1` directory of a manual path. It runs
on Linux, on macOS, and on Windows 11, and needs Git 2.43.0 or newer, below 3.0.0, a
release that is not out yet and may change what git-dupe relies on. Windows has its own
section, [On Windows](#on-windows); the sections before it are for Linux and macOS.

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

Download the tar for your machine,
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
that kind of machine, the Macs' on macOS 26 and Windows's on Windows Server 2025:

| Archive | For |
|---|---|
| `git-dupe-<version>-linux-x86_64.tar` | x86-64 Linux with the GNU C library 2.39 or newer |
| `git-dupe-<version>-macos-arm64.tar` | a Mac with Apple silicon, on macOS 11.0 or newer |
| `git-dupe-<version>-macos-x86_64.tar` | an Intel Mac, on macOS 10.12 or newer |
| `git-dupe-<version>-windows-x86_64.zip` | x86-64 Windows, where the executable starts on Windows 10 or newer and git-dupe is checked on Windows 11 |

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

## On Windows

git-dupe runs on Windows 11 on x86-64, under Git for Windows 2.43.0 or newer, below
3.0.0, its `MinGit` distribution included: the `git` on `PATH` must be Git for Windows's
`git.exe`, a `.cmd` shim or another build of Git being outside what it promises. In
PowerShell, paste:

```powershell
irm https://raw.githubusercontent.com/BrianWeiHaoMa/git-dupe/main/install.ps1 | iex
```

The script, [`install.ps1`](../install.ps1) in this repository, finds the latest release,
downloads `git-dupe-<version>-windows-x86_64.zip` and its `.sha256`, checks the zip's
SHA-256 digest, unpacks it, and runs the executable once. Only then does it install the
executable as `~\.local\bin\git-dupe.exe`. It asks nothing, needs no change to the
execution policy and no elevation, and writes nothing else outside a temporary directory
it removes. A download cut short runs nothing, a failure installs nothing, and run later
the same line installs the newest release over the one installed. Where the old
`git-dupe.exe` is running, the script renames it to `git-dupe.exe.old` beside the new one
and says so, and the next run, once that one has ended, removes it.

The script changes no setting of yours. When `~\.local\bin` is not on `PATH` it prints
two lines and runs neither: one that adds the directory for this session, and one to run
once for every new shell. It also warns, and installs all the same, when the Git on
`PATH` is missing or older than 2.43.0.

By hand, download the zip from the
[releases](https://github.com/BrianWeiHaoMa/git-dupe/releases) and, in its directory,
unpack it and follow its `INSTALL-windows.txt` note, with `~\.local\bin` on `PATH`:

```powershell
Expand-Archive git-dupe-<version>-windows-x86_64.zip .
cd git-dupe-<version>
New-Item -ItemType Directory -Force ~\.local\bin
Copy-Item git-dupe.exe ~\.local\bin\
```

Any other directory on `PATH` serves the same. To check a download first, compare
`Get-FileHash -Algorithm SHA256 <zip>`, or `certutil -hashfile <zip> SHA256`, with the
digest its `.sha256` starts with. The zip's `git-dupe.exe` carries its C runtime in it,
so nothing else needs installing, and `cargo install --path .` works from a clone here
as well, with the same C runtime.

- **No `man`, and no page.** Windows has no `man` and the zip holds no manual page:
  `git dupe help` and `-h` are the documentation, and `git dupe --help` is Git's own
  failure for a page it lacks.
- **Long paths.** Past Windows's 260 characters a Git command fails with
  `Filename too long`, unless `core.longpaths` is set, a key of Git for Windows's that
  git-dupe never sets of its own accord. Every file in the dupe stands five characters
  deeper than its counterpart in `.git`.
- **Symbolic links.** Where `core.symlinks` is false, which Git for Windows's own
  configuration sets, a private symbolic link is checked out as a file holding its
  target, as Git does for any repository.

The rest of what Windows differs in is under
[Limits](how-it-works.md#limits). To uninstall, run `git dupe detach` in each project that
uses it, then delete `~\.local\bin\git-dupe.exe`, or run `cargo uninstall git-dupe` after
an install from source.

## Uninstalling

On Linux and macOS, run `git dupe detach` in each project that uses it, then delete
`~/.local/bin/git-dupe` and `~/.local/share/man/man1/git-dupe.1`, or run
`cargo uninstall git-dupe` after an install from source. [On Windows](#on-windows) ends
with its own.
