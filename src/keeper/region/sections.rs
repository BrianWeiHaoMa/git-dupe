//! The regions of `.git/info/exclude` as bytes (F3, `Composition/Keeper`): the markers,
//! where this worktree's region lies, where every other worktree's region lies, and the
//! file composed with this worktree's region set or removed and the other regions the
//! caller found stale left out. Nothing here touches the filesystem.
//!
//! A git-dupe marker is a whole line, without its newline: `# BEGIN git-dupe` and
//! `# END git-dupe` for the main worktree, `# BEGIN git-dupe worktree <name>` and
//! `# END git-dupe worktree <name>` for the linked worktree named `<name>`, any bytes but
//! none. This worktree's region runs from the first line that is its own begin marker to
//! the first line after it that is an end marker of either form, whoever's, or to the end
//! of the file when there is none. In the bytes before and after it, every other begin
//! marker that no region already holds opens another worktree's region, which runs the
//! same way to the first end marker after it, or to the end of those bytes: the same
//! bound, read around this worktree's region, so that no other region overlaps it. A
//! region of this worktree's own markers after its first is no other worktree's: it is
//! copied as the user's text is. Every byte outside this worktree's region — the user's
//! text and every other worktree's region — is copied as it stands, in its order, except
//! the whole lines of an other region the caller leaves out (G27). A region first set is
//! appended at the end of the file, on a line of its own.
//!
//! This is the one reading of the bounds: `start`, the read before `detach`, and the one
//! write all split the file here, so that a file nobody should have edited (E3) is read
//! the same way by each.

use std::ops::Range;

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

/// An exclude file's bytes, around and inside one worktree's region, and the other
/// worktrees' regions around it.
#[derive(Debug, PartialEq, Eq)]
pub struct Sections<'b> {
    /// The bytes before the region: the whole file when there is no region.
    pub before: &'b [u8],
    /// The lines between the markers, without their newlines; `None` without a region.
    pub rules: Option<Vec<&'b [u8]>>,
    /// The bytes after the end marker's line.
    pub after: &'b [u8],
    /// Every other worktree's region, in the file's order.
    pub others: Vec<Other<'b>>,
    bytes: &'b [u8],
}

/// Another worktree's region: whose markers open it, its lines, and the whole lines it
/// spans in the file, its markers' included.
#[derive(Debug, PartialEq, Eq)]
pub struct Other<'b> {
    pub worktree: Worktree<'b>,
    /// The lines between its markers, without their newlines.
    pub rules: Vec<&'b [u8]>,
    span: Range<usize>,
}

impl Other<'_> {
    /// The paths its rules were written for, a line that is no rule read as nothing, as
    /// this worktree's own are read back.
    pub fn paths(&self) -> Vec<Vec<u8>> {
        paths_of(&self.rules)
    }
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
            others: others(bytes, 0..bytes.len(), worktree),
            bytes,
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
    let after_start = bytes.len() - after.len();
    let mut found = others(bytes, 0..begin, worktree);
    found.extend(others(bytes, after_start..bytes.len(), worktree));
    Sections {
        before: &bytes[..begin],
        rules: Some(rules),
        after,
        others: found,
        bytes,
    }
}

/// The other worktrees' regions within `within`, a range of whole lines of `bytes`: each
/// from a begin marker no region holds to the first end marker after it, or to the end of
/// `within`. A region of `own`'s markers is read past and not returned.
fn others<'b>(bytes: &'b [u8], within: Range<usize>, own: Worktree) -> Vec<Other<'b>> {
    let mut found = Vec::new();
    let mut open: Option<Other> = None;
    for (start, line) in lines(&bytes[..within.end]).filter(|(at, _)| *at >= within.start) {
        let edge = marker(line);
        match &mut open {
            None => {
                if let Some((Edge::Begin, worktree)) = edge {
                    open = Some(Other {
                        worktree,
                        rules: Vec::new(),
                        span: start..within.end,
                    });
                }
            }
            Some(region) => {
                if matches!(edge, Some((Edge::End, _))) {
                    region.span.end = (start + line.len() + 1).min(within.end);
                    found.extend(open.take());
                } else {
                    region.rules.push(line);
                }
            }
        }
    }
    found.extend(open);
    found.retain(|other| other.worktree != own);
    found
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

fn paths_of(rules: &[&[u8]]) -> Vec<Vec<u8>> {
    rules.iter().copied().filter_map(rule::path_of).collect()
}

impl Sections<'_> {
    /// The paths the region's rules were written for, a line that is no rule read as
    /// nothing; none without a region.
    pub fn paths(&self) -> Vec<Vec<u8>> {
        self.rules.as_deref().map(paths_of).unwrap_or_default()
    }

    /// The file with `worktree`'s region holding exactly one rule per path, in the order
    /// given, where its markers stood, or after everything else when it had none; each
    /// other region whose entry in `kept`, one per region of `others` in its order, is
    /// false left out.
    pub fn set(&self, worktree: Worktree, paths: &[Vec<u8>], kept: &[bool]) -> Vec<u8> {
        let mut composed = self.copied(0..self.before.len(), kept);
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
        composed.extend(self.copied(self.after_start()..self.bytes.len(), kept));
        composed
    }

    /// The file without the region: the bytes before its begin marker, then those after
    /// its end marker's line, each other region whose entry in `kept` is false left out.
    pub fn removed(&self, kept: &[bool]) -> Vec<u8> {
        let mut composed = self.copied(0..self.before.len(), kept);
        composed.extend(self.copied(self.after_start()..self.bytes.len(), kept));
        composed
    }

    fn after_start(&self) -> usize {
        self.bytes.len() - self.after.len()
    }

    /// The bytes of `range`, before or after this worktree's region, without the other
    /// regions in it that `kept` leaves out.
    fn copied(&self, range: Range<usize>, kept: &[bool]) -> Vec<u8> {
        assert_eq!(kept.len(), self.others.len(), "one entry per other region");
        let mut copied = Vec::with_capacity(range.len());
        let mut at = range.start;
        let dropped = self.others.iter().zip(kept).filter(|(_, kept)| !**kept);
        for (other, _) in dropped {
            if other.span.start >= range.start && other.span.end <= range.end {
                copied.extend_from_slice(&self.bytes[at..other.span.start]);
                at = other.span.end;
            }
        }
        copied.extend_from_slice(&self.bytes[at..range.end]);
        copied
    }
}

#[cfg(test)]
mod tests;
