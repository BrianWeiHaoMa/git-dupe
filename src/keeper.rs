//! Keeper: the hidden paths, the managed region, `.gitdupe`, and what public Git says
//! about each hidden path.
//!
//! What the listing below cannot show:
//!
//! - Only `settle` and `remove` write the region, through `region`'s one write, always
//!   whole through `replace`, its rules always in `rule`'s form: one write, one rule form,
//!   one reading back (G6, F3). `remove` only deletes it, for `detach`, which never
//!   settles (R3). Only `edit` writes `.gitdupe`, always whole through `replace`, and only
//!   after its caller has decided the refusals over a listing it took (F5, G11).
//! - Every path crossing the keeper's boundary is bytes relative to the root, without a
//!   leading or trailing slash.
//! - Every Git run is one `ls-files`, `diff --cached`, `cat-file`, `check-ignore`, or
//!   `add` over a whole list, never one per path (G22, R4); a list is never shortened to
//!   fit (G23).
//! - `rule` is the one rule form of a path, the region's and the patterns of `clean`
//!   alike (`Composition/Keeper`), and `publicly_ignored` is the exposure question asked
//!   for `clean`: neither is written a second way elsewhere.
//! - A failed read never removes a rule: a listing that settle cannot take leaves the
//!   region as it is. `remove` deletes the region whole whatever was read, because
//!   `detach` leaves none; under `--force` a listing that failed leaves the paths it asks
//!   about to `.gitdupe` on disk and the region it deletes (`Holds/G3`).
//! - Every file the keeper writes is written to a fresh file inside the private Git
//!   directory and renamed into place (G21, `Foundations/Storage`).

mod edit;
mod exposure;
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
pub use gitdupe::NAME as GITDUPE;
pub use listing::{
    public as publicly_tracked, staged_deletions, users_index as tracked_in_the_users_index,
};
pub use region::{NotDeleted, StartingRegion, beyond_a_link as region_beyond_a_link, start};
pub use remove::remove;
pub use rule::of as rule;
pub use settle::settle;

use crate::runner::{End, Failure};

/// How a Git run whose answer git-dupe reads failed: the keeper's, and the steps of
/// `init` and `detach`, which end the command with it.
pub enum Failed {
    /// Git ran and exited other than as an answer, its message already on standard
    /// error.
    Exited(End),
    /// The run's list of this many paths did not fit on one command line; nothing ran.
    TooLong(usize),
    /// The run did not start.
    NotStarted(Failure),
    /// Git ran, and its answer does not answer what it was asked.
    Unreadable,
}

impl Failed {
    /// The cause, as a line names it between parentheses.
    pub fn cause(&self) -> Vec<u8> {
        match self {
            Failed::Exited(End::Code(code)) => format!("git exited with {code}").into_bytes(),
            Failed::Exited(End::Signal(signal)) => {
                format!("git was killed by signal {signal}").into_bytes()
            }
            Failed::TooLong(count) => format!("a list of {count} paths is too long").into_bytes(),
            Failed::NotStarted(Failure::TooLong) => b"the command line is too long".to_vec(),
            Failed::NotStarted(Failure::Other(cause)) => {
                format!("cannot run git: {cause}").into_bytes()
            }
            Failed::Unreadable => b"git's answer cannot be read".to_vec(),
        }
    }
}
