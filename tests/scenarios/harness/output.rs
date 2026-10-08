//! What a run left behind — its standard output, its standard error, how it ended — and
//! the assertions over it: exactly one line of a level, the lines of a level in order or
//! in any order, a line that names a path or a number; and the records of a listing.

use std::fmt;

/// How a run ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    Code(i32),
    Signal(i32),
}

/// What a run printed, as bytes, and how it ended.
#[derive(PartialEq, Eq)]
pub struct Output {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub end: End,
}

impl Output {
    /// The text of git-dupe's one line when standard error is exactly `level: text`.
    pub fn only_line(&self, level: &str) -> &[u8] {
        self.line_then(level, b"")
    }

    /// The texts of every `level: text` line on standard error, in order. Git's own lines
    /// of that level are among them where Git wrote any.
    pub fn lines(&self, level: &str) -> Vec<&[u8]> {
        let opening = format!("{level}: ");
        self.stderr
            .split(|&byte| byte == b'\n')
            .filter_map(|line| line.strip_prefix(opening.as_bytes()))
            .collect()
    }

    /// The text of git-dupe's line when standard error is exactly one `level: text` line
    /// and then `rest`: a usage error's `error:` line and the usage line after it.
    pub fn line_then(&self, level: &str, rest: &[u8]) -> &[u8] {
        let opening = format!("{level}: ");
        self.stderr
            .strip_prefix(opening.as_bytes())
            .and_then(|after| after.strip_suffix(rest))
            .and_then(|line| line.strip_suffix(b"\n"))
            .filter(|text| !text.contains(&b'\n'))
            .unwrap_or_else(|| {
                panic!(
                    "standard error is not one `{level}:` line and then \"{}\": {self:?}",
                    rest.escape_ascii()
                )
            })
    }
}

impl fmt::Debug for Output {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?}, standard output \"{}\", standard error \"{}\"",
            self.end,
            self.stdout.escape_ascii(),
            self.stderr.escape_ascii()
        )
    }
}

pub fn records(bytes: &[u8], separator: u8) -> Vec<&[u8]> {
    bytes
        .split(|&byte| byte == separator)
        .filter(|record| !record.is_empty())
        .collect()
}

/// Whether `text` holds `part` as a run of its bytes.
pub fn holds(text: &[u8], part: &[u8]) -> bool {
    text.windows(part.len()).any(|window| window == part)
}

/// The command `line` offers: the bytes after its first `opening` and before the first
/// `closing` after that, to be run as the line shows it (G25).
pub fn offered<'l>(line: &'l [u8], opening: &[u8], closing: &[u8]) -> &'l [u8] {
    let at = |from: usize, part: &[u8]| {
        line[from..]
            .windows(part.len())
            .position(|window| window == part)
            .map(|found| from + found)
    };
    let start = at(0, opening)
        .unwrap_or_else(|| panic!("{} offers no command", line.escape_ascii()))
        + opening.len();
    let end = at(start, closing)
        .unwrap_or_else(|| panic!("{} does not close its offer", line.escape_ascii()));
    &line[start..end]
}

/// `line` names `part`: it holds it as a run of its bytes.
pub fn names(line: &[u8], part: &[u8]) {
    assert!(
        holds(line, part),
        "{} does not name {}",
        line.escape_ascii(),
        part.escape_ascii()
    );
}

/// `line` names the route by which a path the project's Git tracks becomes private (G11,
/// G14, N9): `git rm --cached`, said to be a deletion the project's other clones receive,
/// and after it `git dupe add`, and never that `git rm --cached` alone makes it private.
pub fn names_the_route_to_private(line: &[u8]) {
    for part in [
        &b"a deletion from the project"[..],
        b"every other clone",
        b"once it is committed and pushed",
    ] {
        names(line, part);
    }
    assert!(
        !holds(line, b"first makes it private"),
        "{}",
        line.escape_ascii()
    );
    let at = |part: &[u8]| line.windows(part.len()).position(|window| window == part);
    let (Some(removal), Some(addition)) = (at(b"'git rm --cached"), at(b"'git dupe add")) else {
        panic!("{} names no route to private", line.escape_ascii());
    };
    assert!(removal < addition, "{}", line.escape_ascii());
}

/// `line` names the count `expected`: one whole run of its digits is that count in
/// decimal.
pub fn names_number(line: &[u8], expected: usize) {
    let expected = expected.to_string();
    assert!(
        line.split(|byte| !byte.is_ascii_digit())
            .any(|part| part == expected.as_bytes()),
        "{} does not name number {expected}",
        line.escape_ascii()
    );
}

/// The `level:` lines are exactly as many as `expected` and name its parts in order; and,
/// for a level other than `warning`, the last of them comes before the first `warning:`,
/// because settle's warnings follow the handler's own lines.
pub fn lines_in_order(output: &Output, level: &str, expected: &[&[u8]]) {
    let found = output.lines(level);
    assert_eq!(found.len(), expected.len(), "{output:?}");
    for (line, name) in found.iter().zip(expected) {
        names(line, name);
    }
    // Settle warnings follow the handler's hints or refusal.
    if !found.is_empty() && level != "warning" {
        let marker = format!("{level}: ");
        let last = output
            .stderr
            .windows(marker.len())
            .rposition(|part| part == marker.as_bytes())
            .unwrap();
        if let Some(first_warning) = output
            .stderr
            .windows(9)
            .position(|part| part == b"warning: ")
        {
            assert!(last < first_warning, "{output:?}");
        }
    }
}

/// The `warning:` lines are exactly as many as `names`, in any order, and each entry of
/// `names` — the parts one warning must hold together — matches exactly one of them.
pub fn warnings_in_any_order(output: &Output, names: &[&[&[u8]]]) {
    let lines = output.lines("warning");
    assert_eq!(lines.len(), names.len(), "{output:?}");
    for parts in names {
        assert_eq!(
            lines
                .iter()
                .filter(|line| parts.iter().all(|part| holds(line, part)))
                .count(),
            1,
            "names {parts:?}: {output:?}"
        );
    }
}
