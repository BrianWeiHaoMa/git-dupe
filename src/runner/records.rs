//! Readers of the machine forms Git answers in (R5): a `-z` listing of paths, a
//! `config -z` answer, as one `--get` value or as the records of `--list` and
//! `--get-regexp`, the records of `check-ignore -v -n -z --stdin`, the name lines of
//! `--list-cmds`, the records of `worktree list --porcelain -z`, the line of
//! `symbolic-ref -q HEAD`, and the `ref:` line of `ls-remote --symref`.
//!
//! A reader parses its form and nothing more. Which listing or key is asked for, and what
//! an answer means, stay with the part that asks. A form read by one caller whose checks
//! of it are that caller's own may stay with it.

use std::collections::BTreeSet;

/// The paths of a `-z` listing: NUL-terminated, each once.
pub fn paths(listing: &[u8]) -> BTreeSet<Vec<u8>> {
    listing
        .split(|&byte| byte == 0)
        .filter(|path| !path.is_empty())
        .map(<[u8]>::to_vec)
        .collect()
}

/// The value in a `config -z --get` answer: its bytes before the terminating NUL.
pub fn config_value(answer: &[u8]) -> &[u8] {
    answer.strip_suffix(b"\0").unwrap_or(answer)
}

/// The records of `config -z --list` or `--get-regexp`: each the key, a newline, and the
/// value, NUL terminated, a newline inside the value kept; a key without a value is the
/// key alone.
pub fn config_records(listing: &[u8]) -> impl Iterator<Item = (&[u8], Option<&[u8]>)> {
    listing
        .split(|&byte| byte == 0)
        .filter(|record| !record.is_empty())
        .map(
            |record| match record.iter().position(|&byte| byte == b'\n') {
                Some(at) => (&record[..at], Some(&record[at + 1..])),
                None => (record, None),
            },
        )
}

/// The names of `git --list-cmds=…`: one command per line.
pub fn command_names(listing: &[u8]) -> impl Iterator<Item = &[u8]> {
    listing
        .split(|&byte| byte == b'\n')
        .filter(|name| !name.is_empty())
}

/// The working tree roots of `worktree list --porcelain -z`, in Git's order, the main
/// working tree first: each record is NUL-terminated fields, `worktree <path>` first, and
/// ends with an empty field (S12). `None` when the listing is not of that shape or names
/// no working tree, which is never taken for an answer.
pub fn worktree_roots(listing: &[u8]) -> Option<Vec<&[u8]>> {
    let mut roots = Vec::new();
    let mut starting = true;
    for field in listing.strip_suffix(b"\0")?.split(|&byte| byte == 0) {
        if starting {
            roots.push(field.strip_prefix(b"worktree ")?);
            starting = false;
        } else if field.is_empty() {
            starting = true;
        }
    }
    (starting && !roots.is_empty()).then_some(roots)
}

/// The branch in a `symbolic-ref -q HEAD` answer: `refs/heads/<name>` on its line, on a
/// branch and an unborn one alike (S12). A detached `HEAD` answers nothing, and a
/// reference outside `refs/heads/` names no branch.
pub fn branch_of(answer: &[u8]) -> Option<&[u8]> {
    let line = answer.strip_suffix(b"\n").unwrap_or(answer);
    line.strip_prefix(b"refs/heads/")
        .filter(|branch| !branch.is_empty())
}

/// The branch a remote's `HEAD` names in an `ls-remote --symref <remote> HEAD` answer:
/// the name in its line `ref: refs/heads/<name>`, a tab, `HEAD`, which Git prints only
/// when the remote's `HEAD` is symbolic and names a branch the remote has (S12). A hash
/// line names none; so do a `ref:` line for another reference that ends in `HEAD`, as a
/// remote's own `refs/remotes/origin/HEAD`, and a target outside `refs/heads/`. No
/// reference name holds a tab or a newline.
pub fn default_branch(answer: &[u8]) -> Option<&[u8]> {
    answer.split(|&byte| byte == b'\n').find_map(|line| {
        let record = line.strip_prefix(b"ref: ")?;
        let tab = record.iter().position(|&byte| byte == b'\t')?;
        if &record[tab + 1..] != b"HEAD" {
            return None;
        }
        record[..tab]
            .strip_prefix(b"refs/heads/")
            .filter(|branch| !branch.is_empty())
    })
}

