//! Files a scenario writes into a workspace, and a workspace copied whole.

use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;

/// Writes `bytes` at `path` below `dir`, making the directories above it first.
pub fn write(dir: &Path, path: &str, bytes: &[u8]) {
    let path = dir.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

/// Copies the directory `from` to `to`, which must not exist yet: every directory, file,
/// and symbolic link below it, with its permissions, so that `Tree` reads the copy as it
/// read the original. Git reads the copied files as changed since its index was written
/// and compares their content, never trusting what it recorded of the originals.
pub fn copy(from: &Path, to: &Path) {
    let failed =
        |path: &Path, cause: std::io::Error| -> ! { panic!("{}: {cause}", path.display()) };
    fs::create_dir(to).unwrap_or_else(|cause| failed(to, cause));
    for entry in fs::read_dir(from).unwrap_or_else(|cause| failed(from, cause)) {
        let entry = entry.unwrap_or_else(|cause| failed(from, cause));
        let (source, target) = (entry.path(), to.join(entry.file_name()));
        let metadata = fs::symlink_metadata(&source).unwrap_or_else(|cause| failed(&source, cause));
        if metadata.is_symlink() {
            let link = fs::read_link(&source).unwrap_or_else(|cause| failed(&source, cause));
            symlink(link, &target).unwrap_or_else(|cause| failed(&target, cause));
        } else if metadata.is_dir() {
            copy(&source, &target);
        } else {
            fs::copy(&source, &target).unwrap_or_else(|cause| failed(&target, cause));
        }
    }
    let permissions = fs::metadata(from).unwrap_or_else(|cause| failed(from, cause));
    fs::set_permissions(to, permissions.permissions()).unwrap_or_else(|cause| failed(to, cause));
}
