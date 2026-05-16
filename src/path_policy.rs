use anyhow::{Result, bail};
use std::path::{Component, Path, PathBuf};

pub fn validate_script_file_operand(path: &str) -> Result<PathBuf> {
    let candidate = Path::new(path);

    if candidate.is_absolute() {
        bail!(
            "unsafe file I/O path '{}': absolute paths are not allowed",
            path
        );
    }

    let mut normalized = PathBuf::new();
    for component in candidate.components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                bail!(
                    "unsafe file I/O path '{}': parent traversal is not allowed",
                    path
                );
            }
            Component::RootDir | Component::Prefix(_) => {
                bail!(
                    "unsafe file I/O path '{}': path prefixes are not allowed",
                    path
                );
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        bail!(
            "unsafe file I/O path '{}': empty paths are not allowed",
            path
        );
    }

    Ok(normalized)
}

pub fn ensure_not_symlink(path: &Path) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            bail!("symlink targets are not allowed: {}", path.display());
        }
        Ok(_) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}
