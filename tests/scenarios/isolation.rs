//! What every scenario starts from: nothing of the machine's configuration, and no
//! repository it could find by accident.

use std::fs;

use crate::harness::{End, under_each_release};

#[test]
fn no_configuration_reaches_a_scenario() {
    under_each_release(|s| {
        let listing = s.git(["config", "--list"]).run();
        assert_eq!(listing.end, End::Code(0), "{listing:?}");
        assert!(listing.stdout.is_empty(), "{listing:?}");
    });
}

#[test]
fn no_repository_is_found_from_a_scenarios_directory_or_below_it() {
    under_each_release(|s| {
        let below = s.dir().join("below/deeper");
        fs::create_dir_all(&below).unwrap();
        for directory in [s.dir(), &below] {
            let found = s.git(["rev-parse", "--git-dir"]).from(directory).run();
            assert_eq!(found.end, End::Code(128), "{found:?}");
        }
    });
}

#[test]
fn a_variable_a_scenario_adds_reaches_git() {
    under_each_release(|s| {
        let read = s
            .git(["config", "--get", "scenario.added"])
            .variable("GIT_CONFIG_COUNT", "1")
            .variable("GIT_CONFIG_KEY_0", "scenario.added")
            .variable("GIT_CONFIG_VALUE_0", "reached")
            .run();
        assert_eq!(read.end, End::Code(0), "{read:?}");
        assert_eq!(read.stdout, b"reached\n", "{read:?}");
    });
}
