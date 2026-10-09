# Changelog

Every change a user of git-dupe would notice, newest first, in the form of
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versions follow
[semantic versioning](https://semver.org/spec/v2.0.0.html) with
[`PRODUCT_SPECIFICATION.md`](PRODUCT_SPECIFICATION.md) as the contract;
[`RELEASING.md`](RELEASING.md) says which change raises which number.

## [Unreleased]

### Added

- Linked worktrees as workspaces of their own: `git dupe init` and `git dupe clone`
  attach a worktree that `git worktree add` made, of an ordinary or a bare repository,
  with its own private repository in its Git directory (`.git/worktrees/<name>/dupe`),
  its own hidden paths and history, and its own region of `.git/info/exclude`, where
  `git dupe` in a linked worktree was refused before. Private work moves between
  worktrees as between machines, and `git dupe init` run after `git worktree move`
  records the new place for plain Git. Commands in different worktrees may run at the
  same time: each waits for another's write of `.git/info/exclude` to finish, so that
  no worktree's region update is lost, and one killed while it writes the file holds
  up none. Every command that keeps its region up to date warns about a path that
  another worktree hides, while it stands in this one, is not hidden here, and the
  project's Git ignores it, naming that worktree, and `git dupe detach` names each
  formerly hidden path another worktree still hides. The region of a worktree whose
  private repository is gone, after `git worktree remove` or `prune`, is dropped by
  the next such command, or `detach`, in any worktree.

### Changed

- The first line of `git dupe help` and the manual page's quick start and examples
  present git-dupe as versioning the files of a checkout that are yours and not the
  project's, notes and an instruction file for a coding agent among them: the quick
  start runs `git dupe add notes/ AGENTS.md`, and the `-f` that a file the project
  ignores takes is shown in the examples, with `.vscode/`, and explained under FILES
  THE PROJECT IGNORES. No command changes.

### Fixed

- Where a symbolic link or a file stands at `.git/dupe`, or at a linked worktree's
  `dupe`, git-dupe no longer runs Git through it: `git dupe init`, `git dupe clone`,
  and a help request such as `git dupe commit -m -h` are refused naming it, where they
  could write into whatever repository it led to, the project's own included.
- A command that a line of git-dupe offers to run, such as `git dupe restore` after
  `clone` kept a file, or `git rm --cached` before a file becomes private, now acts on
  the path it names alone when typed as shown. A path holding a space, a quote, `$`, or
  `!` is quoted for the shell, where it was split or expanded; one beginning with `:`, or
  holding `*`, `?`, `[`, or `\`, is written so that neither Git nor git-dupe reads it as
  a pattern or pathspec magic, where `git dupe restore` could overwrite other files, and
  a Git command offered for such a path works with `GIT_LITERAL_PATHSPECS` exported.
  The alias name in the `git dupe git` command offered for an alias Git releases read
  differently is quoted the same way.

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
