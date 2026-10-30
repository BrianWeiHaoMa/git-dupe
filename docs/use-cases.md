# What git-dupe is for

The uses git-dupe was made for, and how it compares with what you may already use for the
same files. The [README](../README.md) has the quick start, [How it works](how-it-works.md)
what git-dupe runs and writes, and [Questions](questions.md) the usual doubts.

## What it is for

- **Your own agent instructions in someone else's project.** An open-source project that
  takes no `AGENTS.md`, `CLAUDE.md`, skills, or notes from a contributor's own workflow:
  keep yours in the worktree all the same, `git dupe add AGENTS.md .claude/skills/ notes/`,
  and your agent reads them where it expects them, while `git push` sends the branch and
  none of these files. Where the project has its own `AGENTS.md`, yours is a
  `CLAUDE.local.md` or another path the project does not use: a path belongs to one
  repository or the other, and `git dupe add` refuses a file the project tracks.
- **A team's agent kit, in every repository.** Keep the team's skills, instruction files,
  and scripts in one Git repository of the team's, at paths a project does not track
  itself. In a clone of any project, `git dupe clone <kit-url>` brings them in, hidden
  from that project, and `git dupe pull` takes the kit's updates; what you add on top
  goes on a branch or a remote of your own, as in any Git repository. `git dupe clone`
  keeps a file the project already has at one of the kit's paths, as it is, and names it
  in a warning.
- **Notes, plans, and scratch scripts that follow you.** A `notes/` and a `scripts/` of
  your own in each project, with the history of your thinking, on every machine you work
  on, and never in a pull request by accident.
- **Settings for your tools.** `.vscode/`, a local configuration file, whatever the
  project's `.gitignore` already ignores: `git dupe add -f` versions it, so that a fresh
  clone does not start from nothing.
- **Agents working several branches at once.** Each linked worktree that
  `git worktree add` made is a workspace of its own, attached from the main worktree's
  dupe or from your remote, and your work moves between worktrees as between machines
  ([Questions](questions.md) has the commands).
- **Agents and scripts.** git-dupe adds no prompts of its own. `git dupe status
  --porcelain` lists the changes under the hidden paths, a refusal is one `fatal:` line
  with exit status 128, and a usage error exits 129.

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
- **A submodule.** Its gitlink and `.gitmodules` are committed, so the project sees it,
  and it holds a subdirectory, never a file at the root such as `AGENTS.md`.
- **A gitignored folder, or symlinks from a dotfiles repository.** Hidden only where the
  team's `.gitignore` happens to name it, versioned only if the dotfiles repository knows
  every project, and gone from every fresh clone until you set it up again.
- **git-crypt, sops, age.** git-crypt and sops encrypt files that stay in the project's
  repository, and age encrypts a file wherever it is; git-dupe keeps files out of the
  project and encrypts nothing. If you want both, encrypt before you add.
