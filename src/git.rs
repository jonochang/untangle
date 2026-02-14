use crate::errors::{Result, UntangleError};
use git2::Repository;
use std::path::{Path, PathBuf};

/// Read a file's contents at a specific git ref.
pub fn read_file_at_ref(repo: &Repository, reference: &str, path: &Path) -> Result<Vec<u8>> {
    let obj = repo
        .revparse_single(reference)
        .map_err(|_| UntangleError::BadRef {
            reference: reference.to_string(),
        })?;
    let commit = obj.peel_to_commit().map_err(|_| UntangleError::BadRef {
        reference: reference.to_string(),
    })?;
    let tree = commit.tree()?;
    let entry = tree.get_path(path).map_err(|_| UntangleError::BadRef {
        reference: format!("{reference}:{}", path.display()),
    })?;
    let blob = entry
        .to_object(repo)?
        .peel_to_blob()
        .map_err(|_| UntangleError::BadRef {
            reference: format!("{reference}:{}", path.display()),
        })?;
    Ok(blob.content().to_vec())
}

/// List all files at a specific git ref, filtered by extensions.
pub fn list_files_at_ref(
    repo: &Repository,
    reference: &str,
    extensions: &[&str],
) -> Result<Vec<PathBuf>> {
    let obj = repo
        .revparse_single(reference)
        .map_err(|_| UntangleError::BadRef {
            reference: reference.to_string(),
        })?;
    let commit = obj.peel_to_commit().map_err(|_| UntangleError::BadRef {
        reference: reference.to_string(),
    })?;
    let tree = commit.tree()?;

    let mut files = Vec::new();
    tree.walk(git2::TreeWalkMode::PreOrder, |dir, entry| {
        if let Some(name) = entry.name() {
            if extensions
                .iter()
                .any(|ext| name.ends_with(&format!(".{ext}")))
            {
                let path = if dir.is_empty() {
                    PathBuf::from(name)
                } else {
                    PathBuf::from(dir).join(name)
                };
                files.push(path);
            }
        }
        git2::TreeWalkResult::Ok
    })?;

    files.sort();
    Ok(files)
}

/// Open the git repository at the given path (or walk up to find one).
pub fn open_repo(path: &Path) -> Result<Repository> {
    Repository::discover(path).map_err(UntangleError::Git)
}
