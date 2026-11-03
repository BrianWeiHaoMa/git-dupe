# How git-dupe works

What git-dupe runs, where your files' history lives, what it writes, and what it does
not do: more than the [README](../README.md) says, less than the specifications.
[What it is for](use-cases.md) has the uses, [Questions](questions.md) the usual
doubts, and [Installing](install.md) the ways to install it.

"Dupe" as in a duplicate of your checkout that only you see: a second repository laid
over the same working tree, each path belonging to one repository or the other. It has
nothing to do with duplicate files.

## If you know Git, you know git-dupe

git-dupe is Git, run against the dupe. `git dupe commit`, `diff`, `log`, `blame`,
`branch`, `switch`, `merge`, `rebase`, `stash`, `tag`, `remote`, `push`, `pull`,
`restore`, `rm`, `config`, `bisect`: each, the few refused forms below aside, is Git's
own command with Git's own options, output, pager, editor, hooks, aliases, and exit
status, on the paths of the working tree and the revisions of your own history, and a
branch switch in one repository never moves the other. Git's global options go before
`dupe`, as in `git -C project dupe status` and `git --no-pager dupe log`, and plain Git
reads the dupe too: `git --git-dir=.git/dupe log`. What there is to learn fits the one
screen of `git dupe help`:

- **Seven commands added.** `init` and `clone` attach the dupe and `detach` removes it;
  `hide` and `unhide` edit the list of hidden paths; `git` runs Git against the dupe with
  your words and no guard; `help` prints the texts.
- **Three commands changed** to fit a shared working tree. `status` and `add` act on the
  hidden paths instead of every file of the project, and `clean` cleans the project while
  sparing them.
- **A few forms refused.** `stash -u` and `-a`, which would stash the whole project, and
  a `push`, `pull`, `fetch`, `remote`, or `clone` naming the project's repository or any
  of its remotes. `git dupe git <words>` is the route to any of them when you mean it.

A day's work, from the root, is the commands you already type, after `git dupe`:

```sh
git dupe status                        # what changed under the hidden paths
git dupe add -A                        # stage it as git add -A does, under them alone
git dupe commit -m "Notes of the day"
git dupe pull                          # take what your other machines pushed
git dupe push                          # send what this one committed
```

## The dupe and the hidden paths

