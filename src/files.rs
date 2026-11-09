//! All project writes use validated relative paths and atomic replacement.
use anyhow::{Context, Result, bail, ensure};
use std::{
    fs,
    io::Write,
    path::{Component, Path, PathBuf},
};

pub fn project_root(requested: Option<&Path>) -> Result<PathBuf> {
    let raw = requested
        .map(Path::to_path_buf)
        .unwrap_or(std::env::current_dir()?);
    let root = raw.canonicalize().context("project root must exist")?;
    ensure!(
        root.is_dir() && root.parent().is_some(),
        "choose a project directory, not a filesystem root"
    );
    if let Ok(home) = std::env::var("HOME") {
        ensure!(
            root != Path::new(&home),
            "choose a project directory, not the entire home directory"
        );
    }
    Ok(root)
}

pub fn relative(path: &str) -> Result<()> {
    ensure!(
        !path.is_empty()
            && !path.contains('\\')
            && !path.contains('\0')
            && path
                .split('/')
                .all(|c| !c.is_empty() && c != "." && c != ".."),
        "invalid project-relative path"
    );
    ensure!(
        Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_))),
        "path must be relative without dot or parent components"
    );
    Ok(())
}

pub fn inside(root: &Path, path: &str) -> Result<PathBuf> {
    relative(path)?;
    let mut current = root.to_path_buf();
    for component in Path::new(path).components() {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(meta) => ensure!(
                !meta.file_type().is_symlink(),
                "symlink path is not an Astra Watch write target: {}",
                current.display()
            ),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(current)
}

fn prepared(root: &Path, path: &str) -> Result<PathBuf> {
    let target = inside(root, path)?;
    fs::create_dir_all(target.parent().context("missing parent")?)?;
    // Recheck after creating parents. This is a local ownership check, not an OS sandbox.
    inside(root, path)
}

pub fn create(root: &Path, path: &str, data: &[u8]) -> Result<bool> {
    let target = prepared(root, path)?;
    let mut temp = tempfile::NamedTempFile::new_in(target.parent().unwrap())?;
    temp.write_all(data)?;
    temp.as_file().sync_all()?;
    match temp.persist_noclobber(&target) {
        Ok(_) => Ok(true),
        Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
        Err(e) => Err(e.error.into()),
    }
}

pub fn replace(root: &Path, path: &str, data: &[u8]) -> Result<()> {
    let target = prepared(root, path)?;
    let mut temp = tempfile::NamedTempFile::new_in(target.parent().unwrap())?;
    temp.write_all(data)?;
    temp.as_file().sync_all()?;
    temp.persist(target).map_err(|e| e.error)?;
    Ok(())
}

pub fn immutable(root: &Path, path: &str, data: &[u8]) -> Result<bool> {
    if create(root, path, data)? {
        return Ok(true);
    }
    if fs::read(inside(root, path)?)? != data {
        bail!("immutable artifact differs from the existing bytes: {path}");
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn traversal_and_absolute_paths_are_rejected() {
        for path in ["../secret", "/secret", "foo/../secret", "", "foo\\bar"] {
            assert!(relative(path).is_err(), "{path}");
        }
    }
    #[test]
    fn immutable_creation_is_idempotent_not_overwritable() {
        let dir = tempfile::tempdir().unwrap();
        assert!(immutable(dir.path(), "records/a", b"a").unwrap());
        assert!(!immutable(dir.path(), "records/a", b"a").unwrap());
        assert!(immutable(dir.path(), "records/a", b"b").is_err());
        assert_eq!(fs::read(dir.path().join("records/a")).unwrap(), b"a");
    }
    #[cfg(unix)]
    #[test]
    fn symlinked_parent_cannot_escape_project() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join("records")).unwrap();
        assert!(create(root.path(), "records/escape", b"no").is_err());
        assert!(!outside.path().join("escape").exists());
    }
}
