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
  `git dupe` lines `README.md` shows are those lines, and the lines it quotes from
  `INSTALL` stand as `INSTALL` holds them.
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
| `cargo test --test scenarios release_archive` | makes the release archive `git-dupe-<version>-<platform>-<architecture>.tar`, the platform `linux` or `macos` and the architecture as `uname -m` prints it (`linux-x86_64`, `macos-arm64`, or `macos-x86_64` on the machines a release ships for), beside the release executable, `target/release/` here, from a release build of the package, anew each time; checks its three members and follows its `INSTALL` into a temporary home under every listed release | a few seconds, most of them the release build after a change |
| `GIT_DUPE_CHECK_RELEASES=2.43.0,2.56.0 cargo test --test scenarios -- --skip scale` | the form the workflows run: every scenario under the oldest and the newest release, the release archive made among them, but `scale`, and so neither the counted Git runs of six commands among 100,000 files nor their one-second bound, which a shared runner's speed and load decide as much as git-dupe does | about a minute on 32 cores, four or five on an 8-core Intel Mac, once `gits/` holds the two releases |
| `cargo test` | the delivery check: every unit check, and every scenario under every listed release | about eight minutes on 32 cores once `gits/` holds the releases, the 100,000-file scenario (`scale`) the longest |
| `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` | format and lints, both required | seconds |

`GIT_DUPE_CHECK_RELEASES`, a comma-separated subset of `LISTED` in `tests/scenarios/harness/releases.rs`, the one place the code names a Git release, narrows a focused run and is never the delivery check. A scenario run first downloads and builds, into `gits/` (ignored, never committed), each release it runs under that is not there yet, all fourteen for `cargo test` and two for the workflows' form: about a minute and a half more on 32 cores for the fourteen, longer on fewer, and the network, `curl`, a `tar` that reads xz, a C compiler, `make`, and the zlib and C library headers. On Linux these come from the distribution's packages, GNU `tar` with `xz` among them; on macOS from macOS itself and the Command Line Tools for Xcode, whose clang, `make`, and SDK give the compiler, `make`, and the headers, macOS's own `tar` reading xz without a separate `xz`. The Git on `PATH` is not a check target; the scenarios of `manual_page` and `release_archive` need `man`, man-db on Linux and macOS's own on macOS; those of `release_archive` run `cargo build --release`, `tar`, and `uname -m`; and those of `worktree_filesystems` need a second local filesystem, `/dev/shm` on Linux, and on macOS a small ram disk they attach and detach with `hdiutil` and `diskutil`, without `sudo`.

On GitHub, `.github/workflows/ci.yml` runs `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --bin git-dupe`, and the form that leaves out `scale`, under the oldest and the newest release of `LISTED`, on every push to `main` and every pull request, on three runners, one for each platform and architecture a release ships for: `ubuntu-24.04` (x86-64 Linux), `macos-26` (Apple silicon), and `macos-26-intel` (Intel macOS). `.github/workflows/release.yml` runs the same on the same three before it publishes a release, each runner's `release_archive` scenarios making its archive. Neither is the delivery check, which [`RELEASING.md`](RELEASING.md) runs, `scale` with it, on Linux and on macOS before every release tag.
