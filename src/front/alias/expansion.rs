//! An alias's value as Git reads it before running a command from it (S11): split into
//! words, and its leading global options set apart from its command word.

/// An expansion Git runs a command from: its leading global options, its command word, and
/// the words after it.
#[derive(Debug, PartialEq, Eq)]
pub struct Link {
    pub options: Vec<Vec<u8>>,
    pub command: Vec<u8>,
    pub rest: Vec<Vec<u8>>,
}

impl Link {
    /// The link an alias's value makes, or `None` where Git runs no command of git-dupe's
    /// from it: a `!` alias; an expansion Git refuses; one whose leading options are not
    /// all of those Git accepts inside an alias — `-c` and `--config-env` with their
    /// values, `-p` or `--paginate`, `--exec-path=<path>`, and `-C ""` (S11) — or leave no
    /// command word.
    pub fn of(value: &[u8]) -> Option<Link> {
        if value.starts_with(b"!") {
            return None;
        }
        let mut words = split(value)?.into_iter();
        let mut options = Vec::new();
        while let Some(word) = words.next() {
            match word.as_slice() {
                b"-c" | b"--config-env" => {
                    let setting = words.next()?;
                    options.extend([word, setting]);
                }
                b"-C" => {
                    let directory = words.next()?;
                    if !directory.is_empty() {
                        return None;
                    }
                    options.extend([word, directory]);
                }
                b"-p" | b"--paginate" => options.push(word),
                option
                    if option.starts_with(b"--exec-path=")
                        || option.starts_with(b"--config-env=") =>
                {
                    options.push(word);
                }
                option if option.starts_with(b"-") => return None,
                _ => {
                    return Some(Link {
                        options,
                        command: word,
                        rest: words.collect(),
                    });
                }
            }
        }
        None
    }
}

/// An alias's value split as Git splits it (S11): words at unquoted whitespace — space,
/// tab, line feed, carriage return — a run of it at the start yielding an empty first word
/// and one at the end an empty last word; single and double quotes; a backslash taking the
/// next byte as it is, outside single quotes. `None` for a value Git refuses: an open
/// quote, or a final backslash.
fn split(value: &[u8]) -> Option<Vec<Vec<u8>>> {
    let blank = |byte: &u8| matches!(byte, b' ' | b'\t' | b'\n' | b'\r');
    let mut words = vec![Vec::new()];
    let mut quoted = None;
    let mut bytes = value.iter().copied().peekable();
    while let Some(byte) = bytes.next() {
        let word = words.last_mut().expect("a word");
        match (quoted, byte) {
            (None, _) if blank(&byte) => {
                while bytes.next_if(blank).is_some() {}
                words.push(Vec::new());
            }
            (None, b'\'' | b'"') => quoted = Some(byte),
            (Some(quote), _) if byte == quote => quoted = None,
            (_, b'\\') if quoted != Some(b'\'') => word.push(bytes.next()?),
            _ => word.push(byte),
        }
    }
    quoted.is_none().then_some(words)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(words: &[&str]) -> Vec<Vec<u8>> {
        words.iter().map(|word| word.as_bytes().to_vec()).collect()
    }

    #[test]
    fn an_expansion_is_split_as_git_splits_it() {
        for (value, words) in [
            ("a  b ", &["a", "b", ""][..]),
            (" a", &["", "a"]),
            ("a'b c'd", &["ab cd"]),
            ("a\\ b \"c d\" e", &["a b", "c d", "e"]),
            ("\"a\\\"b\" 'a\\b'", &["a\"b", "a\\b"]),
            ("a\tb\nc\rd", &["a", "b", "c", "d"]),
            ("a\x0bb", &["a\x0bb"]),
            ("", &[""]),
            ("   ", &["", ""]),
            ("''", &[""]),
        ] {
            assert_eq!(split(value.as_bytes()), Some(bytes(words)), "{value:?}");
        }
        for refused in ["status 'x", "a \"b", "a\\", "'a", "\"a\\\""] {
            assert_eq!(split(refused.as_bytes()), None, "{refused:?}");
        }
    }

    #[test]
    fn the_options_git_accepts_before_an_aliased_command_lead_it() {
        for (value, options, command, rest) in [
            ("status -s", &[][..], "status", &["-s"][..]),
            ("-c a.b=c status", &["-c", "a.b=c"], "status", &[]),
            (
                "--config-env=a.b=HOME log",
                &["--config-env=a.b=HOME"],
                "log",
                &[],
            ),
            (
                "--config-env a.b=HOME log",
                &["--config-env", "a.b=HOME"],
                "log",
                &[],
            ),
            ("-p log -1", &["-p"], "log", &["-1"]),
            ("--paginate log", &["--paginate"], "log", &[]),
            ("--exec-path=/x zed", &["--exec-path=/x"], "zed", &[]),
            ("-C '' status", &["-C", ""], "status", &[]),
            (
                "-c x.y=1 -p -C \"\" add -A",
                &["-c", "x.y=1", "-p", "-C", ""],
                "add",
                &["-A"],
            ),
            ("-c x.y=1 -c", &[], "", &[]),
        ] {
            let link = Link::of(value.as_bytes());
            if command.is_empty() {
                assert_eq!(link, None, "{value:?}");
                continue;
            }
            assert_eq!(
                link,
                Some(Link {
                    options: bytes(options),
                    command: command.as_bytes().to_vec(),
                    rest: bytes(rest),
                }),
                "{value:?}"
            );
        }
        for value in [
            "!git status",
            "--git-dir=. status",
            "-C . status",
            "--exec-path status",
            "--exec-path",
            "--version",
            "-v",
            "-h",
            "--help",
            "--no-pager log",
            "--literal-pathspecs add",
            "-c x.y=1",
            "-C",
            "status 'x",
        ] {
            assert_eq!(Link::of(value.as_bytes()), None, "{value:?}");
        }
    }
}
