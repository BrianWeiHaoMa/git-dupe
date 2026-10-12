//! Aliases a scenario configures: in the private repository, where git-dupe looks first,
//! or in the global configuration under the scenario's own `HOME`.

use std::ffi::OsStr;
use std::path::Path;

use super::scenario::Scenario;

/// Sets `alias.<name>` to `expansion` in the private repository of the workspace at
/// `dir`.
pub fn private_alias(s: &Scenario, dir: &Path, name: &str, expansion: impl AsRef<OsStr>) {
    let key = format!("alias.{name}");
    s.private(dir)
        .git([OsStr::new("config"), OsStr::new(&key), expansion.as_ref()])
        .succeeds();
}

/// Sets `alias.<name>` to `expansion` in the global configuration, the file under the
/// scenario's `HOME`.
pub fn global_alias(s: &Scenario, name: &str, expansion: impl AsRef<OsStr>) {
    let key = format!("alias.{name}");
    s.git([
        OsStr::new("config"),
        OsStr::new("--global"),
        OsStr::new(&key),
        expansion.as_ref(),
    ])
    .succeeds();
}
