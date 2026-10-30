//! How a Git run whose answer git-dupe reads failed, and the cause a line names for it.

use crate::guards::quoted::quoted;
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
            Failed::NotStarted(Failure::NotAName(path)) => [
                &quoted(path)[..],
                b" cannot be named on a command line here: it is not valid UTF-8",
            ]
            .concat(),
            Failed::NotStarted(Failure::Other(cause)) => {
                format!("cannot run git: {cause}").into_bytes()
            }
            Failed::Unreadable => b"git's answer cannot be read".to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_word_that_is_no_name_is_named_as_a_path_is() {
        let failed = Failed::NotStarted(Failure::NotAName(b"notes/\xff".to_vec()));
        assert_eq!(
            failed.cause(),
            b"\"notes/\xff\" cannot be named on a command line here: it is not valid UTF-8"
        );
    }
}
