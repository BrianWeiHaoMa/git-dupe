//! The managed region as settle leaves it in `.git/info/exclude`.

use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::harness::{
    End, Output, Scenario, Tree, names, names_number, refused_stash_untracked, region,
    region_rules, stash_refusal_first, stash_untracked_line, under_each_release,
};

const ONLY_GITDUPE: &[u8] = b"# BEGIN git-dupe\n/.gitdupe\n# END git-dupe\n";

/// A workspace attached by the fixture `attached_repository`, and the directory to run
/// from: its root, or `sub` below it.
fn fixture_workspace(s: &Scenario, below: bool) -> (PathBuf, PathBuf) {
    let dir = s.dir().join(if below { "below" } else { "root" });
    s.attached_repository(&dir);
    let from = if below { dir.join("sub") } else { dir.clone() };
    fs::create_dir_all(&from).unwrap();
    (dir, from)
}

/// `git dupe stash -u` from `from`, refused by its untracked guard, so that settle
/// alone acts.
fn settled_by_stash_refusal(s: &Scenario, from: &Path) -> Output {
    let output = s.git(["dupe", "stash", "-u"]).from(from).run();
    refused_stash_untracked(s, &output);
    output
}

#[test]
fn user_bytes_without_a_final_newline_survive_and_the_second_run_is_identical() {
    under_each_release(|s| {
        for below in [false, true] {
            let (dir, from) = fixture_workspace(s, below);
            let exclude = dir.join(".git/info/exclude");
            let user = b"# user\r\n*.o\n# caf\xe9 without final newline";
            fs::write(&exclude, user).unwrap();
            let expected = [user.as_slice(), b"\n", ONLY_GITDUPE].concat();
            for _ in 0..2 {
                let output = settled_by_stash_refusal(s, &from);
                assert!(output.lines("warning").is_empty(), "{output:?}");
                assert_eq!(fs::read(&exclude).unwrap(), expected);
                let found = region(&dir).unwrap();
                assert_eq!(found.before, [user.as_slice(), b"\n"].concat());
                assert_eq!(found.rules, [b"/.gitdupe"]);
                assert!(found.after.is_empty());
            }
        }
    });
}

#[test]
fn handwritten_lines_are_cleaned_and_descendants_have_no_second_rule() {
    under_each_release(|s| {
        for below in [false, true] {
            let (dir, from) = fixture_workspace(s, below);
            fs::write(
                dir.join(".gitdupe"),
                b"notes/\n/docs/plan.md\n\na/./b//\n../x\n.\nnotes/sub\n",
            )
            .unwrap();
            for _ in 0..2 {
                let output = settled_by_stash_refusal(s, &from);
                assert_eq!(
                    region_rules(&dir),
                    [&b"/.gitdupe"[..], b"/a/b", b"/docs/plan.md", b"/notes"]
                );
                let warnings = output.lines("warning");
                assert_eq!(warnings.len(), 2, "{output:?}");
                for (line, number) in warnings.iter().zip([5, 6]) {
                    names(line, b".gitdupe");
                    names_number(line, number);
                }
            }
            // Another command settle ends names them as well, until they are removed.
            let status = s.git(["dupe", "status"]).from(&from).run();
            assert_eq!(status.end, End::Code(0), "{status:?}");
            let warnings = status.lines("warning");
            assert_eq!(warnings.len(), 2, "{status:?}");
            for (line, number) in warnings.iter().zip([5, 6]) {
                names(line, b".gitdupe");
                names_number(line, number);
            }
            fs::write(
                dir.join(".gitdupe"),
                b"notes/\n/docs/plan.md\n\na/./b//\nnotes/sub\n",
            )
            .unwrap();
            let output = settled_by_stash_refusal(s, &from);
            assert!(output.lines("warning").is_empty(), "{output:?}");
            assert_eq!(
                region_rules(&dir),
                [&b"/.gitdupe"[..], b"/a/b", b"/docs/plan.md", b"/notes"]
            );
        }
    });
}

#[test]
fn info_links_and_dangling_exclude_links_stay_untouched_and_exposure_is_still_asked() {
    under_each_release(|s| {
        for below in [false, true] {
            for info_link in [true, false] {
                let dir = s.dir().join(format!("links-{below}-{info_link}"));
                s.attached_repository(&dir);
                let from = if below { dir.join("sub") } else { dir.clone() };
                fs::create_dir_all(&from).unwrap();
                fs::write(dir.join(".gitdupe"), b"notes\n").unwrap();
                fs::write(dir.join("notes"), b"private\n").unwrap();
                fs::write(dir.join(".gitignore"), b"/.gitdupe\n!notes\n").unwrap();
                let target = dir.join("target");
                let link = if info_link {
                    fs::rename(dir.join(".git/info"), &target).unwrap();
                    fs::write(target.join("exclude"), b"# target\n").unwrap();
                    dir.join(".git/info")
                } else {
                    fs::remove_file(dir.join(".git/info/exclude")).unwrap();
                    dir.join(".git/info/exclude")
                };
                symlink(&target, &link).unwrap();
                let before = Tree::of(&dir);
                let output = settled_by_stash_refusal(s, &from);
                let warnings = output.lines("warning");
                assert_eq!(warnings.len(), 2, "{output:?}");
                names(
                    warnings[0],
                    if info_link {
                        b".git/info"
                    } else {
                        b".git/info/exclude"
                    },
                );
                names(warnings[0], b"cannot be maintained");
                names(warnings[1], b"notes");
                names(warnings[1], b".gitignore");
                names_number(warnings[1], 2);
                assert_eq!(fs::read_link(&link).unwrap(), target);
                assert!(before.changed_in(&Tree::of(&dir)).is_empty());
                assert!(region(&dir).is_none());
            }
        }
    });
}

