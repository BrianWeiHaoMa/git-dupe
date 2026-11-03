# Changelog

Every change a user of git-dupe would notice, newest first, in the form of
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versions follow
[semantic versioning](https://semver.org/spec/v2.0.0.html) with
[`PRODUCT_SPECIFICATION.md`](PRODUCT_SPECIFICATION.md) as the contract;
[`RELEASING.md`](RELEASING.md) says which change raises which number.

## [Unreleased]

### Fixed

- `git dupe clean -X` without `-d` no longer deletes untracked files under a hidden
  directory spelled in another case than `.gitdupe` spells it, on macOS and on Windows,
  while the project tracks a file there, nor through a Windows junction standing at a
  hidden path into which the project tracks a file. While the project tracks such a path,
  the ignored files beside a hidden directory, in a directory no rule names, are deleted
  where plain `git clean -X` keeps them with it.
- While `.gitdupe` cannot be read, as a file Windows PowerShell 5.1's `>` wrote cannot,
  every command leaves the exclude rules as they stood, so that what they hid stays
  hidden, and `git dupe clean` is refused, naming the file, where it used to delete what
  the file listed.
- `git dupe init` and `git dupe clone` under a template whose `config` includes another
  file leave that file's `user.name`, `user.email`, `status.showUntrackedFiles`, and
  `advice.statusHints` in force, where the keys written after the include outranked them.
- The `clone` help and the page say `git dupe restore <path>`, which replaces a kept file,
  where they said `git dupe restore`, which restores nothing.
- `git dupe add notes/` stages the private changes under `notes` and exits 0 when a public
  file there matches an ignore rule, where it exited 1 and staged nothing over it.
- `git dupe add notes/ --chmod`, its value missing, is Git's usage error and hides nothing.
- After a `git dupe add` that names nothing, a hint names `git dupe add .`, where Git's
  advice names plain `git add .`, which stages the whole project. Where nothing but a
  privately tracked `.gitdupe` is hidden, the hint also names `git dupe add .gitdupe`.
- The refusal while the project tracks `.gitdupe` says to run `git rm --cached .gitdupe`
  from the root.
- After a private `pull --rebase` stopped at a conflict, `git dupe` names the private
  `rebase --continue`, `--skip`, and `--abort`, where Git's advice names plain commands
  that act on a rebase paused in the project's repository.
- After `git dupe switch -m` or `checkout -m` that leaves conflicts, a hint names the
  private `add`, `stash drop`, `reset --hard`, and `stash pop`, where Git from 2.55.0 keeps
  the local change as a stash entry and names plain commands.
- The hint after an amend that would leave the commit empty names `--allow-empty` added to
  the command as typed, so that `--only <path>` stands, and it follows `--am`, `--ame`, and
  `--amen`, which Git reads as `--amend`.
- The hint after a rejected `git dupe push <remote> <branch>` says the pull runs on the
  local branch pushed, switching to it first, where it used to bring the commits into
  whichever branch was checked out.
- The hint after a `push` from a detached `HEAD` writes `HEAD:refs/heads/<branch>`, which a
  tag of the branch's name on the remote does not make ambiguous.
- The hint at a stopped rebase names `git dupe commit --allow-empty` for a commit the replay
  left empty.
- The refusal of a private remote group names `git dupe config --unset-all`, because
  `--unset` fails on a key with several values.
- The lines about a hidden path beyond a symbolic link say that public Git was not asked
  about it, and no longer what Git does there, which was false of a Windows junction.

## [0.4.0] - 2026-11-01

### Added

- A release now ships `git-dupe-<version>-windows-x86_64.zip`, holding `git-dupe.exe`,
  which needs no C runtime installed, and the install note `INSTALL-windows.txt`, beside
  the three tars, and `install.ps1` installs it with one line pasted into PowerShell,
  `irm https://raw.githubusercontent.com/BrianWeiHaoMa/git-dupe/main/install.ps1 | iex`:
  it checks the zip against its published SHA-256 digest, puts the executable in
  `~\.local\bin`, prints the line that puts that directory on `PATH`, and changes no
  setting of yours. `docs/install.md` and the limits in `docs/how-it-works.md` say what
  Windows users do and what Windows differs in.

### Changed

- The scenarios now run on Windows too, under Git for Windows's `MinGit` builds of the
  listed releases, each part Windows cannot hold reporting itself unavailable and naming
  why, so that git-dupe's behavior there is pinned by the same checks as on Linux and
  macOS, on every push on a `windows-2025` runner, which also makes the Windows archive.
- The repository's `.gitattributes` keeps every checkout's bytes as committed, so that a
  build from a clone under `core.autocrlf=true`, which Git for Windows ships, embeds the
  help texts with LF line endings.
- Commands in different worktrees now wait for each other on one empty lock file,
  `.git/git-dupe.lock`, where they waited on the `.git` directory itself. The first
  command that needs it makes it, nothing removes it, `git dupe detach` leaves it, and
  Git never reads it.
- `.gitdupe` is now read with a UTF-8 byte order mark at the start of the file and one
  carriage return at the end of a line dropped, so that a file Git checks out with CRLF
  line endings under `core.autocrlf=true`, or an editor saves so, hides what it listed,
  and `hide` and `unhide` keep the rest of it byte for byte. A file that begins with a
  UTF-16 byte order mark, as Windows PowerShell 5.1's `>` writes one, lists nothing and
  is named in a `warning:`, and `hide`, `unhide`, and a hiding `add` refuse to write it
  until it is saved as UTF-8.
- A destination whose host is one letter, such as `c:proj`, is now also compared as a
  local path, and a `file:///C:/proj` destination as the path `C:/proj`, because Git for
  Windows opens them as paths: `push`, `pull`, `fetch`, `remote`, and `clone` refuse one
  that names the project, naming `git dupe git`.
- `git dupe init` run again after a killed `init` now copies the template files the kill
  left missing, where a repository that had a `HEAD` was completed without them. A file
  the kill left partly copied stays as it is.
- `git dupe clone` now fetches with the refspec `git remote add` records, so that a
  `remotes.origin` group in the developer's configuration ends it with Git's own
  refusal, status 128, the workspace attached, where it fetched into no tracking ref and
  reported a remote with no branch.

### Fixed

- `git dupe pull --rebase=interactive` that stops with nothing unmerged, at an `edit` for
  one, where Git's own advice names plain `git commit --amend`, now ends with the same
  `hint:` as `git dupe rebase` stopped there, naming the `git dupe` commands.

## [0.3.3] - 2026-10-30

### Added

- An install script: `curl -fsSL https://raw.githubusercontent.com/BrianWeiHaoMa/git-dupe/main/install.sh | sh`
  installs the latest release on x86-64 Linux and on macOS, putting the executable in
  `~/.local/bin` and the manual page in `~/.local/share/man/man1` as the archive's
  `INSTALL` note does, once the archive matches its published SHA-256 digest; it
  installs nothing when a step before the copies fails, and says what to add to `PATH`
  when `~/.local/bin` is not on it. `docs/install.md` describes it beside the archives,
  installing from source, the Git git-dupe needs, and uninstalling, which the README's
  Install section now points to.

### Changed

- The README is now a short page for a reader who knows what git-dupe is: what it is,
  the demo, the install line, the quick start, the few things to know, and where to read
  more. What it said beyond that is in `docs/how-it-works.md` (what git-dupe runs and
  writes, its limits, the specification it is built to), `docs/use-cases.md` (what it is
  for, and how it compares with other tools), and `docs/questions.md`. The demo image
  names beside each command the repository it acts on, the public or the private one.

## [0.3.2] - 2026-10-18

### Added

- After a private `merge`, `rebase`, or `pull` whose autostash, asked for by
  `--autostash` or an `autoStash` setting such as `merge.autoStash`, came back with
  conflicts, which Git ends with status 0 and keeps as a stash entry, `git dupe` now
  ends with a `hint:` naming `git dupe add <path>` and `git dupe stash drop` to keep the
  changes, or `git dupe reset --hard` and later `git dupe stash pop` to set them aside,
  because Git's own advice there names plain `git stash drop` and `git stash pop`, and in
  recent releases `git reset --hard`, which act on the project's repository. Git's
  output and exit status are unchanged, and the manual page says so under PLAIN GIT.
- After a private `commit --amend` without `--allow-empty` that exits 1, as one that
  would leave the commit empty does, `git dupe` now ends with a `hint:` naming
  `git dupe commit --amend --allow-empty` and `git dupe reset HEAD^`, and after a
  private `commit` that exits 1 while a `cherry-pick` is stopped, as one does once the
  pick has become empty, with the hint of the stopped `cherry-pick`, naming
  `git dupe commit --allow-empty` and `git dupe cherry-pick --skip`, or, where a rebase
  stands stopped there too, as at a pick that became empty, with the hint of the stopped
  rebase, naming `git dupe rebase --skip`, because Git's own advice after the amend
  names plain `git reset HEAD^`, and after a commit during an empty pick plain
  `git commit --allow-empty` and `git cherry-pick --skip` or `git rebase --skip`, which
  act on the project's repository. Git's output and exit status are unchanged.
- After a private `push` given no refspec that Git refuses on a branch whose upstream
  has another name than the branch, under `push.default` unset or `simple`, `git dupe`
  now ends with a `hint:` naming `git dupe push <remote> HEAD:<upstream>` and
  `git dupe push <remote> HEAD`, and after one that Git refuses on a detached `HEAD`,
  with a hint naming `git dupe push <remote> HEAD:<branch>`, because Git's own advice
  there names plain `git push`, which acts on the project's repository. Git's output and
  exit status are unchanged.
- After a private `pull` given no refspec that exits 1 on a detached `HEAD`, `git dupe`
  now ends with a `hint:` naming `git dupe pull <remote> <branch>`, and after one told
  to fast-forward only, by `--ff-only` or `pull.ff=only`, that Git refuses, as it does
  where the branches have diverged, with a hint naming
  `git dupe merge --no-ff FETCH_HEAD`, which merges what the pull fetched where it was
  given no `--append`, and `git dupe rebase FETCH_HEAD`, which rebases onto it where the
  pull also fetched one branch from one remote alone, because Git's own advice there
  names plain `git pull`, or `git merge --no-ff` and `git rebase`, which act on the
  project's repository. Git's output and exit status are unchanged.

### Fixed

- `git dupe clean` given a pathspec that is not a literal path, one holding `*`, `?`,
  `[`, or `\`, or beginning with `:` other than `:/` alone, such as `:(exclude)notes/x`,
  `:/!notes/x`, or `cache?`, is now refused, deleting nothing, while a hidden path lies in a directory
  that is not hidden and under which the project tracks nothing, where Git's clean could
  delete that directory whole, the never-committed private files in it included. The
  refusal names the pathspec, the directory, and the hidden path; literal pathspecs and
  `-e` patterns run as before, and the manual page explains the refusal under REFUSED
  COMMANDS.
- `git dupe detach` without `--force` now decides whether a private commit is on a
  remote-tracking branch over the history as stored, replace refs (`git replace`) and a
  grafts file (`info/grafts`) set aside, as a remote holds it: a commit that only a
  replace ref or a grafts line grafts under pushed history is refused as unpushed, where
  it was removed with the private repository, and pushed history that a replace ref or a
  grafts line cuts away from its descendants no longer refuses.
- `git dupe push`, `pull`, and `fetch` are now refused, sending and fetching nothing,
  while a member of a private remote group, a word of the value of any `remotes.<group>`
  key, names the project's repository or a remote of it, whatever the command's own
  words name, the refusal naming the key and `git dupe config --unset remotes.<group>`:
  `git dupe fetch <group>` and `pull <group>` with two or more members, and from Git
  2.55.0 `git dupe push <group>`, ran against each member unguarded.
- `git dupe pull` given `-j` or `--jobs` with no value attached, which Git hands to
  `fetch` ahead of the repository word, so that `fetch` takes that word as its number of
  jobs and fetches from the default remote, as for `git dupe pull --jobs --ff 4`, is now
  refused, fetching nothing, where that default remote names the project's repository
  or a remote of it; it ran unguarded where `pull` kept a word of its own between the
  two. So is a `git dupe pull` given a word beginning with `-` after `--`, as
  `git dupe pull -- -q`, which `pull` hands to `fetch` without the `--`, so that `fetch`
  reads it as an option of its own and takes the default remote where no later word
  names one: it is refused where that default remote names the project's repository or
  a remote of it, whatever the later words name; it ran unguarded.
- On a branch with no upstream, a `git dupe push <remote>` with no refspec that Git
  refuses, where `<remote>` is the branch's `branch.<name>.remote`, or, where that is
  unset, the only remote the private configuration defines, `<remote>` counted among
  them, and `origin` where it defines several, now ends with the `hint:` naming
  `git dupe push -u <remote> HEAD` that `git dupe push` already gets, and a failed
  `git dupe pull <remote>` with no refspec, where `<remote>` is the branch's
  `branch.<name>.remote`, with the hint naming `git dupe pull <remote> <branch>` and
  `git dupe branch --set-upstream-to`, because Git's own advice there names plain
  `git push --set-upstream` or `git pull`, which act on the project's repository. On a
  branch with no commit yet, the hint naming `git dupe push -u <remote> HEAD` now names
  `git dupe commit` first, because that push has nothing to push before it, and it now
  also follows on a branch with `branch.<name>.merge` set and no
  `branch.<name>.remote`, which Git's push counts as having no upstream. The hint after
  a `git dupe pull` on a branch with no upstream now follows there too, which Git's pull
  counts so as well, and only where Git exits 1: given a URL or a path as the branch's
  remote, Git has the remote's `HEAD` to merge and, where it cannot, exits 128, and the
  hint of a pull that cannot reconcile divergent branches follows instead. Git's output
  and exit status are unchanged.
- A private `rebase -i` that stops at an `edit` or a `break`, which Git ends with status
  0, now ends with the `hint:` a stopped rebase gets, and that hint now also names
  `git dupe commit --amend` and `git dupe rebase --edit-todo`, because Git's own advice
  at an `edit` stop, a reword whose editor failed, or a todo list Git cannot read names
  plain `git commit --amend` or `git rebase --edit-todo`, which act on the project's
  repository. The hint of a stopped `rebase` or `am` now also follows any run of that
  command that leaves its stop standing, whatever its exit, as
  `git dupe rebase --show-current-patch` or `git dupe am --show-current-patch` does with
  status 0. A private `am` run while a rebase is stopped, and a `rebase` run while an
  `am` is stopped, no longer end with the hint of the other, whose commands fail there,
  and a `cherry-pick` or `revert` run while a rebase is stopped no longer ends with the
  hint of a stopped `cherry-pick` or `revert`, whose commands leave a rebase stopped at
  a pick that became empty as it was. Git's output and exit status are unchanged.
- A `git dupe revert` given `-n`, alone or in a bundle of short options such as `-sn`,
  or `--no-commit`, that stops on a conflict no longer ends with the `hint:` naming
  `git dupe revert --continue`, which commits what `--no-commit` was given to leave
  uncommitted; Git's own advice there names only `git add` and `git rm`. The hint after
  a stopped `revert` now also names `git dupe commit --allow-empty`, as the one after a
  stopped `cherry-pick` does.
- The `hint:` after a rejected `git dupe push` given a repository now names that
  repository, as `git dupe pull <remote> <branch>` and `git dupe fetch <remote>`, where
  it named the plain `git dupe pull` and `git dupe fetch`, which take the branch's
  upstream. A word holding `=`, as `--recurse-submodules=check`, and a negated option,
  as `--no-rebase`, are no longer read as taking the next word as their value, so that
  `git dupe push --recurse-submodules=check origin main` and
  `git dupe pull --no-rebase origin main` are read as Git reads them, given the
  repository `origin`.
- The `hint:` after a `git dupe checkout` or `switch` that leaves a branch for a detached
  `HEAD` now says that `git dupe switch -` goes back to the branch left only where that
  branch has a commit: from a branch with none yet, it has nothing to go back to.
- The `hint:` naming `git dupe branch -D` after a `git dupe branch -d` that refuses now
  also follows an abbreviation of `--delete` that Git reads as it, as `--del` before Git
  2.56.0, whose `branch` also has `--delete-merged` and refuses the abbreviation as
  ambiguous, exit 129, hinting nothing, and `d` in a bundle of short options, as `-vd`,
  where Git's own advice names plain `git branch -D`, which acts on the project's
  repository.

## [0.3.1] - 2026-10-16

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
  listed as untracked unless the deletion of a private file at that same path is
  staged.
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