/// The standard input of `check-ignore -z --stdin` asking about `paths`: each written
/// `./<path>`, so that a leading `:` is not read as magic (S5), and NUL terminated.
pub fn ignore_question(paths: &[&[u8]]) -> Vec<u8> {
    let mut input = Vec::new();
    for path in paths {
        input.extend_from_slice(b"./");
        input.extend_from_slice(path);
        input.push(0);
    }
    input
}

/// One record of `check-ignore -v -n -z`: the source, line, and pattern of the deciding
/// rule, each empty where no rule matches.
#[derive(Debug, PartialEq, Eq)]
pub struct IgnoreRecord<'a> {
    pub source: &'a [u8],
    pub line: &'a [u8],
    pub pattern: &'a [u8],
}

impl IgnoreRecord<'_> {
    /// Whether the record reports its path ignored: a pattern that is empty or begins
    /// with `!` marks the path not ignored (S5).
    pub fn ignored(&self) -> bool {
        !self.pattern.is_empty() && !self.pattern.starts_with(b"!")
    }
}

/// The records answering `ignore_question(paths)`, one per path in order, or `None` when
/// they do not answer exactly those paths: a record per path, four NUL-terminated fields
/// each, the last echoing the path as fed. A partial output is no answer.
pub fn ignore_records<'a>(output: &'a [u8], paths: &[&[u8]]) -> Option<Vec<IgnoreRecord<'a>>> {
    let fields: Vec<&[u8]> = output
        .strip_suffix(b"\0")?
        .split(|&byte| byte == 0)
        .collect();
    if fields.len() != paths.len() * 4 {
        return None;
    }
    fields
        .chunks(4)
        .zip(paths)
        .map(|(record, path)| {
            let [source, line, pattern, fed] = record else {
                return None;
            };
            (fed.strip_prefix(b"./") == Some(*path)).then_some(IgnoreRecord {
                source,
                line,
                pattern,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_listing_is_its_nul_terminated_paths_each_once() {
        let found = paths(b"a b\0notes/x\0cr\r\0notes/x\0caf\xe9\0");
        let expected: BTreeSet<Vec<u8>> = [&b"a b"[..], b"notes/x", b"cr\r", b"caf\xe9"]
            .map(<[u8]>::to_vec)
            .into();
        assert_eq!(found, expected);
        assert!(paths(b"").is_empty());
    }

    #[test]
    fn a_record_is_a_key_then_its_value_and_a_newline_in_the_value_is_kept() {
        let listing = b"core.bare\nfalse\0probe.value\nfirst\nsecond\0probe.novalue\0user.name\n\0";
        let read: Vec<_> = config_records(listing).collect();
        assert_eq!(
            read,
            [
                (&b"core.bare"[..], Some(&b"false"[..])),
                (b"probe.value", Some(b"first\nsecond")),
                (b"probe.novalue", None),
                (b"user.name", Some(b"")),
            ]
        );
        assert_eq!(config_records(b"").count(), 0);
    }

    #[test]
    fn a_question_writes_each_path_from_the_root_and_its_records_answer_it_in_order() {
        let paths: [&[u8]; 3] = [b"notes", b":conf", b"cr\rx"];
        assert_eq!(ignore_question(&paths), b"./notes\0./:conf\0./cr\rx\0");
        let output = b".git/info/exclude\x001\x00/notes\x00./notes\x00\
                       .gitignore\x003\x00!:conf\x00./:conf\x00\
                       \x00\x00\x00./cr\rx\x00";
        let records = ignore_records(output, &paths).expect("records");
        assert_eq!(
            records[1],
            IgnoreRecord {
                source: b".gitignore",
                line: b"3",
                pattern: b"!:conf",
            }
        );
        let ignored: Vec<bool> = records.iter().map(IgnoreRecord::ignored).collect();
        assert_eq!(ignored, [true, false, false]);
    }

    #[test]
    fn records_that_do_not_answer_the_paths_asked_are_no_answer() {
        let one = &b"\x00\x00\x00./a\x00"[..];
        assert!(ignore_records(one, &[b"a"]).is_some());
        // Too few, too many, another path, the path not fed from the root, and a record
        // cut short.
        assert!(ignore_records(one, &[b"a", b"b"]).is_none());
        assert!(ignore_records(one, &[]).is_none());
        assert!(ignore_records(one, &[b"b"]).is_none());
        assert!(ignore_records(b"\x00\x00\x00a\x00", &[b"a"]).is_none());
        assert!(ignore_records(b"\x00\x00\x00./a", &[b"a"]).is_none());
        assert!(ignore_records(b"", &[b"a"]).is_none());
    }

    #[test]
    fn a_command_list_is_one_name_per_line() {
        let names: Vec<&[u8]> = command_names(b"add\nam\npack-redundant\nzed\n").collect();
        assert_eq!(names, [&b"add"[..], b"am", b"pack-redundant", b"zed"]);
        assert_eq!(command_names(b"").count(), 0);
        assert_eq!(command_names(b"log").collect::<Vec<_>>(), [b"log"]);
    }

    #[test]
    fn a_worktree_listing_is_its_roots_one_per_record() {
        let listing = b"worktree /w/repo\0HEAD 12ab\0branch refs/heads/main\0\0\
                        worktree /w/li nked\0HEAD 12ab\0detached\0\0\
                        worktree /w/caf\xe9\0bare\0\0\
                        worktree /w/gone\0HEAD 12ab\0detached\0locked why\nnot\0prunable x\0\0";
        assert_eq!(
            worktree_roots(listing),
            Some(vec![
                &b"/w/repo"[..],
                b"/w/li nked",
                b"/w/caf\xe9",
                b"/w/gone"
            ])
        );
        assert_eq!(
            worktree_roots(b"worktree /w/repo\0\0"),
            Some(vec![&b"/w/repo"[..]])
        );
    }

    #[test]
    fn a_listing_of_another_shape_is_no_answer() {
        for listing in [
            &b""[..],
            b"\0",
            b"worktree /w/repo",
            b"worktree /w/repo\0HEAD 12ab\0",
            b"worktree /w/repo\0\0HEAD 12ab\0\0",
            b"worktree /w/repo\0\0\0",
            b"/w/repo\0\0",
        ] {
            assert_eq!(worktree_roots(listing), None, "{}", listing.escape_ascii());
        }
    }

    #[test]
    fn the_branch_is_the_name_below_refs_heads() {
        assert_eq!(branch_of(b"refs/heads/main\n"), Some(&b"main"[..]));
        assert_eq!(
            branch_of(b"refs/heads/feature/x\n"),
            Some(&b"feature/x"[..])
        );
        assert_eq!(branch_of(b"refs/heads/caf\xe9\n"), Some(&b"caf\xe9"[..]));
        assert_eq!(branch_of(b""), None);
        assert_eq!(branch_of(b"refs/heads/\n"), None);
        assert_eq!(branch_of(b"refs/remotes/origin/main\n"), None);
    }

    #[test]
    fn the_default_branch_is_the_branch_of_the_ref_line_for_head_alone() {
        let hash = "0123456789abcdef0123456789abcdef01234567";
        let symbolic = format!("ref: refs/heads/main\tHEAD\n{hash}\tHEAD\n");
        assert_eq!(default_branch(symbolic.as_bytes()), Some(&b"main"[..]));
        assert_eq!(
            default_branch(b"ref: refs/heads/feature/caf\xe9\tHEAD\n"),
            Some(&b"feature/caf\xe9"[..])
        );
        // A `HEAD` holding a hash, and the empty remote or one whose `HEAD` names a branch
        // it lacks, which print no `ref:` line.
        assert_eq!(default_branch(format!("{hash}\tHEAD\n").as_bytes()), None);
        assert_eq!(default_branch(b""), None);
        // A remote's own remote-tracking `HEAD` beside its `HEAD`, before it and alone.
        let beside = format!(
            "ref: refs/remotes/origin/other\trefs/remotes/origin/HEAD\n\
             {hash}\trefs/remotes/origin/HEAD\nref: refs/heads/main\tHEAD\n{hash}\tHEAD\n"
        );
        assert_eq!(default_branch(beside.as_bytes()), Some(&b"main"[..]));
        let alone = format!(
            "ref: refs/remotes/origin/other\trefs/remotes/origin/HEAD\n\
             {hash}\trefs/remotes/origin/HEAD\n{hash}\tHEAD\n"
        );
        assert_eq!(default_branch(alone.as_bytes()), None);
        // A target outside `refs/heads/`, and none at all.
        assert_eq!(default_branch(b"ref: refs/tags/v1\tHEAD\n"), None);
        assert_eq!(default_branch(b"ref: refs/heads/\tHEAD\n"), None);
        assert_eq!(default_branch(b"ref: refs/heads/main HEAD\n"), None);
    }

    #[test]
    fn a_get_answer_is_the_value_before_its_nul() {
        assert_eq!(config_value(b"A Name\0"), b"A Name");
        assert_eq!(config_value(b"two\nlines\0"), b"two\nlines");
        assert_eq!(config_value(b"\0"), b"");
    }
}