#[test]
fn a_resolving_exclude_link_is_replaced_and_its_target_is_untouched() {
    under_each_release(|s| {
        for below in [false, true] {
            let (dir, from) = fixture_workspace(s, below);
            let target = dir.join("target");
            let exclude = dir.join(".git/info/exclude");
            fs::write(&target, b"# target without newline").unwrap();
            fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
            fs::remove_file(&exclude).unwrap();
            symlink(&target, &exclude).unwrap();
            let before = Tree::of(&dir);
            let output = settled_by_stash_refusal(s, &from);
            let warnings = output.lines("warning");
            assert_eq!(warnings.len(), 1, "{output:?}");
            names(warnings[0], b".git/info/exclude");
            assert!(fs::symlink_metadata(&exclude).unwrap().is_file());
            assert_eq!(fs::read(&target).unwrap(), b"# target without newline");
            assert_eq!(
                fs::read(&exclude).unwrap(),
                [b"# target without newline\n".as_slice(), ONLY_GITDUPE].concat()
            );
            assert_eq!(region_rules(&dir), [b"/.gitdupe"]);
            assert_eq!(
                fs::metadata(&exclude).unwrap().permissions().mode() & 0o7777,
                0o600
            );
            assert!(
                before
                    .without(&[&exclude])
                    .changed_in(&Tree::of(&dir).without(&[&exclude]))
                    .is_empty()
            );
        }
    });
}

#[test]
fn absent_info_is_created_with_only_the_region() {
    under_each_release(|s| {
        for below in [false, true] {
            let (dir, from) = fixture_workspace(s, below);
            fs::remove_dir_all(dir.join(".git/info")).unwrap();
            let output = settled_by_stash_refusal(s, &from);
            assert!(output.lines("warning").is_empty(), "{output:?}");
            assert_eq!(
                fs::read(dir.join(".git/info/exclude")).unwrap(),
                ONLY_GITDUPE
            );
            assert_eq!(region_rules(&dir), [b"/.gitdupe"]);
        }
    });
}

#[test]
fn replacement_keeps_permissions_and_a_new_exclude_gets_a_new_files_mode() {
    under_each_release(|s| {
        for below in [false, true] {
            let (dir, from) = fixture_workspace(s, below);
            let exclude = dir.join(".git/info/exclude");
            fs::write(&exclude, b"# user\n").unwrap();
            fs::set_permissions(&exclude, fs::Permissions::from_mode(0o600)).unwrap();
            let output = settled_by_stash_refusal(s, &from);
            assert!(output.lines("warning").is_empty(), "{output:?}");
            assert_eq!(
                fs::read(&exclude).unwrap(),
                [b"# user\n".as_slice(), ONLY_GITDUPE].concat()
            );
            assert_eq!(
                fs::metadata(&exclude).unwrap().permissions().mode() & 0o7777,
                0o600
            );
            fs::remove_file(&exclude).unwrap();
            let reference = dir.join("new-file-mode");
            fs::write(&reference, b"").unwrap();
            let output = settled_by_stash_refusal(s, &from);
            assert!(output.lines("warning").is_empty(), "{output:?}");
            assert_eq!(fs::read(&exclude).unwrap(), ONLY_GITDUPE);
            assert_eq!(region_rules(&dir), [b"/.gitdupe"]);
            assert_eq!(
                fs::metadata(&exclude).unwrap().permissions().mode(),
                fs::metadata(reference).unwrap().permissions().mode()
            );
        }
    });
}

#[test]
fn an_exclude_directory_warns_and_changes_nothing_under_git() {
    under_each_release(|s| {
        for below in [false, true] {
            let (dir, from) = fixture_workspace(s, below);
            let exclude = dir.join(".git/info/exclude");
            fs::remove_file(&exclude).unwrap();
            fs::create_dir(&exclude).unwrap();
            fs::write(exclude.join("kept"), b"untouched\n").unwrap();
            // Give the hidden path a public rule independently of the failed region.
            fs::write(dir.join(".gitignore"), b"/.gitdupe\n").unwrap();
            let before = Tree::of(&dir.join(".git"));
            let private_before = Tree::of(&dir.join(".git/dupe"));
            let output = s.git(["dupe", "stash", "-u"]).from(&from).run();
            stash_refusal_first(s, &output);
            assert_eq!(output.end, End::Code(128), "{output:?}");
            assert!(output.stdout.is_empty(), "{output:?}");
            assert_eq!(output.lines("fatal")[0], stash_untracked_line(s));
            // Two warnings: the replacement that failed, naming the file, and the
            // exposure question, which Git cannot answer while its exclude file is a
            // directory and which is then one warning of its own.
            let warnings = output.lines("warning");
            assert_eq!(warnings.len(), 2, "{output:?}");
            names(warnings[0], b".git/info/exclude");
            assert!(before.changed_in(&Tree::of(&dir.join(".git"))).is_empty());
            assert!(
                private_before
                    .changed_in(&Tree::of(&dir.join(".git/dupe")))
                    .is_empty()
            );
        }
    });
}

