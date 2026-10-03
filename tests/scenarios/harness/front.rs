//! What scenarios of the front share: the texts `git dupe help` prints.

use super::output::End;
use super::scenario::Scenario;

/// The general text, as `git dupe help` prints it where there is no repository.
pub fn general_text(s: &Scenario) -> Vec<u8> {
    let help = s.git(["dupe", "help"]).run();
    assert_eq!(help.end, End::Code(0), "{help:?}");
    assert!(help.stderr.is_empty(), "{help:?}");
    help.stdout
}

/// The usage line of a text: its first line, the newline included.
pub fn usage_line(text: &[u8]) -> &[u8] {
    let line = text.split_inclusive(|&byte| byte == b'\n').next();
    line.unwrap_or_else(|| panic!("a text has a first line"))
}
