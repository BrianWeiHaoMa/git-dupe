# git-dupe

[![CI](https://github.com/BrianWeiHaoMa/git-dupe/actions/workflows/ci.yml/badge.svg)](https://github.com/BrianWeiHaoMa/git-dupe/actions/workflows/ci.yml)

git-dupe keeps your own files of a project under version control — `.env.local`,
editor settings, notes, plans, scratch scripts, instructions for your coding agent — and
out of the project's repository. They stay where their tools expect them in the working
tree, and their history lives in a second, ordinary Git repository inside the project's
`.git`. Public work is `git …`; private work is `git dupe …`, with
Git's own commands, options, output, and exit codes.

## Install

git-dupe runs on Linux and needs Git 2.43.0 or newer, below 3.0.0. It is one executable,
`git-dupe`, which Git runs for `git dupe …` when it stands in a directory on `PATH`.

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
another machine, or where it does not start, install from source. Check the Git before
the library: `git --version` must say 2.43.0 or newer. Ubuntu 24.04 ships 2.43.0;
Ubuntu 22.04 ships 2.34 and Debian 12 ships 2.39, so there a newer Git comes first, from
a backport, a PPA such as Ubuntu's `git-core`, or a build from source.

**From source.** In a clone of this repository, with Rust installed through rustup, which
installs the toolchain the repository pins on first use:

```sh
cargo install --path .
```

That installs the executable alone: `git dupe help` works with nothing else, while
`man git-dupe` and `git dupe --help` need the manual page, `git-dupe.1` in a release
archive, installed as its `INSTALL` says.

## Quick start

In a clone of a project, where `notes/` holds notes of yours and `.env.local` your
settings, a file the project's `.gitignore` ignores, run from the root:

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

`git status` lists none of the private files, and `git dupe status` lists none of the
project's. `<private-url>` is a repository on a host or a disk you trust, with the
project's branch as its default branch, which `git dupe clone` checks out; on a disk,
`git init --bare -b main` makes one for a project on `main`. git-dupe refuses a URL of
the project's own repository. From then on, `git dupe pull` and `git dupe push` move
private work between machines.

## What it is for

- **Files the project should never see.** `.env.local`, `.vscode/`, notes, plans, and
  scratch scripts, versioned with a history of their own and brought to every machine
  you work on: `git dupe push` and `git dupe pull` move them through a private remote of
  your choosing.
- **Your own instructions for a coding agent.** A project that keeps no `AGENTS.md` or
  `CLAUDE.md`, or accepts none, still lets you keep yours: `git dupe add CLAUDE.md` makes
  it private, with `-f` where the project ignores it. Your agent reads it
  where it expects it, the project never sees it, and once it is committed and pushed,
  `git dupe clone` brings it to every new clone. A file only the project tracks is
  refused: a path belongs to one repository or the other.
- **Agents and scripts.** git-dupe adds no prompts of its own. `git dupe status
  --porcelain` lists the private changes, a refusal is one `fatal:` line with exit
  status 128, and a usage error exits 129.

## How it works

- The **private repository** is an ordinary Git repository at `.git/dupe`, inside the
  project's own `.git`, whose working tree is the project's. Plain Git can read it, and
  clone, fetch, or push from it.
- A **hidden path** is one the project's Git ignores through rules git-dupe maintains in
  `.git/info/exclude`, so that `git status`, `git add -A`, and a public commit never see
  it: `.gitdupe`, a private file at the root that lists the paths you hide, one per line;
  every path it lists; and every file tracked privately, as a file you add is. Nothing
  versioned is added to the project.
- Three commands get the meaning a shared working tree needs: `git dupe status` and
  `git dupe add` act on the hidden paths instead of every file of the project, and
  `git dupe clean` cleans the project while sparing them. `git dupe stash -u` and `-a`,
  which would stash the whole project, are refused, and so are a `push`, `pull`,
  `fetch`, `remote`, or `clone` naming the project's own repository. Every other Git
  command, `commit`, `diff`, `log`, `restore`, `switch`, `merge`, runs against the
  private repository unchanged.
- **Plain `git` is not guarded.** It sees a private file as one the project ignores:
  `git clean -x` deletes it, and a public pull or checkout that brings a file to its path
  overwrites it. Commit private work first; `git dupe restore .`, from the root,
  brings back every privately tracked file as last staged.

## Built to a specification

**git-dupe is new, and a tool that touches your files has to earn your trust.** So
everything here was built toward one document,
[`PRODUCT_SPECIFICATION.md`](PRODUCT_SPECIFICATION.md): in numbered lines, what git-dupe
does, the guarantees it keeps, and what it never does. Every line of code was written to
it and reviewed against it, and
[`TECHNICAL_SPECIFICATION.md`](TECHNICAL_SPECIFICATION.md) says how each guarantee holds.
`cargo test` runs every scenario of the suite, the specification's own `Done when` among
them, under every minor Git release from 2.43.0 to the newest the repository lists.

**You are welcome to check it.** Read the specification beside the code, on your own or
with a strong model, and judge whether git-dupe keeps what it promises under the conditions of
its `Operating envelope`. When something you ran does not go as a line says,
[`CONTRIBUTING.md`](CONTRIBUTING.md) says how to report it.

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
