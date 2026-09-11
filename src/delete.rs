//! File and directory deletion engine.
//!
//! Handles recursive deletion of folders, handling of read-only attributes on Windows,
//! and atomic error handling.

use std::fs;
use std::path::Path;

/// Deletes a file or directory tree permanently from the filesystem.
///
/// If deleting a directory, performs a recursive removal.
/// On Windows, clears read-only attributes if necessary to prevent `AccessDenied` errors.
///
/// # Errors
/// Returns an I/O error if the file or folder could not be removed.
pub fn delete_entry(path: &Path) -> std::io::Result<()> {
    if !path.exists() && !path.is_symlink() {
        return Ok(());
    }

    if path.is_dir() && !path.is_symlink() {
        remove_dir_all_resilient(path)
    } else {
        remove_file_resilient(path)
    }
}

fn remove_file_resilient(path: &Path) -> std::io::Result<()> {
    if let Err(e) = fs::remove_file(path) {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            // Attempt to strip read-only flag
            if let Ok(metadata) = fs::metadata(path) {
                let mut perms = metadata.permissions();
                #[allow(clippy::permissions_set_readonly_false)]
                perms.set_readonly(false);
                let _ = fs::set_permissions(path, perms);
                return fs::remove_file(path);
            }
        }
        return Err(e);
    }
    Ok(())
}

fn remove_dir_all_resilient(path: &Path) -> std::io::Result<()> {
    if let Err(e) = fs::remove_dir_all(path) {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            // Reset readonly recursively if permission denied
            strip_readonly_recursive(path);
            return fs::remove_dir_all(path);
        }
        return Err(e);
    }
    Ok(())
}

fn strip_readonly_recursive(dir: &Path) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let child_path = entry.path();
            if let Ok(meta) = entry.metadata() {
                let mut perms = meta.permissions();
                if perms.readonly() {
                    #[allow(clippy::permissions_set_readonly_false)]
                    perms.set_readonly(false);
                    let _ = fs::set_permissions(&child_path, perms);
                }
            }
            if child_path.is_dir() && !child_path.is_symlink() {
                strip_readonly_recursive(&child_path);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn test_delete_file() {
        let temp_dir = std::env::temp_dir().join("wncdu_test_delete_file");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let file_path = temp_dir.join("readonly.txt");
        let mut f = File::create(&file_path).unwrap();
        f.write_all(b"hello delete").unwrap();
        drop(f);

        // Make readonly
        let mut perms = fs::metadata(&file_path).unwrap().permissions();
        perms.set_readonly(true);
        fs::set_permissions(&file_path, perms).unwrap();

        assert!(delete_entry(&file_path).is_ok());
        assert!(!file_path.exists());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_delete_directory_recursive() {
        let temp_dir = std::env::temp_dir().join("wncdu_test_delete_dir");
        let sub_dir = temp_dir.join("nested");
        fs::create_dir_all(&sub_dir).unwrap();
        File::create(sub_dir.join("child.txt")).unwrap();

        assert!(sub_dir.join("child.txt").exists());
        delete_entry(&temp_dir).unwrap();
        assert!(!temp_dir.exists());
    }
}
