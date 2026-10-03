//! `git dupe --version`.

use crate::harness::{End, under_each_release};

#[test]
fn version_prints_the_package_version_whatever_follows() {
    let line = concat!("git-dupe version ", env!("CARGO_PKG_VERSION"), "\n");
    under_each_release(|s| {
        for words in [&["dupe", "--version"][..], &["dupe", "--version", "status"]] {
            let answer = s.git(words).run();
            assert_eq!(answer.stdout, line.as_bytes(), "{answer:?}");
            assert!(answer.stderr.is_empty(), "{answer:?}");
            assert_eq!(answer.end, End::Code(0), "{answer:?}");
        }
    });
}
