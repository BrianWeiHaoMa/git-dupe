//! The Git runs one `git dupe` invocation made, read from Git's trace2 event log: every
//! Git process, the one that dispatches `dupe` and each that git-dupe starts, writes one
//! `start` event with its words to the file `GIT_TRACE2_EVENT` names, which git-dupe's
//! runs inherit. A count is compared between two fixtures under one release, or with a
//! sequence the specification names for the command, the same under every release; never
//! with a number observed under one release.
//!
//! Each event carries the process's session identifier, `sid`: a Git process started on
//! behalf of another has the other's identifier, a `/`, and its own. git-dupe is no Git
//! process and hands the dispatcher's identifier on unchanged, so the runs it starts itself
//! are those one level below the dispatcher, the first process to start; a deeper one is a
//! child of a Git command (a `commit` starting `maintenance`).

use std::fs;
use std::io;
use std::path::Path;

use super::output::Output;
use super::scenario::Git;

/// The Git processes of one invocation, in the order they started.
#[derive(Debug)]
pub struct Runs {
    /// Each process's words after the program name.
    words: Vec<Vec<Vec<u8>>>,
    /// Each process's session identifier, in the same order.
    sids: Vec<Vec<u8>>,
}

impl Runs {
    /// The processes git-dupe itself started: those whose session identifier is the
    /// dispatcher's, a `/`, and one more part, whatever level the dispatcher stands at.
    pub fn own(&self) -> Runs {
        let dispatcher = self.sids.first().map(Vec::as_slice).unwrap_or_default();
        let (words, sids) = self
            .words
            .iter()
            .zip(&self.sids)
            .filter(|(_, sid)| {
                sid.strip_prefix(dispatcher)
                    .and_then(|below| below.strip_prefix(b"/"))
                    .is_some_and(|own| !own.contains(&b'/'))
            })
            .map(|(words, sid)| (words.clone(), sid.clone()))
            .unzip();
        Runs { words, sids }
    }

    /// Each Git process's words after the program name, in the order it started.
    pub fn words(&self) -> &[Vec<Vec<u8>>] {
        &self.words
    }

    /// How many Git processes ran, the one that dispatched `dupe` included.
    pub fn count(&self) -> usize {
        self.words.len()
    }

    /// The command word of each, in order: the first word that is no global option or
    /// its value.
    pub fn commands(&self) -> Vec<&[u8]> {
        self.words.iter().map(|words| command_word(words)).collect()
    }

    /// How many ran `command`.
    pub fn of(&self, command: &str) -> usize {
        self.commands()
            .iter()
            .filter(|word| **word == command.as_bytes())
            .count()
    }
}

/// Runs `git` as `Git::run` does, with Git's trace2 event log written to `log`, which is
/// emptied first, and returns what it printed and the Git processes it made.
pub fn run_traced(git: Git<'_>, log: &Path) -> (Output, Runs) {
    match fs::remove_file(log) {
        Ok(()) => {}
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => {}
        Err(cause) => panic!("{}: {cause}", log.display()),
    }
    let output = git.variable("GIT_TRACE2_EVENT", log).run();
    let events = fs::read(log).unwrap_or_else(|cause| panic!("{}: {cause}", log.display()));
    let (words, sids) = events
        .split(|&byte| byte == b'\n')
        .filter(|event| holds(event, br#""event":"start""#))
        .map(|event| {
            let words =
                argv(event).unwrap_or_else(|| panic!("no argv in {}", event.escape_ascii()));
            let sid = sid(event).unwrap_or_else(|| panic!("no sid in {}", event.escape_ascii()));
            (words.into_iter().skip(1).collect(), sid)
        })
        .unzip();
    (output, Runs { words, sids })
}

fn holds(text: &[u8], part: &[u8]) -> bool {
    text.windows(part.len()).any(|window| window == part)
}

/// The first word that is no global option, skipping the value of `-c`, `-C`, and
/// `--config-env`.
fn command_word(words: &[Vec<u8>]) -> &[u8] {
    let mut words = words.iter();
    while let Some(word) = words.next() {
        match word.as_slice() {
            b"-c" | b"-C" | b"--config-env" => {
                words.next();
            }
            option if option.starts_with(b"-") => {}
            command => return command,
        }
    }
    b""
}

/// The event's `"sid":"…"` string, its JSON escapes read back.
fn sid(event: &[u8]) -> Option<Vec<u8>> {
    const KEY: &[u8] = br#""sid":""#;
    let at = event.windows(KEY.len()).position(|window| window == KEY)?;
    let (sid, _) = string(&event[at + KEY.len()..])?;
    Some(sid)
}

/// The strings of the event's `"argv":[…]` array, its JSON escapes read back.
fn argv(event: &[u8]) -> Option<Vec<Vec<u8>>> {
    const KEY: &[u8] = br#""argv":["#;
    let at = event.windows(KEY.len()).position(|window| window == KEY)?;
    let mut rest = &event[at + KEY.len()..];
    let mut words = Vec::new();
    loop {
        match rest.first()? {
            b']' => return Some(words),
            b',' => rest = &rest[1..],
            b'"' => {
                let (word, after) = string(&rest[1..])?;
                words.push(word);
                rest = after;
            }
            _ => return None,
        }
    }
}

/// A JSON string's bytes up to its closing quote, and what follows that quote.
fn string(mut rest: &[u8]) -> Option<(Vec<u8>, &[u8])> {
    let mut word = Vec::new();
    loop {
        let (&byte, after) = rest.split_first()?;
        rest = after;
        match byte {
            b'"' => return Some((word, rest)),
            b'\\' => {
                let (&escaped, after) = rest.split_first()?;
                rest = after;
                match escaped {
                    b'n' => word.push(b'\n'),
                    b't' => word.push(b'\t'),
                    b'r' => word.push(b'\r'),
                    b'b' => word.push(0x08),
                    b'f' => word.push(0x0c),
                    b'u' => {
                        let hex = std::str::from_utf8(rest.get(..4)?).ok()?;
                        let code = u32::from_str_radix(hex, 16).ok()?;
                        let mut encoded = [0; 4];
                        word.extend_from_slice(
                            char::from_u32(code)?.encode_utf8(&mut encoded).as_bytes(),
                        );
                        rest = &rest[4..];
                    }
                    other => word.push(other),
                }
            }
            other => word.push(other),
        }
    }
}
