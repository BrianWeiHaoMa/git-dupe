# git-dupe

[![CI](https://github.com/BrianWeiHaoMa/git-dupe/actions/workflows/ci.yml/badge.svg)](https://github.com/BrianWeiHaoMa/git-dupe/actions/workflows/ci.yml)
[![Latest release](https://img.shields.io/github/v/release/BrianWeiHaoMa/git-dupe)](https://github.com/BrianWeiHaoMa/git-dupe/releases/latest)
[![License: MIT](https://img.shields.io/github/license/BrianWeiHaoMa/git-dupe)](LICENSE)

git-dupe versions the files in a checkout that are yours and not the project's:
`.env.local`, notes, plans, scratch scripts, editor settings, your own instructions file
for a coding agent. They stay where their tools expect them in the working tree. Their
history lives in a second, ordinary Git repository inside the project's `.git`, hidden
from the project's `git status` and `git add -A`, and moves between your machines
through a private remote of your own. Public work is `git …`; private work is
`git dupe …`, with Git's own commands, options, output, and exit codes.

"Dupe" as in a duplicate of your checkout that only you see; it has nothing to do with
duplicate files. Linux only and Git 2.43 or newer, for now. What it does not do yet is
under [Limits](#limits), and `git dupe detach` takes it back out.

<img src="docs/demo.png" width="880" alt="A terminal transcript. git status --short lists CLAUDE.md and notes/ as untracked. After git dupe init, git dupe add notes/ CLAUDE.md, git dupe add -f .env.local, and git dupe commit -m 'my private files', git status --short prints nothing, and git dupe log --oneline shows the one private commit.">

## Quick start

In a clone of a project, from the root. Here `notes/` is a directory of yours, and
`.env.local` a file of yours that the project's `.gitignore` ignores:

```sh
git dupe init                             # attach an empty private repository
git dupe add notes/                       # hide notes/ from the project and stage it
git dupe add -f .env.local                # a file the project ignores takes -f
git dupe commit -m "Private files"        # a private commit: git log never shows it
git dupe remote add origin <private-url>  # an empty repository of your own
git dupe push -u origin HEAD              # send the private history there
git dupe clone <private-url>              # in a clone elsewhere: the files come back
```

The project then holds:

```text
project/
├── .env.local     private
├── .git/dupe/     the private repository
├── .gitdupe       private: lists notes, the directory you hid
├── notes/         private
├── README.md      public
└── src/           public
```

`git status` lists none of the private files, `git log` shows no private commit, and
`git dupe status` lists none of the project's. `<private-url>` is an empty repository on
a host or a disk you trust, with the project's branch as its default branch, which
`git dupe clone` checks out unless `-b` names another; on a disk,
`git init --bare -b main` makes one for a project on `main`. A URL of the project's
repository, or of any of its remotes, is refused. From then on, `git dupe pull` and
`git dupe push` move private work between machines.

Everything private lives in `.git/dupe`. Delete the clone, and unpushed private history
goes with it, so push first. `git dupe detach` is the way out: it leaves every file on
disk, and refuses, without `--force`, while private work is uncommitted or on no remote.

## Install

git-dupe runs on Linux and needs Git 2.43.0 or newer, below 3.0.0, a release that is not
out yet and may change what git-dupe relies on. It is one executable, `git-dupe`, which
Git runs for `git dupe …` when it stands in a directory on `PATH`.

**From a release.** Download `git-dupe-<version>.tar` from the
[releases](https://github.com/BrianWeiHaoMa/git-dupe/releases) and follow its `INSTALL`
note, which installs the executable and the manual page, with `~/.local/bin` on `PATH`
and `~/.local/share/man` on the manual path:

```sh
tar -xf git-dupe-<version>.tar
cd git-dupe-<version>
mkdir -p ~/.local/bin ~/.local/share/man/man1
cp git-dupe ~/.local/bin/
cp git-dupe.1 ~/.local/share/man/man1/
```

The release's executable is for x86-64 Linux with the GNU C library 2.34 or newer; on
another machine, or where it does not start, install from source. Either way, the Git on
`PATH` decides: `git --version` must say 2.43.0 or newer. Ubuntu 24.04 ships 2.43.0 and
Debian 13 ships 2.47; Ubuntu 22.04 ships 2.34 and Debian 12 ships 2.39, so there a newer
Git comes first, from the `git-core` PPA, a backport, or a build from source.

**From source.** In a clone of this repository, with Rust installed through rustup, which
installs the toolchain the repository pins on first use:

```sh
cargo install --path .
```

That installs the executable alone. `git dupe help` and `git dupe help <command>` cover
daily use from the executable; the manual page behind `man git-dupe` and
`git dupe --help` comes with a release archive, installed as its `INSTALL` says.

To uninstall, run `git dupe detach` in each project that uses it, then delete the two
files, or `cargo uninstall git-dupe` after an install from source.

## What it is for

- **Files the project should never see.** `.env.local`, `.vscode/`, notes, plans, and
  scratch scripts, versioned with a history of their own and brought to every machine
  you work on: `git dupe push` and `git dupe pull` move them through a private remote of
  your choosing.
- **Your own instructions for a coding agent.** Your own `CLAUDE.md` or `AGENTS.md`
  where the project has none, or a `CLAUDE.local.md` beside the project's own:
  `git dupe add` makes it private, with `-f` where the project ignores it, your agent
  reads it where it expects it, and `git dupe clone` brings it to every new clone. A
  file the project tracks is refused: a path belongs to one repository or the other.
- **Agents and scripts.** git-dupe adds no prompts of its own. `git dupe status
  --porcelain` lists the private changes, a refusal is one `fatal:` line with exit
  status 128, and a usage error exits 129.

## How it works

- The **private repository** is an ordinary Git repository at `.git/dupe`, inside the
  project's own `.git`, whose working tree is the project's. Plain Git can read it, and
  clone, fetch, or push from it. It has its own object store, which the project's Git
  never opens, so the project's `git gc` and `git prune` leave it alone; and every Git
  process git-dupe runs has the object-directory variables cleared, so a hook or alias
  of the project cannot redirect private objects into the project's store. Being inside
  `.git`, it is never committed and never cloned with the project; `git dupe clone` is
  the deliberate way back.
- A **hidden path** is one the project's Git ignores through rules git-dupe maintains in
  `.git/info/exclude`, so that `git status`, `git add -A`, and a public commit never see
  it. The hidden paths are `.gitdupe`, a private file at the root that lists the paths
  you hide, one per line; every path it lists; and every file tracked privately, as a
  file you add is. Nothing versioned is added to the project.
- Three commands are changed to fit a shared working tree: `git dupe status` and
  `git dupe add` act on the hidden paths instead of every file of the project, and
  `git dupe clean` cleans the project while sparing them. `git dupe stash -u` and `-a`,
  which would stash the whole project, are refused, and so are a `push`, `pull`,
  `fetch`, `remote`, or `clone` naming the project's repository or any of its remotes.
  Every other Git command, `commit`, `diff`, `log`, `restore`, `switch`, `merge`, runs
  against the private repository unchanged.
- **What it writes.** Of its own accord, git-dupe writes `.git/dupe`, a marked region of
  `.git/info/exclude` (and `.git/info` when it is missing), and `.gitdupe`;
  `git dupe clone` also writes the private files it checks out, and `git dupe clean`
  deletes what your `git clean` would, hidden paths excepted. Beyond that, a Git command
  writes where your words tell Git to. No hook, no daemon, no edit to `.gitignore` or to
  the project's configuration, no dependency beyond Rust's standard library, and the
  only program it runs is `git`.

## Limits

- **Linux only, for now.** The specification promises behavior on Linux, on a local,
  case-sensitive filesystem, and that is what the suite runs under. Nothing known rules
  macOS out, but its default filesystem is case-insensitive, outside the envelope, so a
  port is more than a rebuild. If you want one, say so in
  [issue #1](https://github.com/BrianWeiHaoMa/git-dupe/issues/1).
- **Git 2.43.0 or newer, below 3.0.0.** No single feature needs it; it is the floor the
  suite covers, and Ubuntu 24.04's stock Git. [Install](#install) says what older
  distributions ship.
- **No encryption.** The private remote is the trust boundary: an `.env.local` you push
  sits there in plaintext, so push to a remote you would trust with it, or keep secrets
  out of what you add.
- **Plain `git` is not guarded.** To the project's Git a private file is an ignored file:
  `git clean -x` deletes it, `git stash -a` stashes it, `git add -f` makes it public, and
  a public pull or checkout that brings a file to its path overwrites it. Commit private
  work first; `git dupe restore .`, from the root, brings back every privately tracked
  file as last staged, and `git dupe clean` cleans the project while sparing them. Public
  `git stash -u`, `git add -A`, and `git add .` leave them alone, as they leave any
  ignored file.
- **One command at a time.** git-dupe commands must not overlap each other or a public
  Git command that writes to the working tree, and one person or agent works in a clone
  at a time; nothing is promised when they do.
- **The main working tree only.** `git dupe` in a linked worktree is refused and names
  the main one. The exclude rules apply in every worktree, since `.git/info/exclude` is
  shared, but private versioning exists in the main one only. Whether linked worktrees
  should share one private repository or each get their own is undecided; if you need
  them, say how you would want them to behave in
  [issue #2](https://github.com/BrianWeiHaoMa/git-dupe/issues/2).
- **Up to 1,000 hidden paths, a few thousand private files.** That is the envelope the
  suite covers. The hidden paths not below another one are passed to Git on one command
  line; a list that does not fit is refused, naming the count, never truncated. Files
  below a hidden directory do not count toward the paths: `notes/` is one path.

## Compared with what you may already use

- **`.git/info/exclude` by hand.** Hides a path from the project and nothing else: no
  history, and nothing on the next machine. git-dupe maintains that file for you and
  adds the history and the sync.
- **The bare-repository dotfiles trick, vcsh, yadm, chezmoi.** The first three are the
  same mechanism as git-dupe's, a Git directory kept elsewhere with a working tree it
  does not own, and all four are built for `$HOME`: none knows about a project's exclude
  file or its remotes. git-dupe is that pattern scoped to one project and kept inside
  its `.git`, with the exclude rules maintained for you, `status` and `add` confined to
  your files, and the forms that would stash the whole project or push to the project's
  remote refused. If the trick already works for you, keep it; and yadm and chezmoi can
  encrypt, which git-dupe does not.
- **git-crypt, sops, age.** git-crypt and sops encrypt files that stay in the team's
  repository, and age encrypts a file wherever it is. git-dupe keeps files out of the
  project and encrypts nothing; if you want both, encrypt before you add.
- **A submodule.** Its gitlink and `.gitmodules` are committed, so the project sees it,
  and it holds a subdirectory, never a file at the root such as `.env.local`.
- **A gitignored folder, or symlinks from a dotfiles repository.** Hidden only where the
  team's `.gitignore` happens to name it, versioned only if the dotfiles repository knows
  every project, and gone from every fresh clone until you set it up again.

## Questions

- **Does the private repository come along when someone clones the project?** No. `.git`
  is not part of a clone, so nothing of yours is in the project. In a fresh clone,
  `git dupe clone <private-url>` writes the private files that are missing, keeps any
  already there, and names the ones that differ.
- **Two people on the same project?** Each clone has its own `.git/dupe`, its own
  `.gitdupe`, and its own private remote, one person per clone. Nothing shows in the
  project for anyone else.
- **My agent runs `git add -A` all day.** Anything that finds files through the ignore
  rules, `add -A`, `add .`, `commit -a`, `status`, never picks up a hidden path. What
  writes the index from an explicit tree or patch can: `add -f`, or a merge, rebase, or
  patch that brings a file to that path. The path is then tracked by both repositories,
  stays hidden, and every git-dupe command warns until one side lets go.
- **What if the team's `.gitignore` has a `!` rule for my path?** `.gitignore` outranks
  `info/exclude`, so the path is exposed. git-dupe names it, and the deciding rule with
  its file and line, after every command until the rule or the path changes. Nothing is
  lost, but until then a public `git add -A` would stage it, so move the path or change
  the rule.
- **What if the project later adds a file at my path?** A public pull or checkout
  overwrites yours. Commit private work first; `git dupe restore` brings it back; then
  one repository has to give the path up.
- **How do I get out?** `git dupe detach` removes `.git/dupe` and its own region of
  `.git/info/exclude`, leaving the rest of that file, leaves every file on disk,
  `.gitdupe` included, and warns which paths the project can now see. Without `--force`
  it refuses while anything is uncommitted or on no remote; history removed by
  `detach --force` is gone, since git-dupe keeps no copy. Then delete
  `~/.local/bin/git-dupe` and `~/.local/share/man/man1/git-dupe.1`, or
  `cargo uninstall git-dupe`.

## Built to a specification

git-dupe is new, and a tool that writes inside `.git` has to earn trust, so it is held to
a specification. [`PRODUCT_SPECIFICATION.md`](PRODUCT_SPECIFICATION.md) says, in
numbered lines, what git-dupe does, the guarantees it keeps, and what it never does;
[`TECHNICAL_SPECIFICATION.md`](TECHNICAL_SPECIFICATION.md) says how each guarantee holds.
The code follows both, and a change of behavior amends the specification first. The
scenario suite, the specification's own `Done when` among it, runs on every push under
Git 2.43.0 and 2.56.0, the oldest and the newest release it lists
([CI](https://github.com/BrianWeiHaoMa/git-dupe/actions/workflows/ci.yml)), and before
each release under every minor release between them, fourteen at this writing, each
built from source.

**How it was built.** I wrote the product specification and own every decision in it:
the command surface, what is refused, what is left to plain Git, and the tradeoffs
behind each. Coding agents wrote most of the Rust and the scenarios, to those two
documents and under my direction, and I reviewed each change against them before it
landed. The rules they worked under are the ones in [`CONTRIBUTING.md`](CONTRIBUTING.md):
one directory of code per part of the technical specification's `Composition`; code,
checks, and documents citing the lines they serve by label; a change of behavior
amending the specification before it touches the code; and no check weakened or skipped
to get to green. The first commit is a squash of the history before the tag.

Read the specification beside the code and judge for yourself whether git-dupe keeps
what it promises inside its `Operating envelope`. When something you ran does not go as
a line says, [`CONTRIBUTING.md`](CONTRIBUTING.md) says how to report it, and the issue
form asks for exactly that.

## Read more

- `git dupe help`, and `git dupe help <command>` for each command git-dupe adds or
  changes: a screen each, from the executable alone.
- `man git-dupe`, where the manual page is installed: the commands, a quick start,
  examples, and explanations.
- [`PRODUCT_SPECIFICATION.md`](PRODUCT_SPECIFICATION.md): what git-dupe does and
  guarantees, and what it does not.
- [`TECHNICAL_SPECIFICATION.md`](TECHNICAL_SPECIFICATION.md): how each guarantee holds.
- [`CONTRIBUTING.md`](CONTRIBUTING.md): what governs a change, how to report a problem,
  and how to run the checks.
- [`CHANGELOG.md`](CHANGELOG.md): what changed in each release.
- [`RELEASING.md`](RELEASING.md): what a release holds and how one is made.

## License

MIT, in [`LICENSE`](LICENSE).
