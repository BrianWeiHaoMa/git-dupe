# Contributing

git-dupe is small on purpose and held to two documents, the place to start before
changing it or reporting a problem.

## What governs a change

- [`PRODUCT_SPECIFICATION.md`](PRODUCT_SPECIFICATION.md) outranks everything else in
  this repository, [`TECHNICAL_SPECIFICATION.md`](TECHNICAL_SPECIFICATION.md) ranks
  second, and the code follows both. Code, checks, and documents cite their lines by
  label (`G6`, `R3`).
- A change of behavior amends `PRODUCT_SPECIFICATION.md` first, then
  `TECHNICAL_SPECIFICATION.md` where it says how the behavior holds, then the code. A
  new command, option, environment variable, configuration key, or file of git-dupe's,
  or a crate dependency, is such an amendment (R7): git-dupe is Rust's standard library
  alone.
- The help texts (`src/front/help/`) and the manual page's own texts
  (`src/front/page/`) change in the same change as any behavior they describe (R7).
  Every command line the page's texts show is run as written by a scenario; the
  `git dupe` lines `README.md` shows are those lines (`readme`); the two install lines
  `README.md` and `docs/install.md` show run `install.sh` and `install.ps1` as a scenario
  runs them (`install`, `install_powershell`), and the lines `docs/install.md` quotes from
  `INSTALL` and `INSTALL-windows.txt` stand as the notes hold them (`install_lines`);
  every `git dupe` line of a `sh` or `powershell` block in a page under `docs/` is a
  command line of the page's texts or a whole command in their prose (`docs`).
- What git-dupe gives Git or reads from it exists in Git 2.43.0 and is unchanged through
  the newest release the checks list, apart from the differences
  `TECHNICAL_SPECIFICATION.md` records; a doubt is settled in the source of both (R6).
- The code has one directory under `src/` per part of `TECHNICAL_SPECIFICATION.md`'s
  `Composition`. The rules of a part, and how to write a scenario, are in the doc at the
  top of the file that owns them: `src/front.rs`, `src/keeper.rs`, `src/guards.rs`,
  `src/runner.rs`, `tests/scenarios/main.rs`, and so on.
- A change is done when the checks below pass, the delivery check among them, with no
  check weakened, skipped, or deleted to get there.

## Reporting a problem

Reports are welcome. The most useful one starts from use: something you ran that did not
go as `PRODUCT_SPECIFICATION.md` says. Open an issue that gives the commands that show it
from an empty directory, the Git version (`git --version`), what happened, and what you
expected, with the label of the line you expected it from, as `G16`.

The specification is written in words, and words can be read more than one way. Its
guarantees hold inside its `Operating envelope`, not beyond it, and its `Non-goals` name
what git-dupe leaves alone. A guarantee that fails in ordinary use is a defect, repaired
in the code. A report that turns on a rare path, or on a reading the line allows but was
not written for, is more often settled by amending the line to say plainly what it means
than by changing the code. A line that is unclear, or that no code could keep as written,
is worth an issue too: name it, and say how you read it.

## Checks

From the root. This is their one list:

| Command | Runs | Cost |
|---|---|---|
| `cargo test --bin git-dupe` | the unit checks | under a second |
| `GIT_DUPE_CHECK_RELEASES=2.43.0,2.56.0 cargo test --test scenarios hide` | scenarios whose name holds `hide`, under the oldest and the newest release | about a second |
| `cargo test --test scenarios hide` | the same under every listed release | seconds |
| `cargo test --test scenarios release_archive` | makes the release archive `git-dupe-<version>-<platform>-<architecture>.<extension>`, the platform `linux`, `macos`, or `windows`, the architecture as `uname -m` prints it (`linux-x86_64`, `macos-arm64`, `macos-x86_64`, or `windows-x86_64` on the machines a release ships for), and the extension `tar`, or `zip` on Windows, with its `.sha256` beside it, beside the release executable, `target/release/` here, from a release build of the package, anew each time; checks its members, on Windows that the executable names no dynamic C runtime, and follows its install note into a temporary home under every listed release | a few seconds, most of them the release build after a change |
| `GIT_DUPE_CHECK_RELEASES=2.43.0,2.56.0 cargo test --test scenarios -- --skip scale` | the form the workflows run: every scenario under the oldest and the newest release, the release archive made among them, but `scale`, and so neither the counted Git runs of six commands among 100,000 files nor their one-second bound, which a shared runner's speed and load decide as much as git-dupe does | about a minute on 32 cores, four or five on an 8-core Intel Mac, once `gits/` holds the two releases |
| `cargo test` | the delivery check: every unit check, and every scenario under every listed release | about eight minutes on 32 cores, half an hour on an 8-core Intel Mac, a hundred minutes on 16 cores on Windows, once `gits/` holds the releases, the 100,000-file scenario (`scale`) the longest |
| `set GIT_DUPE_CHECK_RELEASES=2.43.0,2.56.0&& cargo test --test scenarios -- --skip scale`, in `cmd` on Windows | the workflows' form on Windows: every scenario under the oldest and the newest release, the Windows archive made among them, but `scale`; a part Windows cannot hold reports itself unavailable, naming why | eleven to fourteen minutes on 16 cores |
| `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` | format and lints, both required | seconds |

