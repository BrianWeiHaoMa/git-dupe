//! `stash`'s refusal: an option word that asks Git to stash untracked files (G17,
//! `Holds/G17`). In the private repository every file of the project is untracked, so
//! such a stash would remove the project from disk. Which words Git reads as options of
//! `stash` is the front's reading; this decides over those words alone.

/// Whether `option`, a word Git reads as an option of `stash push`, `save`, or the bare
/// form, spells `-u`, `--include-untracked`, `-a`, or `--all` as Git would read it: in
/// full, as a long prefix of at least three bytes, or as a letter in a bundle before any
/// `m`, after which the rest of the word is the message. A `--no-` form never does.
pub fn untracked(option: &[u8]) -> bool {
    if let Some(long) = option.strip_prefix(b"--") {
        return !long.is_empty()
            && (b"all".starts_with(long) || b"include-untracked".starts_with(long));
    }
    match option.strip_prefix(b"-") {
        Some(letters) => letters
            .iter()
            .take_while(|&&letter| letter != b'm')
            .any(|&letter| letter == b'u' || letter == b'a'),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn untracked_in_full_as_a_prefix_or_in_a_bundle() {
        for option in [
            &b"-u"[..],
            b"-a",
            b"--include-untracked",
            b"--all",
            b"--inc",
            b"--i",
            b"--al",
            b"--a",
            b"-ku",
            b"-qa",
            b"-um",
            b"-kqau",
            b"-uk",
        ] {
            assert!(untracked(option), "{}", option.escape_ascii());
        }
    }

    #[test]
    fn not_untracked_after_m_in_a_negation_or_in_another_option() {
        for option in [
            &b"-m"[..],
            b"-mu",
            b"-qmu",
            b"-ma",
            b"--no-include-untracked",
            b"--no-all",
            b"--",
            b"-",
            b"--message",
            b"--keep-index",
            b"--alls",
            b"--include-untracked-files",
            b"--all=yes",
            b"-k",
            b"-p",
            b"u",
            b"all",
            b"--pathspec-from-file",
        ] {
            assert!(!untracked(option), "{}", option.escape_ascii());
        }
    }
}
