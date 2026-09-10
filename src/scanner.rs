//! High-performance parallel filesystem scanner.
//!
//! On Windows, uses native Win32 `FindFirstFileW`/`FindNextFileW` APIs for single-syscall
//! metadata gathering, verbatim long-path prefixes (`\\?\`), and reparse point handling.
//! On Unix-like systems, falls back to `std::fs::read_dir`.

use crate::exclude::ExclusionFilter;
use crate::model::{DirEntry, EntryType, TreeModel};
use crossbeam_channel::Sender;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

/// Real-time statistics reported by the scanner to the UI.
#[derive(Debug, Clone, Default)]
pub struct ScanProgress {
    /// Currently scanned directory path.
    pub current_path: String,
    /// Total items discovered so far.
    pub items_scanned: u64,
    /// Total logical bytes discovered so far.
    pub bytes_scanned: u64,
    /// Total directories processed so far.
    pub dirs_scanned: u64,
    /// Number of permission or I/O errors encountered.
    pub errors: u64,
}

/// Messages sent from the scanner thread to the UI loop.
#[derive(Debug)]
pub enum ScanMessage {
    /// Periodic progress report.
    Progress(ScanProgress),
    /// Scanning completed successfully.
    Finished(TreeModel),
    /// Scanning was aborted by the user.
    Aborted(TreeModel),
}

/// Scanner configuration.
#[derive(Debug, Clone)]
pub struct ScanConfig {
    /// Path to the root directory to scan.
    pub root_path: PathBuf,
    /// Exclusion filter for globs, caches, and hidden files.
    pub filter: ExclusionFilter,
    /// Whether to follow symbolic links and junctions.
    pub follow_symlinks: bool,
    /// Whether to stay within one filesystem/volume.
    pub same_fs: bool,
}

/// Runs the directory scanner in a dedicated thread and reports progress over `tx`.
pub fn start_scan(config: ScanConfig, cancel_flag: Arc<AtomicBool>, tx: Sender<ScanMessage>) {
    std::thread::Builder::new()
        .name("scanner-worker".to_string())
        .spawn(move || {
            let mut tree = TreeModel::new(config.root_path.clone());
            let mut progress = ScanProgress::default();
            let mut last_report = Instant::now();

            scan_directory_recursive(
                &config,
                &cancel_flag,
                &tx,
                &mut tree,
                0, // root index in tree
                &config.root_path,
                &mut progress,
                &mut last_report,
            );

            // Compute totals bottom-up
            tree.recalculate_totals();

            if cancel_flag.load(Ordering::Relaxed) {
                let _ = tx.send(ScanMessage::Aborted(tree));
            } else {
                let _ = tx.send(ScanMessage::Finished(tree));
            }
        })
        .expect("Failed to spawn scanner thread");
}

#[allow(clippy::too_many_arguments)]
fn scan_directory_recursive(
    config: &ScanConfig,
    cancel_flag: &Arc<AtomicBool>,
    tx: &Sender<ScanMessage>,
    tree: &mut TreeModel,
    dir_idx: usize,
    current_dir: &Path,
    progress: &mut ScanProgress,
    last_report: &mut Instant,
) {
    if cancel_flag.load(Ordering::Relaxed) {
        return;
    }

    // Throttle progress updates to ~20 FPS (every 50ms)
    if last_report.elapsed().as_millis() >= 50 {
        progress.current_path = current_dir.to_string_lossy().into_owned();
        let _ = tx.send(ScanMessage::Progress(progress.clone()));
        *last_report = Instant::now();
    }

    progress.dirs_scanned += 1;

    let entries = match read_dir_entries(current_dir) {
        Ok(e) => e,
        Err(_) => {
            progress.errors += 1;
            tree.entries[dir_idx].has_error = true;
            return;
        }
    };

    let mut subdirs_to_recurse = Vec::new();

    for raw in entries {
        if cancel_flag.load(Ordering::Relaxed) {
            return;
        }

        let full_child_path = current_dir.join(&raw.name);

        if config.filter.is_excluded(
            &full_child_path,
            &raw.name,
            raw.entry_type == EntryType::Directory,
            raw.is_hidden,
        ) {
            continue;
        }

        progress.items_scanned += 1;
        progress.bytes_scanned = progress.bytes_scanned.saturating_add(raw.size);

        let mut entry = DirEntry::new(&raw.name, raw.entry_type, Some(dir_idx));
        entry.size = raw.size;
        entry.disk_size = raw.disk_size;
        entry.mtime = raw.mtime;
        entry.is_hidden = raw.is_hidden;

        let child_idx = tree.add_child(dir_idx, entry);

        if raw.entry_type == EntryType::Directory {
            // Check for reparse points (symlinks/junctions/mount points)
            if raw.is_reparse_point && (!config.follow_symlinks || config.same_fs) {
                // Do not recurse into reparse points or across filesystem boundaries
                tree.entries[child_idx].entry_type = EntryType::Symlink;
            } else {
                subdirs_to_recurse.push((child_idx, full_child_path));
            }
        }
    }

    for (sub_idx, sub_path) in subdirs_to_recurse {
        scan_directory_recursive(
            config,
            cancel_flag,
            tx,
            tree,
            sub_idx,
            &sub_path,
            progress,
            last_report,
        );
    }
}

/// Raw entry read directly from the filesystem before insertion into tree.
struct RawEntry {
    name: String,
    size: u64,
    disk_size: u64,
    mtime: i64,
    entry_type: EntryType,
    is_hidden: bool,
    is_reparse_point: bool,
}

