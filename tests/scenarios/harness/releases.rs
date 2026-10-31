//! The Git releases the repository checks, and the narrowing of a focused run.

use std::ffi::OsStr;

/// Every minor release from the oldest git-dupe supports to the newest, raised as new
/// ones appear: the one list of the releases the checks run under.
pub const LISTED: [&str; 14] = [
    "2.43.0", "2.44.0", "2.45.0", "2.46.0", "2.47.0", "2.48.0", "2.49.0", "2.50.0", "2.51.0",
    "2.52.0", "2.53.0", "2.54.0", "2.55.0", "2.56.0",
];

/// The first Git for Windows build of a listed minor release, which stands for it on Windows
/// (F4, S21): its `MinGit` archive, whose digest the repository pins.
pub struct MinGit {
    /// The word of `LISTED` it stands for.
    pub word: &'static str,
    /// The version its tag and asset carry.
    pub build: &'static str,
    /// The SHA-256 of `asset()`, as its release's notes give it.
    pub sha256: &'static str,
}

impl MinGit {
    /// The tag of the release that publishes it.
    pub fn tag(&self) -> String {
        format!("v{}.windows.1", self.build)
    }

    /// The name of its archive among that release's assets.
    pub fn asset(&self) -> String {
        format!("MinGit-{}-64-bit.zip", self.build)
    }
}

/// One row per word of `LISTED`. Git for Windows built no 2.48.0, so 2.48.1 stands for it.
pub static MINGIT: [MinGit; 14] = [
    MinGit {
        word: "2.43.0",
        build: "2.43.0",
        sha256: "1905d93068e986258fafc69517df8fddff829bb2a289c1fa4dcc6cdf720ddf36",
    },
    MinGit {
        word: "2.44.0",
        build: "2.44.0",
        sha256: "ed4e74e171c59c9c9d418743c7109aa595e0cc0d1c80cac574d69ed5e571ae59",
    },
    MinGit {
        word: "2.45.0",
        build: "2.45.0",
        sha256: "f607bbd459bae73369e6509fe849c4c48152f4d33b0021d5881e1e9e7ae79e26",
    },
    MinGit {
        word: "2.46.0",
        build: "2.46.0",
        sha256: "9f3c58e50e265043b1dd7bdaea9db4e2f23b939c189aa0fc820eb8dcfee523be",
    },
    MinGit {
        word: "2.47.0",
        build: "2.47.0",
        sha256: "6b175d4675bfa7014ba649e3c0976da2a281d89bd9a096f1f41437f9995497a8",
    },
    MinGit {
        word: "2.48.0",
        build: "2.48.1",
        sha256: "11e8f462726827acccc7ecdad541f2544cbe5506d70fef4fa1ffac7c16288709",
    },
    MinGit {
        word: "2.49.0",
        build: "2.49.0",
        sha256: "971cdee7c0feaa1e41369c46da88d1000a24e79a6f50191c820100338fb7eca5",
    },
    MinGit {
        word: "2.50.0",
        build: "2.50.0",
        sha256: "69b1a81f881077d3c89806d2cd13cfd0f2b31a4e934e6fb9f8370435aaa7a94e",
    },
    MinGit {
        word: "2.51.0",
        build: "2.51.0",
        sha256: "c2c955a21fa99889d83f485f24fa5d9a38fffc2d509d4022385510e11c26b250",
    },
    MinGit {
        word: "2.52.0",
        build: "2.52.0",
        sha256: "f42a561840627747ad48e6ece05a14093292d31f3393a401a7f7c780ee7695c2",
    },
    MinGit {
        word: "2.53.0",
        build: "2.53.0",
        sha256: "82b562c918ec87b2ef5316ed79bb199e3a25719bb871a0f10294acf21ebd08cd",
    },
    MinGit {
        word: "2.54.0",
        build: "2.54.0",
        sha256: "04f937e1f0918b17b9be6f2294cb2bb66e96e1d9832d1c298e2de088a1d0e668",
    },
    MinGit {
        word: "2.55.0",
        build: "2.55.0",
        sha256: "31497e7968196332263459ee319d2524e3ebc5786ab895e2abad34ffdd4f4ebf",
    },
    MinGit {
        word: "2.56.0",
        build: "2.56.0",
        sha256: "064b440ff870ed5198527e8f3a92cdf5bd2fd0fedf5e718af95e3fdaddeff718",
    },
];

