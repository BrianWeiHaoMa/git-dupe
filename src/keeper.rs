//! Keeper: the hidden paths, the managed region, `.gitdupe`, and what public Git says
//! about each hidden path.
//!
//! What the listing below cannot show:
//!
//! - Only `settle` and `remove` write the region, through `region`'s one `update`, the one
//!   site that reads `.git/info/exclude` to write it and the one that takes the lock every
//!   worktree's command shares, held there across no Git run (R10, `Holds/G28`), always
//!   whole through `replace`, its rules always in `rule`'s form, its bounds always
//!   `region`'s one reading of them: one write, one rule form, one reading back (G6,
//!   F3). `remove` only deletes it, for `detach`, which never settles (R3). The user's
//!   text and every other worktree's region are copied through it as they stand, except
//!   a region whose worktree holds no private repository, which the write leaves out
//!   (G27); the other regions it read are handed back once, as `foreign` reads them. Only
//!   `edit` writes `.gitdupe`, always whole through `replace`, and only after its caller
//!   has decided the refusals over a listing it took (F5, G11).
//! - Every path crossing the keeper's boundary is bytes relative to the root, without a
//!   leading or trailing slash.
//! - Every Git run is one `ls-files`, `diff --cached`, `cat-file`, `check-ignore`, or
//!   `add` over a whole list, never one per path (G22, R4); a list is never shortened to
//!   fit (G23).
//! - `rule` is the one rule form of a path, the region's and the patterns of `clean`
//!   alike (`Composition/Keeper`), and `publicly_ignored` is the exposure question asked
//!   for `clean`: neither is written a second way elsewhere.
//! - A failed read never removes a rule: the private listing that settle cannot take
//!   leaves the region as it is. The public listing comes after the replacement, its
//!   query holding the foreign paths, and one that fails leaves exposure unchecked, not a
//!   rule dropped. `remove` deletes the region whole whatever was read, because
//!   `detach` leaves none; under `--force` a listing that failed leaves the paths it asks
//!   about to `.gitdupe` on disk and the region it deletes (`Holds/G3`).
//! - Every file the keeper writes is written to a fresh file inside the private Git
//!   directory and renamed into place (G21, `Foundations/Storage`).

mod edit;
mod exposure;
mod failed;
mod foreign;
mod gitdupe;
mod hidden;
mod listing;
mod region;
mod remove;
mod replace;
mod rule;
mod settle;

pub use edit::{Edited, Hidden, NotEdited, hidden, hide, hiding, unhide};
pub use exposure::{Answered, beyond_a_link, ignored as publicly_ignored, link_above};
pub use failed::Failed;
pub use gitdupe::NAME as GITDUPE;
pub use listing::{
    public as publicly_tracked, staged_deletions, users_index as tracked_in_the_users_index,
};
pub use region::{NotDeleted, StartingRegion, beyond_a_link as region_beyond_a_link, start};
pub use remove::remove;
pub use rule::of as rule;
pub use settle::settle;
