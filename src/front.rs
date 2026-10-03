//! Front: the words and the lines.
//!
//! What the listing below cannot show:
//!
//! - Every invocation goes through the one sequence in `sequence`, and no handler is
//!   reachable around it. A handler ends by returning an `Outcome`; nothing here ends
//!   the process, so that whatever the sequence runs after a handler runs after every
//!   handler, its refusals included. A usage error is reported before any handler runs,
//!   and no settle follows it, nor any `detach`, which G6 does not cover (R3).
//! - Every line of git-dupe's own is written by `lines`, to standard error. Standard
//!   output carries only the help texts, the usage line of bare `git dupe`, and the
//!   version line, written by `outcome::print`.
//! - `commands` holds F6's words, each of which has a handler here. A word F6 does not
//!   name is resolved through the aliases by `alias`, and either runs as Git itself
//!   through `passthrough` or goes on as the command of F6 its chain reaches, through that
//!   command's reading, table, guard, or refusal; `stash`, `push`, `pull`, `fetch`, and
//!   `remote` run through `passthrough` after their guards, the last four's comparing
//!   what they would reach with the public places (`transfer`, `places`), as `clone`
//!   compares its words before it attaches.
//! - Words, paths, and lines are bytes. Nothing is printed through `println!` or
//!   `eprintln!`, which panic when the write fails.

mod add;
mod alias;
mod clean;
mod clone;
mod commands;
mod detach;
mod git;
mod help;
mod hide;
mod init;
mod lines;
mod outcome;
mod passthrough;
mod places;
mod sequence;
mod stash;
mod status;
mod table;
mod transfer;

pub use sequence::run;
