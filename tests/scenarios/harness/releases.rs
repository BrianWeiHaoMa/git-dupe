//! The Git releases the repository checks, and the narrowing of a focused run.

use std::ffi::OsStr;

/// Every minor release from the oldest git-dupe supports to the newest, raised as new
/// ones appear. This is the only place that names a release: scenarios never do.
pub const LISTED: [&str; 14] = [
    "2.43.0", "2.44.0", "2.45.0", "2.46.0", "2.47.0", "2.48.0", "2.49.0", "2.50.0", "2.51.0",
    "2.52.0", "2.53.0", "2.54.0", "2.55.0", "2.56.0",
];

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
