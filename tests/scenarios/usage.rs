//! What the first word alone decides, before anything runs: bare `git dupe`, and the
//! usage error of a dashed word after `dupe`.

use std::ffi::OsStr;

use crate::harness::{End, general_text, under_each_release, usage_line};

#[test]
fn bare_git_dupe_prints_the_general_usage_line_and_exits_1() {
    under_each_release(|s| {
        let general = general_text(s);
        let bare = s.git(["dupe"]).run();
        assert_eq!(bare.stdout, usage_line(&general), "{bare:?}");
        assert!(bare.stdout.starts_with(b"usage: git dupe"), "{bare:?}");
        assert!(bare.stderr.is_empty(), "{bare:?}");
        assert_eq!(bare.end, End::Code(1), "{bare:?}");
    });
}

#[test]
fn a_dashed_first_word_is_a_usage_error_before_anything_runs() {
    let invocations: [&[&str]; 9] = [
        &["--no-pager", "status"],
        &["-C", "x", "status"],
        &["-c", "color.ui=never", "status"],
        &["--git-dir=x", "status"],
        &["--bogus"],
        &["-"],
        &["-h", "status"],
        &["-h", "-h"],
        // No typed byte becomes a line, or a color, of git-dupe's own.
        &["--bogus\nhint: typed\x1b[31m"],
    ];
    under_each_release(|s| {
        let general = general_text(s);
        for words in invocations {
            // Where no repository is: a locate run before the error would end with 128.
            let dupe = [OsStr::new("dupe")];
            let error = s
                .git(dupe.into_iter().chain(words.iter().map(OsStr::new)))
                .run();
            assert!(error.stdout.is_empty(), "{words:?}: {error:?}");
            let fault = error.line_then("error", usage_line(&general));
            assert!(
                fault
                    .windows(b"git <options> dupe".len())
                    .any(|window| window == b"git <options> dupe"),
                "{words:?}: {error:?}"
            );
            assert!(!fault.contains(&0x1b), "{words:?}: {error:?}");
            assert_eq!(error.end, End::Code(129), "{words:?}: {error:?}");
        }
    });
}