#[test]
fn every_literal_rule_ignores_its_path_and_leaves_its_sibling_visible() {
    under_each_release(|s| {
        let paths: [(&[u8], &[u8], &[u8]); 10] = [
            (b"has space", b"has_space", b"/has\\ space"),
            (b"trail ", b"trail", b"/trail\\ "),
            (b"#hash", b"hash", b"/\\#hash"),
            (b"!bang", b"bang", b"/\\!bang"),
            (b"star*", b"starx", b"/star\\*"),
            (b"q?x", b"qax", b"/q\\?x"),
            (b"br[1]", b"br1", b"/br\\[1]"),
            (b"back\\slash", b"backslash", b"/back\\\\slash"),
            (b"cr\rx", b"crx", b"/cr[\r]x"),
            (b"caf\xe9", b"cafe", b"/caf\xe9"),
        ];
        for below in [false, true] {
            let (dir, from) = fixture_workspace(s, below);
            let mut listed = Vec::new();
            for (path, sibling, _) in paths {
                listed.extend_from_slice(path);
                listed.push(b'\n');
                fs::write(dir.join(OsStr::from_bytes(path)), b"private\n").unwrap();
                fs::write(dir.join(OsStr::from_bytes(sibling)), b"public\n").unwrap();
            }
            fs::write(dir.join(".gitdupe"), listed).unwrap();
            // Byte order is path order; escaping must not change that ordering.
            let mut ordered: Vec<(&[u8], &[u8])> = paths
                .iter()
                .map(|(path, _, rule)| (*path, *rule))
                .chain([(b".gitdupe".as_slice(), b"/.gitdupe".as_slice())])
                .collect();
            ordered.sort_by_key(|(path, _)| *path);
            let expected: Vec<Vec<u8>> =
                ordered.into_iter().map(|(_, rule)| rule.to_vec()).collect();
            let output = settled_by_stash_refusal(s, &from);
            assert!(output.lines("warning").is_empty(), "{output:?}");
            assert_eq!(region_rules(&dir), expected);
            let status = s
                .git([
                    "status",
                    "--porcelain",
                    "-z",
                    "--ignored",
                    "--untracked-files=all",
                ])
                .from(&dir)
                .run();
            assert_eq!(status.end, End::Code(0), "{status:?}");
            let records: Vec<&[u8]> = status.stdout.split(|&byte| byte == 0).collect();
            for (path, sibling, _) in paths {
                for (prefix, name) in [(b"!! ".as_slice(), path), (b"?? ".as_slice(), sibling)] {
                    let expected = [prefix, name].concat();
                    assert!(
                        records.contains(&expected.as_slice()),
                        "missing {}: {status:?}",
                        expected.escape_ascii()
                    );
                }
            }
        }
    });
}

#[test]
fn an_argument_list_over_arg_max_is_refused_with_its_count_before_any_write() {
    under_each_release(|s| {
        let limit = Command::new("getconf").arg("ARG_MAX").output().unwrap();
        assert!(limit.status.success());
        let limit: usize = std::str::from_utf8(&limit.stdout)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        // Each distinct path has at least 128 bytes, so the path bytes alone exceed ARG_MAX.
        let count = limit / 128 + 1;
        let mut listed = Vec::new();
        for index in 0..count {
            listed.extend_from_slice(format!("path-{index:016}-{}\n", "x".repeat(128)).as_bytes());
        }
        for below in [false, true] {
            let (dir, from) = fixture_workspace(s, below);
            fs::write(dir.join(".gitdupe"), &listed).unwrap();
            let exclude = dir.join(".git/info/exclude");
            let before = fs::read(&exclude).unwrap();
            let output = s.git(["dupe", "stash", "-u"]).from(&from).run();
            assert_eq!(output.end, End::Code(128), "{output:?}");
            assert!(output.stdout.is_empty(), "{output:?}");
            let fatal = output.lines("fatal");
            assert_eq!(fatal.len(), 2, "{output:?}");
            assert_eq!(fatal[0], stash_untracked_line(s));
            assert!(
                output.stderr.starts_with(
                    &[b"fatal: ".as_slice(), &stash_untracked_line(s), b"\n"].concat()
                ),
                "{output:?}"
            );
            names_number(fatal[1], count + 1);
            assert!(output.lines("warning").is_empty(), "{output:?}");
            assert_eq!(fs::read(&exclude).unwrap(), before);
            assert!(region(&dir).is_none());
        }
    });
}
