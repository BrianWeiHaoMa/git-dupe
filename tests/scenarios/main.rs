//! The scenarios: git-dupe driven as `git dupe …` under every Git release the repository
//! checks, each built by the harness under `gits/` at the repository root.
//!
//! The commands that run the checks, and what each costs, are listed in `CONTRIBUTING.md`
//! at the repository root, and nowhere else.
//!
//! The rules for a scenario:
//!
//! - It is one `#[test]` function whose body is `under_each_release(|s| { … })`. It is
//!   written once and runs once per release: it names no release and branches on none.
//! - It reaches git-dupe only as `s.git(["dupe", …])`, so that Git's own dispatch is part
//!   of every observation, and writes only inside `s.dir()`.
//! - It asserts git-dupe's own lines, exit statuses, and effects, never the text of a Git
//!   message. Where Git's own answer is the required result, it runs that Git command
//!   itself, under the same release and in the same place, and compares.
//! - `tests/` holds this directory alone, the package's one integration test target. A
//!   scenario file is a module here, named for what it pins.
//!
//! A check of something the build makes rather than of a run of Git — the page the build
//! renders, read with this machine's `man`, and the release archive's members — is a
//! plain `#[test]`, kept in `manual_page` or `release_archive` beside the scenarios that
//! install what it checks. The check of `README.md` is a plain `#[test]` too, in
//! `readme`: the `git dupe` lines the README shows are the ones `manual_page_commands`
//! runs, and the install note's lines it shows are the ones `release_archive` follows, so
//! that a changed command line of a page text or of the note fails there until the README
//! follows it.

mod harness;

mod add;
mod add_forms;
mod add_ignored;
mod add_settings;
mod add_skipped;
mod alias;
mod alias_ambiguous;
mod alias_chain;
mod alias_prefix;
mod alias_runs;
mod clean;
mod clean_cases;
mod clean_forms;
mod clean_run;
mod clone;
mod clone_alias;
mod clone_branches;
mod clone_kept;
mod clone_killed;
mod clone_refusals;
mod clone_runs;
mod detach;
mod detach_killed;
mod detach_refusals;
mod done_when;
mod done_when_bare_worktrees;
mod done_when_two_machines;
mod done_when_worktrees;
mod git;
mod gitdupe;
mod help;
mod help_request;
mod hide;
mod home_destinations;
mod in_use;
mod in_use_scripted;
mod init;
mod init_template;
mod isolation;
mod killed;
mod manual_page;
mod manual_page_commands;
mod offered_commands;
mod own_lines;
mod own_runs;
mod passthrough;
mod passthrough_settings;
mod plain_git;
mod private_repository;
mod public_places;
mod public_to_private;
mod publicly_tracked_is_visible;
mod readme;
mod region;
mod region_lock;
mod release_archive;
mod released;
mod remote;
mod remote_where;
mod scale;
mod settle;
mod settle_failures;
mod stash;
mod status;
mod status_run;
mod table;
mod transfer;
mod transfer_default;
mod transfer_help;
mod transfer_push_hint;
mod transfer_runs;
mod unhide;
mod usage;
mod version;
mod workspace;
mod workspace_lifetime;
mod worktree_clone;
mod worktree_concurrency;
mod worktree_detach;
mod worktree_filesystems;
mod worktree_foreign_detach;
mod worktree_foreign_paths;
mod worktree_foreign_runs;
mod worktree_move;
mod worktree_regions;
mod worktree_runs;
mod worktree_stale_regions;
mod worktrees;
