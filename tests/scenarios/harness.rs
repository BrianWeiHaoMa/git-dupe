//! Support for scenarios: the releases and their builds, the scenario environment, and
//! the fixtures and assertions scenarios share. A scenario file starts from what is here
//! and copies none of it; what several scenarios need is added here.

mod attached;
mod clean;
mod concurrent;
mod detach;
mod exclude_lock;
mod files;
mod fresh;
mod front;
mod kill;
mod man;
mod output;
mod provision;
mod public;
mod release_archive;
mod releases;
mod remotes;
mod repository;
mod running;
mod scenario;
mod sha256;
mod stash;
mod sweep;
mod trace;
mod tree;
mod worktrees;

pub use attached::{
    Region, daily_edited, daily_state, gitdupe_written_and_staged, private_add, private_commit,
    region, region_in, region_rules, staged_gitdupe,
};
pub use clean::{EVERY_KIND_SPARED, Twin, hidden_path_of_every_kind};
pub use concurrent::started_together;
pub use detach::{
    daily_warnings, detached, now_visible, stale_file_timestamp, stale_timestamp, warnings,
};
pub use exclude_lock::{commands_wait_for_the_lock, hold_the_lock, lock_is_free};
pub use files::{copy, write};
pub use fresh::FreshDirectory;
pub use front::{general_text, usage_line};
pub use kill::{ForwardEffect, Point};
pub use man::{
    TERMINAL, asking_for_the_page, command_lines, every_text_shown,
    holds_a_line_of_the_general_text, lines_of, manpage, page_texts_present, shown_by_man,
    shown_under, text_present, texts_present,
};
pub use output::{
    End, Output, holds, lines_in_order, names, names_number, names_the_route_to_private, records,
    warnings_in_any_order,
};
pub use public::{
    EVERY_OBJECT, held_publicly, leaving_public_git, privately_tracked, public_git,
    publicly_tracked,
};
pub use release_archive::{install_note, note_commands, release_archive};
pub use releases::oldest_supported;
pub use remotes::{Everything, MANY_URLS, PROJECT_URL, SecondMachine, Transfer};
pub use repository::locate_words;
pub use scenario::{Scenario, report, under_each_release};
pub use stash::{refused_stash_untracked, stash_refusal_first, stash_untracked_line};
pub use sweep::{GITDUPE, Killed, Sweep, Version, copied, files, lived_in, outside};
pub use trace::{Runs, run_traced};
pub use tree::{Tree, changed_since, unchanged};
pub use worktrees::Worktree;
