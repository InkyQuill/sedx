//! Error helper functions for creating actionable error messages

use std::io;
use std::path::Path;

/// Check if an IO error is a permission denied error
pub fn is_permission_denied(err: &io::Error) -> bool {
    err.kind() == io::ErrorKind::PermissionDenied
}

/// Check if an IO error is a "not found" error
pub fn is_not_found(err: &io::Error) -> bool {
    err.kind() == io::ErrorKind::NotFound
}

/// Create an enhanced error message for file permission issues
pub fn permission_error(path: &Path, operation: &str) -> String {
    let parent_dir = path
        .parent()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| ".".to_string());

    format!(
        "Permission denied when {} '{}'\n\n\
         Possible fixes:\n\
         1. Check file permissions: ls -l '{}'\n\
         2. If directory: Ensure write access with: chmod u+w '{}'\n\
         3. If owned by another user: Try with sudo (not recommended)\n\
         4. For backup operations: Use --no-backup to skip backup creation",
        operation,
        path.display(),
        path.display(),
        parent_dir
    )
}

/// Create an enhanced error message for file not found issues
pub fn not_found_error(path: &Path, context: &str) -> String {
    format!(
        "File not found: '{}'\n\n\
         Context: {}\n\n\
         Possible fixes:\n\
         1. Check the file path is correct\n\
         2. Use an absolute path if the relative path is ambiguous\n\
         3. Create the file first if it doesn't exist: touch '{}'\n\
         4. Check if the file exists in a different directory",
        path.display(),
        context,
        path.display()
    )
}

pub fn file_error(path: &Path, operation: &str, err: &io::Error) -> String {
    if is_permission_denied(err) {
        permission_error(path, operation)
    } else if is_not_found(err) {
        not_found_error(path, operation)
    } else {
        format!("Error when {} '{}': {}", operation, path.display(), err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::ErrorKind;

    #[test]
    fn test_is_permission_denied() {
        let perm_err = io::Error::new(ErrorKind::PermissionDenied, "access denied");
        assert!(is_permission_denied(&perm_err));

        let not_found_err = io::Error::new(ErrorKind::NotFound, "not found");
        assert!(!is_permission_denied(&not_found_err));
    }

    #[test]
    fn test_is_not_found() {
        let not_found_err = io::Error::new(ErrorKind::NotFound, "not found");
        assert!(is_not_found(&not_found_err));

        let perm_err = io::Error::new(ErrorKind::PermissionDenied, "access denied");
        assert!(!is_not_found(&perm_err));
    }

    #[test]
    fn test_permission_error_formatting() {
        let path = Path::new("/tmp/test.txt");
        let msg = permission_error(path, "reading");
        assert!(msg.contains("Permission denied"));
        assert!(msg.contains("reading"));
        assert!(msg.contains("/tmp/test.txt"));
        assert!(msg.contains("Possible fixes"));
    }

    #[test]
    fn test_not_found_error_formatting() {
        let path = Path::new("/home/user/file.txt");
        let msg = not_found_error(path, "processing file");
        assert!(msg.contains("File not found"));
        assert!(msg.contains("/home/user/file.txt"));
        assert!(msg.contains("processing file"));
        assert!(msg.contains("Possible fixes"));
    }
}
