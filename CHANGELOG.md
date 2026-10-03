# Changelog

Every change a user of git-dupe would notice, newest first, in the form of
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versions follow
[semantic versioning](https://semver.org/spec/v2.0.0.html) with
[`PRODUCT_SPECIFICATION.md`](PRODUCT_SPECIFICATION.md) as the contract;
[`RELEASING.md`](RELEASING.md) says which change raises which number.

## [Unreleased]

## [0.1.0] - 2026-10-03

### Added

- git-dupe, invoked as `git dupe`: a developer's private files of a project versioned
  in a second Git repository at `.git/dupe`, and kept out of the project's Git through
  rules it maintains in `.git/info/exclude`.
- Its own commands `init`, `clone`, `detach`, `hide`, `unhide`, `git`, and `help`;
  `status` and `add` acting on the hidden paths, and `clean` cleaning the project while
  sparing them; `stash` refusing the forms that would stash the whole project, and
  `push`, `pull`, `fetch`, `remote`, and `clone` refusing the project's own repository.
  Every other Git command runs against the private repository unchanged.
- Support for Git 2.43.0 or newer, below 3.0.0, checked under every minor release from
  2.43.0 to 2.56.0.
- The manual page `git-dupe(1)`: the commands, a quick start, examples, and
  explanations.
- The release archive `git-dupe-<version>.tar`: the executable, for x86-64 Linux with
  the GNU C library 2.34 or newer, the manual page, and an install note.
