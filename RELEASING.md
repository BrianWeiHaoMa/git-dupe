# Releasing

How a release of git-dupe is made, step by step. Every step is run from the root of a
clone of the public repository, `https://github.com/BrianWeiHaoMa/git-dupe`, whose
remote is `origin`. In the commands, `<version>` stands for the new version, as
`0.1.0`; the first release is `v0.1.0`. The steps name the default branch `main`; where
it has another name, read that name for `main` in them (`Set once`, below).

## What a release is

- **The version**, `<major>.<minor>.<patch>`, is set in one place, `version` in
  `Cargo.toml`, which `Cargo.lock` follows (step 3): `git dupe --version` prints it, and
  the archive is named by it. The README names no version, so a release never edits it.
- **The tag** is an annotated tag `v<version>` on a commit of the default branch,
  `main`.
- **The GitHub Release** of that tag is titled with the tag. Its notes are the version's
  section of [`CHANGELOG.md`](CHANGELOG.md), and its two files are
  `git-dupe-<version>.tar` and `git-dupe-<version>.tar.sha256`. The archive holds the
  executable, the manual page, and the install note [`INSTALL`](INSTALL), under
  `git-dupe-<version>/`. It is made only by the scenarios of `release_archive`, which
  build it from the source into `target/release/` and check what it holds, whether
  `cargo test --test scenarios release_archive` runs them or a wider check does; it is
  never packed by hand. Its executable is built for the machine that makes it: x86-64
  Linux with the GNU C library, on the workflow's `ubuntu-22.04`, whose library is 2.35,
  so that the executable runs with 2.34 or newer, the platform the README's install
  section names. An executable runs only where the library is at least as new as the one
  it was linked against, so a newer image would raise that floor. The `.sha256` names the
  archive by its bare name, so that `sha256sum -c git-dupe-<version>.tar.sha256` checks a
  download in its own directory.

Pushing the tag runs [`.github/workflows/release.yml`](.github/workflows/release.yml),
which makes the Release. Before it checks or publishes anything, it refuses a tag that
is not `v` followed by `Cargo.toml`'s version, a `Cargo.lock` that names another version,
a tag that is not annotated, a tag whose commit is not on the default branch, and a
`CHANGELOG.md` without the version's section. Then it runs the checks
[`.github/workflows/ci.yml`](.github/workflows/ci.yml) runs, and publishes only when they
pass.

## Which number to raise

