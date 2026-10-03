//! The fifteen words F6 names, and Git's synonym of one of them.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    // Own.
    Init,
    Clone,
    Detach,
    Hide,
    Unhide,
    Git,
    Help,
    // Changed.
    Status,
    Add,
    Clean,
    // Guarded.
    Stash,
    Push,
    Pull,
    Fetch,
    Remote,
}

/// Each word is written here and nowhere else in the executable.
const WORDS: [(&[u8], Command); 15] = [
    (b"init", Command::Init),
    (b"clone", Command::Clone),
    (b"detach", Command::Detach),
    (b"hide", Command::Hide),
    (b"unhide", Command::Unhide),
    (b"git", Command::Git),
    (b"help", Command::Help),
    (b"status", Command::Status),
    (b"add", Command::Add),
    (b"clean", Command::Clean),
    (b"stash", Command::Stash),
    (b"push", Command::Push),
    (b"pull", Command::Pull),
    (b"fetch", Command::Fetch),
    (b"remote", Command::Remote),
];

/// Git's builtin synonym of `add`, read as the word `add` wherever git-dupe reads a
/// command word: typed first, first in an alias's expansion, after `help`, and before
/// `-h` (F6). `git dupe git stage` is Git's own, because those words are not read.
const STAGE: &[u8] = b"stage";

/// The command a word names, exactly as typed, `stage` naming `add`.
pub fn named(word: &[u8]) -> Option<Command> {
    if word == STAGE {
        return Some(Command::Add);
    }
    WORDS
        .iter()
        .find(|(name, _)| *name == word)
        .map(|(_, command)| *command)
}

impl Command {
    /// The command's own word, as F6 writes it: what its texts and lines name.
    pub fn word(self) -> &'static [u8] {
        WORDS
            .iter()
            .find(|(_, command)| *command == self)
            .map(|(word, _)| *word)
            .expect("every command has its word")
    }

    /// `init` and `clone` are what attaches a workspace; every other command but `help`
    /// requires an attached one (G4).
    pub fn attaches(self) -> bool {
        matches!(self, Command::Init | Command::Clone)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_word_is_distinct_and_names_its_own_command() {
        for (index, (word, command)) in WORDS.iter().enumerate() {
            assert_eq!(named(word), Some(*command));
            assert_eq!(command.word(), *word);
            assert!(WORDS[..index].iter().all(|(_, earlier)| earlier != command));
        }
        assert_eq!(named(b"log"), None);
        assert_eq!(named(b"Help"), None);
        assert_eq!(named(b""), None);
    }

    #[test]
    fn stage_is_read_as_add_and_no_other_spelling_is() {
        assert_eq!(named(b"stage"), Some(Command::Add));
        assert_eq!(Command::Add.word(), b"add");
        assert_eq!(named(b"Stage"), None);
        assert_eq!(named(b"stage "), None);
    }
}
