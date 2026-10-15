# Changelog

Every change a user of git-dupe would notice, newest first, in the form of
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versions follow
[semantic versioning](https://semver.org/spec/v2.0.0.html) with
[`PRODUCT_SPECIFICATION.md`](PRODUCT_SPECIFICATION.md) as the contract;
[`RELEASING.md`](RELEASING.md) says which change raises which number.

## [Unreleased]

### Added

- After a private `rebase` or `am` that stopped, a `cherry-pick` or `revert` that
  stopped, a `checkout` or `switch` that left a branch for a detached `HEAD`, a
  `branch -d` that refused, a `push` from a branch with no upstream, a rejected `push`,
  a `pull` on a branch with no upstream, a `pull` that finds divergent branches with no
  way to reconcile them configured, and an `add` of a path the private repository
  ignores, `git dupe` now ends with a `hint:` naming the `git dupe` commands to go on
  with, such as `git dupe rebase --continue`, `git dupe push -u origin HEAD`, or
  `git dupe add -f <path>`, because Git's own advice there names plain `git` commands,
  which act on the project's repository. Git's output and exit status are unchanged,
  and the hint also follows such a command reached through an alias. The manual page
  says so under PLAIN GIT.

### Fixed

- `git dupe hide` of a path an ignore rule ignores, such as `.env.local` under the
  project's `.gitignore`, now also offers `git dupe add -f` for it, where its hint
  offered only the plain `git dupe add`, which Git refuses for an ignored path; for a
  path beyond a symbolic link, which Git's add does not follow, the hint names the link
  and offers no `git dupe add`.
- `git dupe status`, while nothing but `.gitdupe` is hidden, says so in its hint, where
  it said that nothing was private yet even after private files had been committed and
  then released.
- `git dupe clean`, refusing while a hidden path lies beyond a symbolic link, says that
  Git's clean does not follow the link and can delete it, the hidden path's only way to
  what stands at it, where it said that Git would delete the link and what it leads to.
- `git dupe detach` without `--force` now refuses while the commit `HEAD` names, such as
  one made on a detached `HEAD`, is on no remote-tracking branch, as it already refused
  for a commit on a branch, tag, or stash entry, where it removed the private repository
  and that commit with it.
- `git dupe init` in an attached workspace whose private repository has a sparse
  checkout (`git dupe sparse-checkout set`) now reads and records the work tree in
  `config.worktree`, where the sparse checkout moved it and where Git reads it first:
  after `git worktree move` it records the linked worktree's new place there, so that
  plain Git reads the private repository again, where it wrote the local file that
  `config.worktree` outranks; and on an attachment that was already complete it changes
  nothing, where it took the attachment for unfinished and set the identity,
  `status.showUntrackedFiles`, and `advice.statusHints` again over the developer's own
  values.
- `git dupe status` now shows the untracked and ignored files under a directory that
  stands, below a hidden path, where the project's Git tracks a file, as when a draft
  directory replaces a teammate's file, where it left out the directory and everything
  in it; the project's file itself is still not listed, and a directory holding a
  `.git` entry that stands at such a path, as a submodule's checkout does, is not
  listed as untracked.
- `git dupe push`, `pull`, `fetch`, `remote`, and `clone` now refuse a URL of the
  project's remote whose credentials hold an encoded `/`, as
  `https://reader:p%2Fss@example.com/team/project.git` does, and every other spelling
  of the project's remote when its own configured URL is spelled so, where they read
  the host from the credentials, before the `%2F`, and let the URL through. A `file://`
  URL so spelled is compared by the path after its host as well as by the path Git
  opens, which the encoded `/` moves.
- `git dupe push`, `pull`, `fetch`, `remote`, and `clone` now refuse a `file://` URL of
  the project's repository whose path holds `%00`, which Git keeps as written and
  opens, where they read it as a zero byte and let the URL through.

## [0.3.0] - 2026-10-12

### Added

- macOS support, on Apple silicon and on Intel, the default case-insensitive volume
  included. A release has an archive for each kind of Mac,
  `git-dupe-<version>-macos-arm64.tar` for macOS 11.0 or newer on Apple silicon and
  `git-dupe-<version>-macos-x86_64.tar` for macOS 10.12 or newer on Intel, each built
  and checked on a Mac of its kind, with the same manual page and install note as the
  Linux archive. On a case-insensitive volume git-dupe compares paths as spelled, so a
  path spelled in another case is another path to it, while the project's Git ignores
  a hidden path in every spelling of its ASCII case.

### Changed

- The release archive is named for the platform and the architecture it is built for,
  `git-dupe-<version>-linux-x86_64.tar` for x86-64 Linux, where its name held the
  version alone; what it holds and its install note are unchanged.
- The Linux executable is built on Ubuntu 24.04 and needs the GNU C library 2.39 or
  newer, where it needed 2.34; on an older system, install from source.

### Fixed

- `git dupe status` shows Git's status when thousands of private files below a hidden
  directory have their deletion staged, where it was refused once their paths together
  no longer fit on one command line; an untracked directory standing where they were is
  shown as one entry, as Git shows one.
- `git dupe detach` removes the private repository when a hidden directory holds
  thousands of private files whose paths together do not fit on one command line,
  where it was refused, `--force` included, after removing the region, so that the
  project's Git saw the private files until the next `git dupe` command.
- `git dupe clone` of a shallow private repository, such as one made shallow by
  `git dupe fetch --depth=1`, checks out its branch and writes the private files, the
  new private repository shallow as `git clone` of it would be, where it checked out
  nothing and said that the remote had no branch.

## [0.2.0] - 2026-10-09

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