Versions follow [semantic versioning](https://semver.org/spec/v2.0.0.html), with
[`PRODUCT_SPECIFICATION.md`](PRODUCT_SPECIFICATION.md) as the contract:

- **Major:** a change that removes or alters a fixed decision (an `F` line), a guarantee
  (a `G` line), or the form of `.gitdupe` or of the managed region in
  `.git/info/exclude`.
- **Minor:** an amendment that adds a fixed decision or a guarantee.
- **Patch:** a repair, a check, or a document.

Before `1.0.0`, each moves one place down, as semantic versioning allows during initial
development: what would raise the major raises the minor, and everything else raises the
patch. `1.0.0` comes when you decide git-dupe has been used in public long enough that
its guarantees have held; it is made by the same steps as any other version.

## Steps

### 1. Start from the default branch

```console
$ git switch main
$ git pull --ff-only
$ git status --short
```

The last command prints nothing: a release is made of what `main` holds, and of nothing
else.

### 2. Bring the list of Git releases up to date

The minor Git releases published, oldest first, and the newest one the checks list:

```console
$ curl -s https://mirrors.edge.kernel.org/pub/software/scm/git/ | grep -oE 'git-2\.[0-9]+\.0\.tar\.xz' | grep -oE '2\.[0-9]+\.0' | sort -uV
$ grep -oE '"2\.[0-9]+\.0"' tests/scenarios/harness/releases.rs | tr -d '"' | tail -n 1
```

Call the second's answer `<old>`. When it is the last line of the first, go to step 3.
Otherwise every release the first lists after `<old>` is added, oldest first, before the
release and as a change of its own; `<new>` is the newest of them:

1. Check each of them against what git-dupe gives Git and reads from it, as R6 in
   [`TECHNICAL_SPECIFICATION.md`](TECHNICAL_SPECIFICATION.md) requires: read its release
   notes, `Documentation/RelNotes/<release>.adoc` in its source, for every command,
   option, configuration key, environment variable, output format, and pathspec form the
   technical specification relies on, and settle any doubt in the source of `<old>` and
   of that release. A difference is added to `Substrate assumptions`, and an option of a
   table whose meaning changed leaves the table (F4).
2. Add each, in order, at the end of `LISTED` in `tests/scenarios/harness/releases.rs`,
   the one place the code names a Git release, and raise the length in its type by as
   many.
3. Raise `<old>` to `<new>` where a line names the newest listed release: in
   `TECHNICAL_SPECIFICATION.md`, `Foundations` ("Git"), `Composition` ("The checks"),
   the opening of `Substrate assumptions`, and an assumption bounded "through `<old>`",
   as S11 is; in `PRODUCT_SPECIFICATION.md`, F4's newest "at this writing"; in
   [`CONTRIBUTING.md`](CONTRIBUTING.md), the examples of the `Checks` table and the
   count of releases the paragraph after it gives. This lists every line that names
   `<old>`, to read one by one:

   ```console
   $ grep -rnF --exclude-dir=.git --exclude-dir=target --exclude-dir=gits '<old>' .
   ```

   A line that says where something began, as "from 2.54.0", stays as it is.
4. Run the three checks of step 5. The first run obtains and builds the added releases
   into `gits/`.
5. Commit:

   ```console
   $ git commit -am "Check under Git <new>"
   ```

A Git 3 release is not added: git-dupe supports the releases below 3.0.0 (F4), and
supporting Git 3 is an amendment to the product specification.

### 3. Set the version

Set `version` in `Cargo.toml` to `<version>`, then bring the lock file along:

```console
$ cargo update -w
$ git diff Cargo.lock
```

The diff shows only the version of the `git-dupe` entry. A lock file that still names
the old version does not build with `--locked`, as the archive is built, from a fresh
checkout of the tag, and `release.yml` refuses a tag whose `Cargo.lock` names another
version. For `v0.1.0`, `Cargo.toml` already says `0.1.0`: leave both files as they are.

### 4. Date the changelog

In `CHANGELOG.md`, rename `## [Unreleased]` to `## [<version>] - <date>`, `<date>` being
today as `date -u +%F` prints it, as `2026-10-02`. Above it, start a new, empty
`## [Unreleased]`. The section under the version's heading becomes the Release's notes;
the workflow refuses a tag whose version has no such section, or an empty one.

### 5. Run the checks

```console
$ cargo test
$ cargo fmt --check
$ cargo clippy --all-targets -- -D warnings
```

All three pass on your machine. `cargo test` is the delivery check: every unit check, and
every scenario under every listed Git release, the 100,000-file scenario `scale` among
them. The workflows run a focused form that leaves `scale` out, so this run is the one
that holds a release to the counted Git runs among 100,000 files and their one-second
bound. What the checks need, and what each costs, is in the `Checks` section of
[`CONTRIBUTING.md`](CONTRIBUTING.md).

### 6. Commit the release

```console
$ git add Cargo.toml Cargo.lock CHANGELOG.md
$ git commit -m "Release v<version>"
```

### 7. Tag it

```console
$ git tag -a v<version> -m "git-dupe <version>"
$ git cat-file -t v<version>
```

The second command prints `tag`: the tag is annotated. A tag made without `-a`, `-s`, or
`-m` prints `commit`; delete it with `git tag -d v<version>` and tag again.

### 8. Push the branch and the tag

```console
$ git push --atomic origin main v<version>
```

Both are pushed, or neither. The branch runs `ci.yml`; the tag runs `release.yml`.

### 9. Watch the release

Open `https://github.com/BrianWeiHaoMa/git-dupe/actions/workflows/release.yml` and the
run for `v<version>`. When it passes, the Release is at
`https://github.com/BrianWeiHaoMa/git-dupe/releases/tag/v<version>`, with its notes and
its two files. Download both into one directory and check them there:

```console
$ sha256sum -c git-dupe-<version>.tar.sha256
```

It prints `git-dupe-<version>.tar: OK`.

## When the release workflow fails

The log of the run names the step and why.

- **A refusal before the checks**: the tag's name, the version, the lock file, an
  unannotated tag, a commit not on `main`, or the changelog. Nothing was published.
  Remove the tag, here and on GitHub:

  ```console
  $ git tag -d v<version>
  $ git push origin :refs/tags/v<version>
  ```

  Fix the cause; where a file must change, that is a new commit on `main`, made after the
  checks of step 5 pass on it. Then tag and push again, steps 7 and 8. A tag is moved
  only while no Release of it exists; once one is published, a fix is a new version.
- **A check failed.** Run the same check on your machine. A failure there is a defect to
  repair before the release, which then goes as the refusal above. A check that fails
  only on the runner is still a finding: read its log, and do not skip, weaken, or
  re-run it until it passes.
- **The workflow cannot run or cannot publish**, as when GitHub Actions is unavailable:
  make the Release by hand, from the tagged commit, on an x86-64 Linux machine with the
  GNU C library no newer than the workflow's 2.35, so that the executable runs where the
  workflow's would, after the checks of step 5 passed on it:

  ```console
  $ git switch --detach v<version>
  $ cargo test --test scenarios release_archive
  $ cd target/release
  $ sha256sum git-dupe-<version>.tar > git-dupe-<version>.tar.sha256
  $ sha256sum -c git-dupe-<version>.tar.sha256
  $ cd ../..
  ```

  Copy the text of the version's section of `CHANGELOG.md`, without its heading, into a
  file `notes.md` outside the clone, as `../notes.md`. Then, with the GitHub CLI signed
  in (`gh auth login`):

  ```console
  $ gh release create v<version> --verify-tag --title v<version> --notes-file ../notes.md target/release/git-dupe-<version>.tar target/release/git-dupe-<version>.tar.sha256
  ```

  Or, on the repository's Releases page, draft a new release: choose the tag, title it
  with the tag, paste the notes, attach the two files, and publish. Then
  `git switch main`.

## Set once

- **Before the first release**, the repository on GitHub holds the project on `main`,
  and `ci.yml` has passed there once; its first run also builds the two Git releases it
  checks under, which later runs, the release's included, take from its cache while
  GitHub keeps it.
- **The default branch is `main`.** `ci.yml` runs on every push to `main` and every pull
  request. If the public repository's default branch has another name, change `main` in
  `ci.yml`'s `on: push: branches:` to it, and read it for `main` in the steps above.
  `release.yml` reads the default branch from the repository itself, as the branch its
  `HEAD` names.
- **The badge** under the README's title is `ci.yml`'s: the result of its latest run on
  the default branch.
- **Permissions.** `ci.yml` reads the repository's contents and nothing more;
  `release.yml` asks for `contents: write`, which publishing a Release needs, in its own
  file. The repository needs GitHub Actions enabled and nothing else configured.

After a release, the next change a user would notice gets its line under
`## [Unreleased]` in `CHANGELOG.md`, in the same change.
