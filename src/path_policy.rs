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
    ensure_no_symlink_components(path)
}

pub fn validate_restore_target(path: &Path) -> Result<PathBuf> {
    if path.as_os_str().is_empty() {
        bail!("backup metadata path validation failed: empty original path");
    }

    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        bail!(
            "backup metadata path validation failed for '{}': parent traversal is not allowed",
            path.display()
        );
    }

    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component);

        match std::fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                bail!(
                    "backup metadata path validation failed for '{}': symlink targets are not allowed",
                    current.display()
                );
            }
            Ok(_) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err.into()),
        }
    }

    Ok(path.to_path_buf())
}

pub fn ensure_no_symlink_components(path: &Path) -> Result<()> {
    let mut current = PathBuf::new();

    for component in path.components() {
        current.push(component);

        match std::fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                bail!("symlink targets are not allowed: {}", current.display());
            }
            Ok(_) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err.into()),
        }
    }

    Ok(())
}
