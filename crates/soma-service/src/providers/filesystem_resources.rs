//! Recursive discovery of `providers/resources/` files, with the drop-in
//! provider layout's trust-boundary enforcement: no resource file, symlink,
//! or nested path may resolve outside the provider root. Split out of
//! `filesystem.rs` to stay under the module size hard limit — see
//! `docs/contracts/drop-in-provider-layout.md`.

use std::{
    fs,
    path::{Path, PathBuf},
};

use super::FileProviderLoadError;

/// Files under `root/resources/`, recursively, as
/// `(absolute_path, path_relative_to_resources_dir)` pairs. A symlink whose
/// target escapes the canonicalized `resources/` root is a hard error for
/// the whole scan, not a skip — the caller's refresh path
/// (`ProviderRegistry::refresh_file_providers`) already keeps the last
/// valid snapshot active rather than tearing down the live server over it.
pub(super) fn resource_paths(
    root: &Path,
) -> Result<Vec<(PathBuf, PathBuf)>, FileProviderLoadError> {
    let Some(canonical_root) = canonical_resources_root(root)? else {
        return Ok(Vec::new());
    };
    let resources_dir = root.join("resources");
    let mut results = Vec::new();
    walk_resources_dir(&resources_dir, &canonical_root, Path::new(""), &mut results)?;
    results.sort();
    Ok(results)
}

/// The canonicalized `root/resources/` directory, or `None` if it doesn't
/// exist. Also the trust-boundary root every discovered file (at discovery
/// time) and every resource read (at read time, to close the TOCTOU window
/// between the two) is checked against. Verifies `resources/` itself
/// resolves inside the canonicalized provider `root` — a symlink replacing
/// the `resources/` directory entry itself, not just an entry inside it,
/// would otherwise defeat every `starts_with(canonical_root)` check that
/// follows, since they'd all be comparing against the wrong root.
pub(super) fn canonical_resources_root(
    root: &Path,
) -> Result<Option<PathBuf>, FileProviderLoadError> {
    let resources_dir = root.join("resources");
    if !resources_dir.is_dir() {
        return Ok(None);
    }
    let canonical_provider_root = root
        .canonicalize()
        .map_err(|source| FileProviderLoadError {
            path: root.to_path_buf(),
            message: format!("failed to canonicalize provider root: {source}"),
        })?;
    let canonical_root = resources_dir
        .canonicalize()
        .map_err(|source| FileProviderLoadError {
            path: resources_dir.clone(),
            message: format!("failed to canonicalize resources directory: {source}"),
        })?;
    if !canonical_root.starts_with(&canonical_provider_root) {
        return Err(FileProviderLoadError {
            path: resources_dir,
            message: "resources directory escapes the provider root".to_owned(),
        });
    }
    Ok(Some(canonical_root))
}

fn walk_resources_dir(
    dir: &Path,
    canonical_root: &Path,
    relative_prefix: &Path,
    results: &mut Vec<(PathBuf, PathBuf)>,
) -> Result<(), FileProviderLoadError> {
    let entries = fs::read_dir(dir).map_err(|source| FileProviderLoadError {
        path: dir.to_path_buf(),
        message: format!("failed to read resources directory: {source}"),
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| FileProviderLoadError {
            path: dir.to_path_buf(),
            message: format!("failed to read resources directory entry: {source}"),
        })?;
        let path = entry.path();
        let relative = relative_prefix.join(entry.file_name());
        let file_type = entry.file_type().map_err(|source| FileProviderLoadError {
            path: path.clone(),
            message: format!("failed to stat resources directory entry: {source}"),
        })?;

        if file_type.is_symlink() {
            // The only entry kind that can point outside the physically
            // nested directory tree — canonicalize and verify before trusting
            // it at all. Never recurse into a symlinked directory (avoids
            // symlink cycles); a symlinked file is accepted as a leaf only
            // if its resolved target is still inside the root.
            let canonical = path
                .canonicalize()
                .map_err(|source| FileProviderLoadError {
                    path: path.clone(),
                    message: format!("failed to resolve resource symlink: {source}"),
                })?;
            if !canonical.starts_with(canonical_root) {
                return Err(FileProviderLoadError {
                    path: path.clone(),
                    message: "resource path escapes the provider root via a symlink".to_owned(),
                });
            }
            if canonical.is_file() {
                results.push((path, relative));
            }
            continue;
        }

        if file_type.is_dir() {
            walk_resources_dir(&path, canonical_root, &relative, results)?;
            continue;
        }

        if file_type.is_file() {
            results.push((path, relative));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "filesystem_resources_tests.rs"]
mod tests;