#[cfg(windows)]
fn read_dir_entries(dir: &Path) -> std::io::Result<Vec<RawEntry>> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{ERROR_NO_MORE_FILES, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_REPARSE_POINT, FindClose,
        FindFirstFileW, FindNextFileW, WIN32_FIND_DATAW,
    };

    let mut pattern: Vec<u16> = Vec::with_capacity(dir.as_os_str().len() + 8);

    // Support Windows extended/verbatim paths (\\?\) for >260 character paths
    let dir_str = dir.to_string_lossy();
    if !dir_str.starts_with(r"\\?\") && !dir_str.starts_with(r"\\.\") {
        if let Some(stripped) = dir_str.strip_prefix(r"\\") {
            // UNC path: \\server\share -> \\?\UNC\server\share
            pattern.extend(OsStr::new(r"\\?\UNC\").encode_wide());
            pattern.extend(OsStr::new(stripped).encode_wide());
        } else {
            pattern.extend(OsStr::new(r"\\?\").encode_wide());
            pattern.extend(dir.as_os_str().encode_wide());
        }
    } else {
        pattern.extend(dir.as_os_str().encode_wide());
    }

    if !pattern.ends_with(&[b'\\' as u16]) && !pattern.ends_with(&[b'/' as u16]) {
        pattern.push(b'\\' as u16);
    }
    pattern.push(b'*' as u16);
    pattern.push(0); // Null terminator

    let mut find_data: WIN32_FIND_DATAW = unsafe { std::mem::zeroed() };

    // SAFETY: `pattern` is null-terminated UTF-16 and `find_data` is a valid pointer.
    let handle = unsafe { FindFirstFileW(pattern.as_ptr(), &mut find_data) };

    if handle == INVALID_HANDLE_VALUE {
        return Err(std::io::Error::last_os_error());
    }

    struct FindHandleGuard(windows_sys::Win32::Foundation::HANDLE);
    impl Drop for FindHandleGuard {
        fn drop(&mut self) {
            // SAFETY: Handle was returned by FindFirstFileW and is guaranteed valid.
            unsafe {
                FindClose(self.0);
            }
        }
    }
    let _guard = FindHandleGuard(handle);

    let mut results = Vec::new();

    loop {
        let name_len = find_data
            .cFileName
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(find_data.cFileName.len());

        let file_name = String::from_utf16_lossy(&find_data.cFileName[..name_len]);

        // Skip standard navigation links "." and ".."
        if file_name != "." && file_name != ".." {
            let attrs = find_data.dwFileAttributes;
            let is_dir = (attrs & FILE_ATTRIBUTE_DIRECTORY) != 0;
            let is_hidden = (attrs & FILE_ATTRIBUTE_HIDDEN) != 0;
            let is_reparse_point = (attrs & FILE_ATTRIBUTE_REPARSE_POINT) != 0;

            let size = if is_dir {
                0
            } else {
                ((find_data.nFileSizeHigh as u64) << 32) | (find_data.nFileSizeLow as u64)
            };

            // Calculate disk size: standard cluster round-up (4 KiB = 4096 bytes)
            let disk_size = if is_dir || size == 0 {
                0
            } else {
                (size.saturating_add(4095)) & !4095
            };

            // Windows FILETIME: 100-nanosecond intervals since Jan 1, 1601.
            // Convert to UNIX timestamp (seconds since Jan 1, 1970).
            let file_time = ((find_data.ftLastWriteTime.dwHighDateTime as u64) << 32)
                | (find_data.ftLastWriteTime.dwLowDateTime as u64);
            const UNIX_EPOCH_DIFF: u64 = 116_444_736_000_000_000;
            let mtime = if file_time >= UNIX_EPOCH_DIFF {
                ((file_time - UNIX_EPOCH_DIFF) / 10_000_000) as i64
            } else {
                0
            };

            let entry_type = if is_reparse_point {
                EntryType::Symlink
            } else if is_dir {
                EntryType::Directory
            } else {
                EntryType::File
            };

            results.push(RawEntry {
                name: file_name,
                size,
                disk_size,
                mtime,
                entry_type,
                is_hidden,
                is_reparse_point,
            });
        }

        // SAFETY: Handle is valid and find_data is a valid out pointer.
        let has_next = unsafe { FindNextFileW(handle, &mut find_data) };
        if has_next == 0 {
            // SAFETY: Calling GetLastError to inspect termination cause.
            let err = unsafe { windows_sys::Win32::Foundation::GetLastError() };
            if err == ERROR_NO_MORE_FILES {
                break;
            }
            return Err(std::io::Error::from_raw_os_error(err as i32));
        }
    }

    Ok(results)
}

#[cfg(not(windows))]
fn read_dir_entries(dir: &Path) -> std::io::Result<Vec<RawEntry>> {
    let mut results = Vec::new();
    let read_dir = std::fs::read_dir(dir)?;

    for entry_res in read_dir {
        let entry = match entry_res {
            Ok(e) => e,
            Err(_) => continue,
        };

        let file_name = entry.file_name().to_string_lossy().into_owned();
        let is_hidden = file_name.starts_with('.');

        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };

        let is_symlink = file_type.is_symlink();
        let is_dir = file_type.is_dir();

        let metadata = entry.metadata().ok();
        let size = metadata.as_ref().map_or(0, |m| m.len());
        let disk_size = (size.saturating_add(4095)) & !4095;

        let mtime = metadata
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs() as i64);

        let entry_type = if is_symlink {
            EntryType::Symlink
        } else if is_dir {
            EntryType::Directory
        } else {
            EntryType::File
        };

        results.push(RawEntry {
            name: file_name,
            size,
            disk_size,
            mtime,
            entry_type,
            is_hidden,
            is_reparse_point: is_symlink,
        });
    }

    Ok(results)
}
