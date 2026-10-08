//! The regions of `.git/info/exclude` as bytes (F3, `Composition/Keeper`): the markers,
//! where this worktree's region lies, and the file composed with that region set or
//! removed. Nothing here touches the filesystem.
//!
//! A git-dupe marker is a whole line, without its newline: `# BEGIN git-dupe` and
//! `# END git-dupe` for the main worktree, `# BEGIN git-dupe worktree <name>` and
//! `# END git-dupe worktree <name>` for the linked worktree named `<name>`, any bytes but
//! none. This worktree's region runs from the first line that is its own begin marker to
//! the first line after it that is an end marker of either form, whoever's, or to the end
//! of the file when there is none. Every other byte — the user's text and every other
//! worktree's region — lies before or after it and is copied as it stands, in its order.
//! A region first set is appended at the end of the file, on a line of its own.
//!
//! This is the one reading of the bounds: `start`, the read before `detach`, and the one
//! write all split the file here, so that a file nobody should have edited (E3) is read
//! the same way by each.

use crate::keeper::rule;

const BEGIN: &[u8] = b"# BEGIN git-dupe";
const END: &[u8] = b"# END git-dupe";
/// What follows a marker's first words for a linked worktree, before its name.
const WORKTREE: &[u8] = b" worktree ";

/// Whose region a marker bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Worktree<'n> {
    Main,
    /// The linked worktree of this name, as bytes.
    Linked(&'n [u8]),
}

impl Worktree<'_> {
    fn begin(self) -> Vec<u8> {
        self.marker(BEGIN)
    }

    fn end(self) -> Vec<u8> {
        self.marker(END)
    }

    fn marker(self, first: &[u8]) -> Vec<u8> {
        match self {
            Worktree::Main => first.to_vec(),
            Worktree::Linked(name) => [first, WORKTREE, name].concat(),
        }
    }
}

/// Which marker a line is, and whose; `None` for any other line.
fn marker(line: &[u8]) -> Option<(Edge, Worktree<'_>)> {
    let (edge, rest) = if let Some(rest) = line.strip_prefix(BEGIN) {
        (Edge::Begin, rest)
    } else {
        (Edge::End, line.strip_prefix(END)?)
    };
    if rest.is_empty() {
        return Some((edge, Worktree::Main));
    }
    match rest.strip_prefix(WORKTREE) {
        Some(name) if !name.is_empty() => Some((edge, Worktree::Linked(name))),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Edge {
    Begin,
    End,
}

/// An exclude file's bytes, around and inside one worktree's region.
#[derive(Debug, PartialEq, Eq)]
pub struct Sections<'b> {
    /// The bytes before the region: the whole file when there is no region.
    pub before: &'b [u8],
    /// The lines between the markers, without their newlines; `None` without a region.
    pub rules: Option<Vec<&'b [u8]>>,
    /// The bytes after the end marker's line.
    pub after: &'b [u8],
}

/// The file's bytes around and inside `worktree`'s region.
pub fn split<'b>(bytes: &'b [u8], worktree: Worktree) -> Sections<'b> {
    let mut lines = lines(bytes);
    let Some((begin, _)) = lines.find(|(_, line)| marker(line) == Some((Edge::Begin, worktree)))
    else {
        return Sections {
            before: bytes,
            rules: None,
            after: b"",
        };
    };
    let mut rules = Vec::new();
    let mut after: &[u8] = b"";
    for (start, line) in lines {
        if matches!(marker(line), Some((Edge::End, _))) {
            let end = start + line.len();
            after = bytes.get(end + 1..).unwrap_or_default();
            break;
        }
        rules.push(line);
    }
    Sections {
        before: &bytes[..begin],
        rules: Some(rules),
        after,
    }
}

/// Each line with the offset it starts at, without its newline; the empty piece after a
/// final newline is no line.
fn lines(bytes: &[u8]) -> impl Iterator<Item = (usize, &[u8])> {
    let mut start = 0;
    bytes
        .split(|&byte| byte == b'\n')
        .map(move |line| {
            let at = start;
            start += line.len() + 1;
            (at, line)
        })
        .filter(move |(at, _)| *at < bytes.len())
}