After the [quick start](../README.md#quick-start), the project holds:

```text
project/
├── .git/dupe/     the dupe: the second repository
├── .gitdupe       yours: lists notes, the directory you hid
├── AGENTS.md      yours
├── notes/         yours
├── README.md      the project's
└── src/           the project's
```

`git status` lists none of your files, `git log` shows none of your commits, and
`git dupe status` lists none of the project's.

- The **dupe** is an ordinary Git repository inside the project's own `.git`, one per
  worktree: `.git/dupe` in the main worktree, and `.git/worktrees/<name>/dupe` in a
  linked one, whose working tree is that worktree's. The help texts and the manual page
  call it the private repository: private to you, as a repository of your own is, and
  not encrypted. Plain Git can read it, and clone, fetch, or push from it. It has its own
  object store, which the project's Git never opens, so the project's `git gc` and
  `git prune` leave it alone; and every Git process git-dupe runs against the dupe has
  the object-directory variables cleared, so a hook or alias of the project cannot
  redirect your objects into the project's store. Being inside `.git`, it is never
  committed and never cloned with the project; `git dupe clone` is the deliberate way
  back.
- A **hidden path** is one the project's Git ignores through rules git-dupe maintains in
  `.git/info/exclude`, so that `git status`, `git add -A`, and a public commit never see
  it. The hidden paths are `.gitdupe`, a file of yours at the root that lists the paths
  you hide, one per line; every path it lists, or, while it is missing from disk and
  staged, its staged version lists; and every file the dupe tracks, as a file you add
  is. Nothing versioned is added to the project.
- Three commands are changed to fit a shared working tree: `git dupe status` and
  `git dupe add` act on the hidden paths instead of every file of the project, and
  `git dupe clean` cleans the project while sparing them. `git dupe stash -u` and `-a`,
  which would stash the whole project, are refused, and so are a `push`, `pull`,
  `fetch`, `remote`, or `clone` naming the project's repository or any of its remotes.
  Every other Git command, `commit`, `diff`, `log`, `restore`, `switch`, `merge`, runs
  against the dupe unchanged.
- **What it writes.** Of its own accord, git-dupe writes the worktree's dupe, its own
  marked region of `.git/info/exclude` (and `.git/info` when it is missing, and the
  removal of a region whose worktree's dupe is gone), the empty lock file
  `.git/git-dupe.lock`, which it never removes and under which the worktrees of a
  project take turns at that file, and `.gitdupe`; `git dupe clone`
  also writes the files it checks out, and `git dupe clean` runs your `git clean`,
  never deleting a hidden path, or refuses the few forms that could take one. Beyond
  that, a Git command writes where your words tell Git to. No hook, no daemon, no edit
  to `.gitignore` or to the project's configuration, no dependency beyond Rust's
  standard library, and the only program it runs is `git`.

Your history lives in `.git/dupe`, or, in a linked worktree, in that worktree's own Git
directory under `.git/worktrees/`. Delete the clone, or remove the worktree, and
unpushed history goes with it, so push first. `git dupe detach` is the way out: it
leaves every file on disk, and refuses, without `--force`, while a change is uncommitted
or a commit of a branch, a tag, a stash entry, or `HEAD` is on no remote.

## Limits

- **Paths are compared as spelled, on a case-insensitive volume too.** Such a volume,
  macOS's default and NTFS, makes one difference: git-dupe compares paths as spelled, so
  a path spelled in another case is another path to it, while the project's Git ignores
  a hidden path in every spelling of its ASCII case.
- **Git 2.43.0 or newer, below 3.0.0.** No single feature needs it; it is the floor the
  suite covers, and Ubuntu 24.04's stock Git. [Installing](install.md#git) says what
  older distributions ship.
- **Nothing is encrypted.** The dupe's remote is an ordinary Git remote: what you push
  is stored there as you pushed it, for whoever can read that repository. Push to one
  you would trust with the files, and keep out what you would not push to any Git
  repository, credentials and keys above all, or encrypt them before you add them.
- **Plain `git` is not guarded.** To the project's Git a file of yours is an ignored
  file: `git clean -x` deletes it, `git stash -a` stashes it, `git add -f` makes it
  public, and a public pull or checkout that brings a file to its path overwrites it.
  Commit first; `git dupe restore .`, from the root, brings back every file the dupe
  tracks as last staged, and `git dupe clean` cleans the project while sparing them.
  Public `git stash -u`, `git add -A`, and `git add .` leave them alone, as they leave
  any ignored file.
- **A private pull can overwrite a file the project ignores.** A file the project
  ignores that the dupe does not track yet on this machine is overwritten without a
  word, as Git overwrites any ignored file, when a private `pull`, `switch`,
  `checkout`, `merge`, or `rebase` brings a file to its path from another machine.
  `git dupe clone` keeps such files, and Git's own `switch`, and `checkout` of a
  branch, refuse to overwrite them under `--no-overwrite-ignore`.
- **The remote guard compares names.** `push`, `pull`, `fetch`, `remote`, and `clone`
  are refused when a word names the project's repository, one of its remotes, or one of
  its worktrees, and `push`, `pull`, and `fetch` also when a remote of the dupe's
  configuration does. A URL Git rewrites through `url.<base>.insteadOf`, a path
  beginning with `~` and a user's name, and a remote defined by a file in a `remotes` or
  `branches` directory are not seen through; nor, on Windows, are a UNC path to a local
  drive, a `/<path>` that Git for Windows's shell reads below Git's own installation, and
  a public remote reached through that shell's `/proc`.
- **One command at a time in a worktree.** git-dupe commands in one worktree must not
  overlap each other or a public Git command that writes to that worktree, and one
  person or agent works in a worktree at a time; nothing is promised when they do.
  Commands in different worktrees may run at the same time.
- **Each worktree on its own.** A linked worktree is attached by its own `git dupe init`
  or `git dupe clone`, and has its own dupe, hidden paths, and history. Its exclude rules
  apply in every worktree, since `.git/info/exclude` is shared: a path one worktree hides
  is ignored by the project's Git in the others too. `git worktree remove` and
  `git worktree prune` delete a linked worktree's dupe with it, whatever files of yours
  stand on disk, so push first.
- **Up to 1,000 hidden paths, a few thousand files.** That is the envelope the suite
  covers, the hidden paths not below another one taking at most 128,000 bytes together.
  They are passed to Git on one command line; a list that does not fit is refused,
  naming the count, never truncated. Files below a hidden directory do not count toward
  the paths: `notes/` is one path.

- **An end forced on git-dupe alone leaves Git running.** A terminal's Ctrl-C reaches
  git-dupe and its Git child together and ends both. A signal sent to the git-dupe
  process alone by its process number, or `TerminateProcess` on Windows, ends git-dupe at
  once with that end's status, and the Git command it was waiting on runs on to its own
  end, as the child of any launcher not told to stop does, so that a caller who ended it
  that way cannot take its end for Git's.

### On Windows

The commands of a day's work hold in PowerShell, `cmd`, and Git Bash. Windows differs in
these, where keeping Linux's and macOS's behavior is impossible or out of proportion:

- Its command line holds fewer hidden paths: their paths and those of other worktrees'
  regions take at most 32,000 UTF-16 code units together, each path counted with 15
  more, 2 more when it holds a space, and 1 more for each `"` in it and each character
  its exclude rule escapes, which is 17,000 units of paths at 1,000 paths that hold no
  space and no such character.
- `.gitdupe` is read as UTF-8, its byte order mark and CRLF line endings dropped. What
  Windows PowerShell 5.1's `>` writes is UTF-16: that file cannot be read, every command
  says so in a warning and leaves the exclude rules as they stood, so that what they hid
  stays hidden, and `git dupe hide`, `unhide`, and `clean` refuse until it is saved as
  UTF-8.
- A name stored in an index or in `.gitdupe` that is not valid UTF-8 cannot stand on a
  command line: a command that would need one there is refused, naming the path, and
  otherwise it is named in a warning.
- There is no `man` and no page: `git dupe help` and `-h` are the documentation, and
  `git dupe --help` is Git's own failure for a page it lacks.
- A Ctrl-C ends a command with no particular exit status.
- An NTFS junction counts as a symbolic link: a hidden path below one is warned about
  and `git dupe clean` refuses, though Git walks a junction as a directory. A path of
  another worktree's hidden paths that lies below one is not warned about.
- Where `core.symlinks` is false, a private symbolic link is checked out as a file
  holding its target, as Git does for any repository.
- A path past 260 characters fails in Git unless `core.longpaths` is set, which
  git-dupe does not do of its own accord, and every file in the dupe stands five
  characters deeper than its counterpart in `.git`.
- `cmd` and PowerShell do not expand `*`, so `git dupe add notes/*.md`, which Git Bash
  expands, is a usage error in them. `cmd` does not read the `"$(…)"` of the command
  that attaches a linked worktree. A command a git-dupe line offers is quoted for a
  POSIX shell, as Git Bash reads it.
- Under Git for Windows 2.43 through 2.47, a Git command in another worktree that opens
  `.git/info/exclude` at the moment git-dupe replaces it can fail with Git's own
  message that it cannot use that file, exit 128, and succeeds when run again; so can
  git-dupe's own question of which hidden paths are exposed, which then says in a
  warning that it could not be checked and is asked again by the next command. From
  2.48 on Git reads the old file or the new.
- Through a dangling symbolic link at `.git/info/exclude`, every Git for Windows command
  that reads the file fails, so that a git-dupe command warns that exposure could not
  be checked, where Linux's and macOS's Git answer.
- In `git dupe status`, a path the project tracks under the hidden paths whose spelling
  holds no ASCII byte other than `/` is left out with everything below it, where on
  Linux and macOS a staged deletion or an untracked file below it stays shown.

## Built to a specification

git-dupe is new, and a tool that writes inside `.git` has to earn trust, so it is held to
a specification. [`PRODUCT_SPECIFICATION.md`](../PRODUCT_SPECIFICATION.md) says, in
numbered lines, what git-dupe does, the guarantees it keeps, and what it never does;
[`TECHNICAL_SPECIFICATION.md`](../TECHNICAL_SPECIFICATION.md) says how each guarantee
holds. The code follows both, and a change of behavior amends the specification first.
The scenario suite, the specification's own `Done when` among it, runs on every push on
Linux, on an Apple silicon Mac, on an Intel Mac, and on Windows Server, under Git 2.43.0
and 2.56.0, the oldest and the newest release it lists
([CI](https://github.com/BrianWeiHaoMa/git-dupe/actions/workflows/ci.yml)), all but
its timing of six commands among 100,000 files, which a shared runner decides as much
as git-dupe does, and in full before each release, on Linux, on macOS, and on Windows 11,
under every minor release between them, fourteen at this writing, each built from source
on Linux and macOS, and on Windows the `MinGit` build of Git for Windows unpacked in
place of one, a part of a check Windows cannot hold reporting itself unavailable and
naming why.

**How it was built.** I wrote the product specification and own every decision in it:
the command surface, what is refused, what is left to plain Git, and the tradeoffs
behind each. Coding agents wrote most of the Rust and the scenarios, to those two
documents and under my direction, and I reviewed each change against them before it
landed. The rules they worked under are the ones in [`CONTRIBUTING.md`](../CONTRIBUTING.md):
one directory of code per part of the technical specification's `Composition`; code,
checks, and documents citing the lines they serve by label; a change of behavior
amending the specification before it touches the code; and no check weakened or skipped
to get to green. The first commit is a squash of the history before the tag.

Read the specification beside the code and judge for yourself whether git-dupe keeps
what it promises inside its `Operating envelope`. When something you ran does not go as
a line says, [`CONTRIBUTING.md`](../CONTRIBUTING.md) says how to report it, and the issue
form asks for exactly that.
