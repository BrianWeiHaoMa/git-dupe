//! Stash's durable untracked refusal, used to exercise settle without running a command.

use super::output::{End, Output, lines_in_order, names};
use super::scenario::Scenario;

/// The refusal read with a help request outside any repository: add before stash (G17).
pub fn stash_untracked_line(s: &Scenario) -> Vec<u8> {
    let output = s.git(["dupe", "stash", "-u", "-h"]).run();
    assert_eq!(output.end, End::Code(128), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let line = output.only_line("fatal");
    names(line, b"git dupe add");
    names(line, b"git dupe stash");
    let text = String::from_utf8_lossy(line);
    assert!(text.find("git dupe add").unwrap() < text.find("git dupe stash").unwrap());
    line.to_vec()
}

/// Exit 128 with no stdout; the handler's refusal is first, before settle's Git messages.
pub fn stash_refusal_first(s: &Scenario, output: &Output) {
    assert_eq!(output.end, End::Code(128), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let first = [b"fatal: ".as_slice(), &stash_untracked_line(s), b"\n"].concat();
    assert!(output.stderr.starts_with(&first), "{output:?}");
}

/// One fatal refusal line, followed by any settle warnings.
pub fn refused_stash_untracked(s: &Scenario, output: &Output) {
    stash_refusal_first(s, output);
    assert_eq!(
        output.lines("fatal"),
        [stash_untracked_line(s)],
        "{output:?}"
    );
    lines_in_order(output, "fatal", &[b"git dupe add"]);
}