`GIT_DUPE_CHECK_RELEASES`, a comma-separated subset of `LISTED` in `tests/scenarios/harness/releases.rs`, the one list of the Git releases the checks run under, narrows a focused run and is never the delivery check. A scenario run first downloads and builds, into `gits/` (ignored, never committed), each release it runs under that is not there yet, all fourteen for `cargo test` and two for the workflows' form: about a minute and a half more on 32 cores for the fourteen, longer on fewer, and the network, `curl`, a `tar` that reads xz, a C compiler, `make`, and the zlib and C library headers. On Linux these come from the distribution's packages, GNU `tar` with `xz` among them; on macOS from macOS itself and the Command Line Tools for Xcode, whose clang, `make`, and SDK give the compiler, `make`, and the headers, macOS's own `tar` reading xz without a separate `xz`. The Git on `PATH` is not a check target; the scenarios of `manual_page`, `release_archive`, and `install` need `man`, man-db on Linux and macOS's own on macOS; those of `release_archive` and `install` run `cargo build --release`, `tar`, and `uname -m`, and those of `install` also run `install.sh` under `sh` with `curl`, `mktemp`, and `shasum` or `sha256sum`, against a server of their own on `127.0.0.1`; and those of `worktree_filesystems` need a second local filesystem, `/dev/shm` on Linux, and on macOS a small ram disk they attach and detach with `hdiutil` and `diskutil`, without `sudo`. On Windows no release is built: a scenario run downloads each release's `MinGit` zip with `curl.exe`, checks it against the digest `MINGIT` in the same file pins, and unpacks it with Windows's own `tar.exe`, both of which Windows ships, a few seconds a release; the machine needs Developer Mode or an elevated session, because making a symbolic link needs one of the two, and `TMP` and `TEMP` at a short directory such as `C:\t`, because every file in a private repository stands deeper than its counterpart in `.git`; those of `release_archive` run `cargo build --release` and pack with Windows's own `tar.exe`, reading the architecture from the platform and running no `uname`; those of `install_powershell` run `install.ps1` as the pasted line runs it, under Windows PowerShell 5.1, against the same server of their own, and report themselves unavailable off Windows, as those of `install` do on it; those of `worktree_filesystems` need a second fixed NTFS volume under a drive letter and another mounted at a folder of the first drive with no letter, and report a way the machine lacks unavailable.

On GitHub, `.github/workflows/ci.yml` runs `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --bin git-dupe`, and the form that leaves out `scale`, under the oldest and the newest release of `LISTED`, on every push to `main` and every pull request, on four runners, one for each platform and architecture a release ships for: `ubuntu-24.04` (x86-64 Linux), `macos-26` (Apple silicon), `macos-26-intel` (Intel macOS), and `windows-2025` (x86-64 Windows Server), which also has `TMP` and `TEMP` at `C:\t`. `.github/workflows/release.yml` runs the same on the same four before it publishes a release, each runner's `release_archive` scenarios making its archive. Neither is the delivery check, which [`RELEASING.md`](RELEASING.md) runs, `scale` with it, on Linux, on macOS, and on Windows 11 before every release tag.
