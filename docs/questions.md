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
- **Is everything in a worktree saved, before `git worktree remove`?** A push alone does
  not say. `git worktree remove` deletes the worktree's dupe, and your files there are
  ignored files to the project's Git, which removes them without asking. In that worktree,
  in this order, as the manual page's WORKTREES section gives it:
  `git dupe status --short -uall` lists what no commit holds, `??` an untracked file and
  `!!` one the project ignores: `git dupe add <path>`, with `-f` for a `!!` one, then
  `git dupe commit`, keeps each. A warning that a path stands here and is hidden by
  another worktree alone names a file that status does not list: `git dupe add <path>`,
  with `-f` where Git says the project ignores it, and a commit keep it.
  `git dupe stash list` shows the stash entries, which no push sends:
  `git dupe stash pop --index` puts the newest back, what was staged staged again, and
  `git dupe commit`, where something is staged, then `git dupe commit -a` keep it.
  `git dupe fetch --all --prune`, then
  `git dupe log HEAD --branches --tags --not --remotes`, lists the commits no
  remote-tracking branch holds: `git dupe push origin <branch>` sends a branch, and
  `git dupe push origin HEAD:refs/heads/<name>`, or `<tag>~0:refs/heads/<name>`, the commit
  `HEAD` or a tag names, to a branch of a name no branch or tag there has; attached from
  another worktree's dupe, that is how this branch goes back, and `git dupe merge <name>`
  in that worktree takes it, as the hint after a refused push there says.
  `git dupe detach` without `--force` refuses while a change is staged or a commit or a
  stash entry is on no remote, but it sees no untracked file, nor one the project ignores.
- **How do I rename a hidden directory?** Hide the new name first, then move, then
  unhide the old one, each from the root: `git dupe hide journal/`, then
  `git dupe mv notes journal`, then `git dupe unhide notes`. No step lets the project's
  Git see a file of yours, unless a rule of the project's re-includes the new name, which
  a warning then names; where `git dupe hide` refuses the new name, because the project
  tracks a file under it, stop there: nothing has moved. `git dupe mv` alone leaves
  `.gitdupe` listing `notes`, and what the dupe does not track under `journal` is then
  visible to the project; its hint names the two commands that list `journal` instead.
  A public `git mv` of a directory that holds private files moves them too, to paths
  nothing hides: hide each one's new path first; `git dupe add -A` then records the move,
  with `git dupe add -f` for a moved file the project ignores.
- **Does the dupe come along when someone clones the project?** No. `.git` is not part
  of a clone, so nothing of yours is in the project. In a fresh clone,
  `git dupe clone <url>` writes the files that are missing, but none at a path the
  project tracks, keeps any already there, and names the ones that differ.
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
  overwrites yours, and both repositories then track the path: `git dupe status` and the
  other commands warn of it, naming the command that releases it from each. The file
  there is now the project's, and a public `git add` stages whatever stands there, so
  `git dupe restore` would put yours where the project's next commit takes it. Commit
  first. Then, from the root, `git dupe show HEAD:<path> > <new path>` saves yours under
  another path, `git dupe add -f -- <new path>` makes that one private, and
  `git dupe rm --cached -- <path>` gives the project's path up; the warning spells each
  command for the path it names. `git dupe clone` on another machine keeps the project's
  file and names it the same way.
- **How do I get out?** `git dupe detach` removes the worktree's dupe and its own region
  of `.git/info/exclude`, leaving the rest of that file but the region of a worktree
  whose dupe is gone, leaves every file on disk, `.gitdupe` included, and warns which
  paths the project can now see and which another worktree still hides. Without
  `--force` it refuses while a change is uncommitted or a commit of a branch, a tag, a
  stash entry, or `HEAD` is on no remote; history removed by `detach --force` is gone,
  since git-dupe keeps no copy. Then delete `~/.local/bin/git-dupe` and
  `~/.local/share/man/man1/git-dupe.1`, on Windows `~\.local\bin\git-dupe.exe`, or
  `cargo uninstall git-dupe`, as [Installing](install.md#uninstalling) says.
