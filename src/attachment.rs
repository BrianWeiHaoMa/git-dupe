//! Attachment: `init`, which makes this worktree's private repository at
//! `<Git directory>/dupe` — `.git/dupe` in the main worktree, `.git/worktrees/<name>/dupe`
//! in a linked one — and gives it the keys F2 and G1 name, `clone`, which makes it as
//! `init` does and attaches it from a remote, and `detach`, which removes it. Every step
//! acts on this worktree's private repository alone (G28).
//!
//! What the listing below cannot show:
//!
//! - Only this part creates a private repository, writes its keys, or removes it, and
//!   every write there is Git's own: `git init`, `git config`, and `clone`'s steps, each a
//!   private run. git-dupe writes no file of its own in it here.
//! - The refusals of G4 are decided before any Git step that writes, by `refusals` for
//!   `init` and `clone` alike; `clone` decides its own before them and G18's after them,
//!   and runs `init`'s steps as its first.
//! - A Git step of `init` or `clone` that fails ends the command with Git's message and
//!   status, the workspace attached to the extent of the steps done; `init` run again
//!   completes it, and `detach --force` and `clone` start a `clone` over.
//! - `core.worktree` set to the relative work tree (`settings`), `../..` in the main
//!   worktree, is the last step of `init`, so that any other value, or none, is an `init`
//!   that did not finish, however it came to be so, a linked worktree moved since
//!   included.
//! - In the working tree, this part has Git write only what `clone`'s write step feeds
//!   it: privately tracked files absent from disk, never over anything present (`write`).
//! - `detach` deletes the region before it removes the private repository, and removes
//!   it only once no region stands, so that a kill at any point leaves what
//!   `detach --force` completes (`State` "`detach`"). That removal is the one deletion
//!   git-dupe makes of its own (R8).

mod clone;
mod detach;
mod init;
mod refusals;
mod settings;
mod write;

pub use clone::{Cloned, NotCloned, clone};
pub use detach::{NotDetached, NotRemoved, detach};
pub use init::{Initialized, NotInitialized, init};
pub use refusals::Refusal;
pub use settings::Unresolved;
pub use write::{Kept, Standing, Written};

use crate::keeper::Failed;
use crate::runner::{End, Finished, Run};

/// Runs one Git step whose exit codes in `answers` are answers, and returns what it
/// captured; any other end is Git's failure, its message already on standard error.
fn step(run: Run, answers: &[u8]) -> Result<Finished, Failed> {
    let finished = run.start().map_err(Failed::NotStarted)?;
    match finished.end {
        End::Code(code) if answers.contains(&code) => Ok(finished),
        _ => Err(Failed::Exited(finished.end)),
    }
}