impl Sections<'_> {
    /// The paths the region's rules were written for, a line that is no rule read as
    /// nothing; none without a region.
    pub fn paths(&self) -> Vec<Vec<u8>> {
        self.rules
            .iter()
            .flatten()
            .copied()
            .filter_map(rule::path_of)
            .collect()
    }

    /// The file with `worktree`'s region holding exactly one rule per path, in the order
    /// given, where its markers stood, or after everything else when it had none.
    pub fn set(&self, worktree: Worktree, paths: &[Vec<u8>]) -> Vec<u8> {
        let mut composed = self.before.to_vec();
        if !composed.is_empty() && !composed.ends_with(b"\n") {
            composed.push(b'\n');
        }
        composed.extend_from_slice(&worktree.begin());
        composed.push(b'\n');
        for path in paths {
            composed.extend_from_slice(&rule::of(path));
            composed.push(b'\n');
        }
        composed.extend_from_slice(&worktree.end());
        composed.push(b'\n');
        composed.extend_from_slice(self.after);
        composed
    }

    /// The file without the region: the bytes before its begin marker, then those after
    /// its end marker's line.
    pub fn removed(&self) -> Vec<u8> {
        [self.before, self.after].concat()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAIN: Worktree = Worktree::Main;
    const AGENT: Worktree = Worktree::Linked(b"agent");

    fn paths(paths: &[&[u8]]) -> Vec<Vec<u8>> {
        paths.iter().map(|path| path.to_vec()).collect()
    }

    #[test]
    fn a_file_without_a_region_is_all_the_users_and_the_region_is_appended() {
        for (file, composed) in [
            (
                &b""[..],
                &b"# BEGIN git-dupe\n/.gitdupe\n# END git-dupe\n"[..],
            ),
            (
                b"*.o\n",
                b"*.o\n# BEGIN git-dupe\n/.gitdupe\n# END git-dupe\n",
            ),
            // The begin marker starts a line; the user's bytes stay a prefix.
            (
                b"*.o",
                b"*.o\n# BEGIN git-dupe\n/.gitdupe\n# END git-dupe\n",
            ),
            (
                b"a\r\n",
                b"a\r\n# BEGIN git-dupe\n/.gitdupe\n# END git-dupe\n",
            ),
        ] {
            let found = split(file, MAIN);
            assert_eq!(found.before, file);
            assert_eq!(found.rules, None);
            assert_eq!(found.set(MAIN, &paths(&[b".gitdupe"])), composed);
        }
    }

    #[test]
    fn a_region_is_rewritten_where_it_stands_with_the_users_bytes_around_it() {
        let file = b"*.o\n# BEGIN git-dupe\n/.gitdupe\n/old\n# END git-dupe\nafter\nlast";
        let found = split(file, MAIN);
        assert_eq!(found.before, b"*.o\n");
        assert_eq!(found.rules, Some(vec![&b"/.gitdupe"[..], b"/old"]));
        assert_eq!(found.after, b"after\nlast");
        assert_eq!(
            found.set(MAIN, &paths(&[b".gitdupe", b"new"])),
            b"*.o\n# BEGIN git-dupe\n/.gitdupe\n/new\n# END git-dupe\nafter\nlast"
        );
    }

    #[test]
    fn a_begin_marker_without_an_end_marker_runs_to_the_end_of_the_file() {
        for file in [
            &b"x\n# BEGIN git-dupe\n/.gitdupe\n/old\n"[..],
            b"x\n# BEGIN git-dupe\n/.gitdupe\n/old",
        ] {
            let found = split(file, MAIN);
            assert_eq!(found.before, b"x\n");
            assert_eq!(found.rules, Some(vec![&b"/.gitdupe"[..], b"/old"]));
            assert_eq!(found.after, b"");
            assert_eq!(
                found.set(MAIN, &paths(&[b".gitdupe"])),
                b"x\n# BEGIN git-dupe\n/.gitdupe\n# END git-dupe\n"
            );
        }
        // A linked worktree's the same way, and its markers are its own.
        let found = split(b"x\n# BEGIN git-dupe worktree agent\n/a", AGENT);
        assert_eq!(found.rules, Some(vec![&b"/a"[..]]));
        assert_eq!(
            found.set(AGENT, &paths(&[b"b"])),
            b"x\n# BEGIN git-dupe worktree agent\n/b\n# END git-dupe worktree agent\n"
        );
    }

    #[test]
    fn the_first_own_begin_marker_and_the_first_end_marker_after_it_bound_the_region() {
        let file = b"# END git-dupe\n# BEGIN git-dupe\n/a\n# END git-dupe\n# BEGIN git-dupe\n# END git-dupe\n";
        let found = split(file, MAIN);
        assert_eq!(found.before, b"# END git-dupe\n");
        assert_eq!(found.rules, Some(vec![&b"/a"[..]]));
        assert_eq!(found.after, b"# BEGIN git-dupe\n# END git-dupe\n");
        let file = b"# BEGIN git-dupe worktree agent\n/a\n# END git-dupe worktree agent\n\
                     # BEGIN git-dupe worktree agent\n/b\n# END git-dupe worktree agent\n";
        let found = split(file, AGENT);
        assert_eq!(found.before, b"");
        assert_eq!(found.rules, Some(vec![&b"/a"[..]]));
        assert_eq!(
            found.after,
            b"# BEGIN git-dupe worktree agent\n/b\n# END git-dupe worktree agent\n"
        );
    }

    #[test]
    fn an_end_marker_of_either_form_ends_the_region() {
        // Another worktree's end marker, and the main one's, end a linked worktree's region,
        // and the main region the other way round; whatever follows is copied.
        for (file, worktree, rules, after) in [
            (
                &b"# BEGIN git-dupe\n/a\n# END git-dupe worktree agent\nuser\n"[..],
                MAIN,
                vec![&b"/a"[..]],
                &b"user\n"[..],
            ),
            (
                b"# BEGIN git-dupe worktree agent\n/a\n# END git-dupe\nuser\n",
                AGENT,
                vec![b"/a"],
                b"user\n",
            ),
            (
                b"# BEGIN git-dupe worktree agent\n/a\n# END git-dupe worktree other\nuser\n",
                AGENT,
                vec![b"/a"],
                b"user\n",
            ),
            // A begin marker inside the region is one of its lines; the first end marker
            // after the own begin marker ends it all the same.
            (
                b"# BEGIN git-dupe\n/a\n# BEGIN git-dupe worktree agent\n/b\n\
                  # END git-dupe worktree agent\nuser\n",
                MAIN,
                vec![b"/a", b"# BEGIN git-dupe worktree agent", b"/b"],
                b"user\n",
            ),
        ] {
            let found = split(file, worktree);
            assert_eq!(found.before, b"", "{}", file.escape_ascii());
            assert_eq!(found.rules, Some(rules), "{}", file.escape_ascii());
            assert_eq!(found.after, after, "{}", file.escape_ascii());
        }
    }

    #[test]
    fn a_marker_is_a_whole_line() {
        for file in [
            &b"  # BEGIN git-dupe\n/a\n"[..],
            b"# BEGIN git-dupe \n/a\n",
            b"# BEGIN git-dupe\r\n/a\n",
            // A linked worktree's marker is not the main one's, whatever its name.
            b"# BEGIN git-dupe worktree agent\n/a\n",
            b"# BEGIN git-dupe worktree\n/a\n",
            b"# BEGIN git-dupe worktree \n/a\n",
        ] {
            assert_eq!(split(file, MAIN).rules, None, "{}", file.escape_ascii());
        }
        let found = split(b"# BEGIN git-dupe\n/a\n# END git-dupe\r\nx\n", MAIN);
        assert_eq!(
            found.rules,
            Some(vec![&b"/a"[..], b"# END git-dupe\r", b"x"])
        );
        // A name is the whole rest of the line: one that only begins with this one's, or
        // has a trailing byte, is another worktree's.
        for file in [
            &b"# BEGIN git-dupe worktree agent2\n/a\n"[..],
            b"# BEGIN git-dupe worktree agen\n/a\n",
            b"# BEGIN git-dupe worktree agent \n/a\n",
            b"# BEGIN git-dupe worktree agent\r\n/a\n",
            b"# BEGIN git-dupe\n/a\n",
            b"# BEGIN git-dupe  worktree agent\n/a\n",
        ] {
            assert_eq!(split(file, AGENT).rules, None, "{}", file.escape_ascii());
        }
        // Lines that are no marker of either form end nothing.
        let found = split(
            b"# BEGIN git-dupe\n/a\n# END git-dupe worktree\n# END git-dupe worktree \n\
              # END git-dupe \n#END git-dupe\n",
            MAIN,
        );
        assert_eq!(found.rules.map(|rules| rules.len()), Some(5));
        assert_eq!(found.after, b"");
    }

    #[test]
    fn an_end_marker_without_a_final_newline_ends_the_file() {
        let found = split(b"# BEGIN git-dupe\n/a\n# END git-dupe", MAIN);
        assert_eq!(found.before, b"");
        assert_eq!(found.rules, Some(vec![&b"/a"[..]]));
        assert_eq!(found.after, b"");
        let found = split(
            b"# BEGIN git-dupe worktree agent\n/a\n# END git-dupe",
            AGENT,
        );
        assert_eq!(found.rules, Some(vec![&b"/a"[..]]));
        assert_eq!(found.after, b"");
    }

    /// The user's text, the main region, and two linked worktrees' regions, in this order.
    const THREE: &[u8] = b"user before\n\
        # BEGIN git-dupe worktree agent\n/a\n# END git-dupe worktree agent\n\
        between\n\
        # BEGIN git-dupe\n/.gitdupe\n/main\n# END git-dupe\n\
        # BEGIN git-dupe worktree caf\xe9\n/caf\xe9\n# END git-dupe worktree caf\xe9\n\
        user after";

    #[test]
    fn each_worktrees_region_is_read_set_and_removed_alone() {
        let cafe = Worktree::Linked(b"caf\xe9");
        assert_eq!(split(THREE, MAIN).paths(), paths(&[b".gitdupe", b"main"]));
        assert_eq!(split(THREE, AGENT).paths(), paths(&[b"a"]));
        assert_eq!(split(THREE, cafe).paths(), paths(&[b"caf\xe9"]));
        assert_eq!(split(THREE, Worktree::Linked(b"gone")).rules, None);

        let set = paths(&[b".gitdupe", b"n\xffew"]);
        assert_eq!(
            split(THREE, MAIN).set(MAIN, &set),
            b"user before\n\
              # BEGIN git-dupe worktree agent\n/a\n# END git-dupe worktree agent\n\
              between\n\
              # BEGIN git-dupe\n/.gitdupe\n/n\xffew\n# END git-dupe\n\
              # BEGIN git-dupe worktree caf\xe9\n/caf\xe9\n# END git-dupe worktree caf\xe9\n\
              user after"
        );
        assert_eq!(
            split(THREE, AGENT).set(AGENT, &set),
            b"user before\n\
              # BEGIN git-dupe worktree agent\n/.gitdupe\n/n\xffew\n# END git-dupe worktree agent\n\
              between\n\
              # BEGIN git-dupe\n/.gitdupe\n/main\n# END git-dupe\n\
              # BEGIN git-dupe worktree caf\xe9\n/caf\xe9\n# END git-dupe worktree caf\xe9\n\
              user after"
        );
        assert_eq!(
            split(THREE, cafe).set(cafe, &set),
            b"user before\n\
              # BEGIN git-dupe worktree agent\n/a\n# END git-dupe worktree agent\n\
              between\n\
              # BEGIN git-dupe\n/.gitdupe\n/main\n# END git-dupe\n\
              # BEGIN git-dupe worktree caf\xe9\n/.gitdupe\n/n\xffew\n# END git-dupe worktree caf\xe9\n\
              user after"
        );

        assert_eq!(
            split(THREE, MAIN).removed(),
            b"user before\n\
              # BEGIN git-dupe worktree agent\n/a\n# END git-dupe worktree agent\n\
              between\n\
              # BEGIN git-dupe worktree caf\xe9\n/caf\xe9\n# END git-dupe worktree caf\xe9\n\
              user after"
        );
        assert_eq!(
            split(THREE, AGENT).removed(),
            b"user before\n\
              between\n\
              # BEGIN git-dupe\n/.gitdupe\n/main\n# END git-dupe\n\
              # BEGIN git-dupe worktree caf\xe9\n/caf\xe9\n# END git-dupe worktree caf\xe9\n\
              user after"
        );
    }

    #[test]
    fn a_missing_region_is_appended_after_every_other_one() {
        // After the other regions and the user's last line, which had no newline.
        let file = b"# BEGIN git-dupe worktree agent\n/a\n# END git-dupe worktree agent\nlast";
        assert_eq!(
            split(file, MAIN).set(MAIN, &paths(&[b".gitdupe"])),
            b"# BEGIN git-dupe worktree agent\n/a\n# END git-dupe worktree agent\nlast\n\
              # BEGIN git-dupe\n/.gitdupe\n# END git-dupe\n"
        );
        // Removing a region that is not there is the file as it was.
        assert_eq!(split(file, MAIN).removed(), file);
        assert_eq!(split(THREE, Worktree::Linked(b"gone")).removed(), THREE);
    }

    #[test]
    fn deleting_the_region_keeps_the_users_bytes_around_it_and_nothing_else() {
        let file = b"*.o\n# BEGIN git-dupe\n/.gitdupe\n/notes\n# END git-dupe\nafter\nlast";
        assert_eq!(split(file, MAIN).removed(), b"*.o\nafter\nlast");
        assert_eq!(
            split(b"# BEGIN git-dupe\n/.gitdupe\n# END git-dupe\n", MAIN).removed(),
            b""
        );
        // A begin marker without an end marker: everything from it goes.
        assert_eq!(
            split(b"x\n# BEGIN git-dupe\n/a\n# END git-dupe \ny\n", MAIN).removed(),
            b"x\n"
        );
        // The newline composing put before the begin marker stays: it cannot be told from
        // the user's own.
        let composed = split(b"*.o", MAIN).set(MAIN, &paths(&[b".gitdupe"]));
        assert_eq!(split(&composed, MAIN).removed(), b"*.o\n");
    }

    #[test]
    fn composing_what_was_composed_changes_nothing() {
        let region = paths(&[b".gitdupe", b"has space", b"cr\rx"]);
        for worktree in [MAIN, AGENT, Worktree::Linked(b"caf\xe9")] {
            let once = split(THREE, worktree).set(worktree, &region);
            assert_eq!(split(&once, worktree).set(worktree, &region), once);
            assert_eq!(split(&once, worktree).paths(), region);
        }
    }
}