/// The row of `table` for `word`, which `MINGIT` holds for every listed word.
pub fn row<'t>(table: &'t [MinGit], word: &str) -> Option<&'t MinGit> {
    table.iter().find(|row| row.word == word)
}

/// The row of the listed release `word`.
pub fn mingit(word: &str) -> &'static MinGit {
    row(&MINGIT, word).unwrap_or_else(|| panic!("{word} is not a release the repository checks"))
}

/// The oldest release git-dupe supports, for a scenario that must see it named.
pub fn oldest_supported() -> &'static str {
    LISTED[0]
}

/// The harness's one environment variable: a comma-separated subset of the list.
pub const NARROWING_VARIABLE: &str = "GIT_DUPE_CHECK_RELEASES";

/// The releases a run exercises, in the list's order: all of them when the variable is
/// unset, otherwise the ones its value names. A value that is empty or names anything
/// not on the list is an error, so that a mistyped value never narrows a run to nothing.
pub fn selection(narrowing: Option<&OsStr>) -> Result<Vec<&'static str>, String> {
    let Some(value) = narrowing else {
        return Ok(LISTED.to_vec());
    };
    // A value that is not UTF-8 names nothing on the list, in whatever form it is shown.
    let value = value.to_string_lossy();
    let named: Vec<&str> = value.split(',').collect();
    if let Some(unlisted) = named.iter().find(|name| !LISTED.contains(name)) {
        return Err(format!(
            "{NARROWING_VARIABLE}={value:?}: {unlisted:?} is not a release the repository \
             checks; the value is a comma-separated subset of {}",
            LISTED.join(",")
        ));
    }
    Ok(LISTED
        .into_iter()
        .filter(|listed| named.contains(listed))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn narrowed(value: &str) -> Result<Vec<&'static str>, String> {
        selection(Some(OsStr::new(value)))
    }

    /// Every word has exactly one row, whose tag and asset carry the row's build, and the one
    /// word Git for Windows has no build of is the only one whose build is another.
    #[test]
    fn every_listed_word_has_one_mingit_row() {
        assert_eq!(MINGIT.len(), LISTED.len());
        for word in LISTED {
            let rows: Vec<&MinGit> = MINGIT.iter().filter(|row| row.word == word).collect();
            assert_eq!(rows.len(), 1, "{word}");
            let row = rows[0];
            let expected = if word == "2.48.0" { "2.48.1" } else { word };
            assert_eq!(row.build, expected);
            assert_eq!(row.tag(), format!("v{expected}.windows.1"));
            assert_eq!(row.asset(), format!("MinGit-{expected}-64-bit.zip"));
            assert!(
                row.sha256.len() == 64
                    && row
                        .sha256
                        .bytes()
                        .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f')),
                "{word}: {}",
                row.sha256
            );
            assert!(std::ptr::eq(mingit(word), row));
        }
    }

    #[test]
    fn unset_is_the_whole_list() {
        assert_eq!(selection(None).unwrap(), LISTED);
    }

    #[test]
    fn a_subset_is_taken_in_the_lists_order() {
        let (oldest, newest) = (LISTED[0], LISTED[LISTED.len() - 1]);
        assert_eq!(
            narrowed(&format!("{newest},{oldest},{newest}")).unwrap(),
            [oldest, newest]
        );
        assert_eq!(narrowed(oldest).unwrap(), [oldest]);
    }

    #[test]
    fn a_value_that_is_not_a_subset_fails_naming_itself() {
        let oldest = LISTED[0];
        let abbreviated = oldest.strip_suffix(".0").unwrap();
        for value in [
            String::new(),
            "0.0.0".to_string(),
            abbreviated.to_string(),
            format!("{oldest},"),
            format!("{oldest}, {oldest}"),
            format!("{oldest},0.0.0"),
        ] {
            let cause = narrowed(&value).unwrap_err();
            assert!(cause.contains(&format!("{value:?}")), "{cause}");
        }
    }
}
