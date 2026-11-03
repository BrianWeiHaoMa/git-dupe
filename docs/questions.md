# Questions

What people ask once git-dupe is in a project of theirs. The [README](../README.md) has
the quick start, [How it works](how-it-works.md) what git-dupe runs, writes, and leaves
alone, and [What it is for](use-cases.md) the uses it was made for.

- **Agents working several branches at once, in linked worktrees?** Each worktree is a
  workspace of its own, and your work moves between worktrees as between machines. In
  one that `git worktree add` made, run
  `git dupe clone "$(git rev-parse --git-common-dir)/dupe"` to attach it from the main
  worktree's dupe; then `git dupe push origin HEAD:agent` there puts its branch in the
  main worktree's dupe, and `git dupe merge agent` in the main worktree takes it.
  Attached instead by `git dupe clone <url>` of your remote, it moves its work through
  that remote with `git dupe push` and `git dupe pull`. After `git worktree move`, run
  `git dupe init` in the new place so that plain Git finds it again.
- **Does the dupe come along when someone clones the project?** No. `.git` is not part
  of a clone, so nothing of yours is in the project. In a fresh clone,
  `git dupe clone <url>` writes the files that are missing, keeps any already there, and
  names the ones that differ.
- **Two people on the same project?** Each clone has its own `.git/dupe`, its own
  `.gitdupe`, and its own remote, one person per clone; nothing shows in the project for
  anyone else. What a team shares, it shares through a remote, as the kit under
  [What it is for](use-cases.md#what-it-is-for) does.
- **My agent runs `git add -A` all day.** Anything that finds files through the ignore
  rules, `add -A`, `add .`, `commit -a`, `status`, never picks up a hidden path. What
  writes the index from an explicit tree or patch can: `add -f`, or a merge, rebase, or
  patch that brings a file to that path. The path is then tracked by both repositories,
  stays hidden, and every git-dupe command warns until one side lets go.
- **What if the team's `.gitignore` has a `!` rule for my path?** `.gitignore` outranks
  `info/exclude`, so the path is exposed. git-dupe names it, and the deciding rule with
  its file and line, after every command until the rule or the path changes, unless a
  sparse checkout keeps that `.gitignore` out of the working tree, where git-dupe does
  not read it. Nothing is lost, but until then a public `git add -A` would stage it, so
  move the path or change the rule.
- **What if the project later adds a file at my path?** A public pull or checkout
  overwrites yours. Commit first; `git dupe restore <path>` brings it back; then one
  repository has to give the path up.
- **How do I get out?** `git dupe detach` removes the worktree's dupe and its own region
  of `.git/info/exclude`, leaving the rest of that file but the region of a worktree
  whose dupe is gone, leaves every file on disk, `.gitdupe` included, and warns which
  paths the project can now see and which another worktree still hides. Without
  `--force` it refuses while a change is uncommitted or a commit of a branch, a tag, a
  stash entry, or `HEAD` is on no remote; history removed by `detach --force` is gone,
  since git-dupe keeps no copy. Then delete `~/.local/bin/git-dupe` and
  `~/.local/share/man/man1/git-dupe.1`, on Windows `~\.local\bin\git-dupe.exe`, or
  `cargo uninstall git-dupe`, as [Installing](install.md#uninstalling) says.
