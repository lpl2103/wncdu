//! Integration tests for wncdu scanner, tree model, and browser navigation.

use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use wncdu::browser::{BrowserState, SortColumn, SortOrder};
use wncdu::exclude::ExclusionFilter;
use wncdu::scanner::{ScanConfig, ScanMessage, start_scan};

fn create_test_directory() -> PathBuf {
    let base = std::env::temp_dir().join(format!("wncdu_itest_{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).expect("failed to create temp test root");

    // Create folder structure:
    // base/
    //   file_root.txt (1024 bytes)
    //   folder_a/
    //     sub1.bin (2048 bytes)
    //     sub2.log (512 bytes) -> to be excluded in tests
    //   folder_b/
    //     nested/
    //       deep.dat (4096 bytes)
    //   folder_empty/

    let mut f_root = File::create(base.join("file_root.txt")).unwrap();
    f_root.write_all(&vec![b'X'; 1024]).unwrap();

    let folder_a = base.join("folder_a");
    fs::create_dir_all(&folder_a).unwrap();
    let mut f_sub1 = File::create(folder_a.join("sub1.bin")).unwrap();
    f_sub1.write_all(&vec![b'A'; 2048]).unwrap();

    let mut f_sub2 = File::create(folder_a.join("sub2.log")).unwrap();
    f_sub2.write_all(&vec![b'L'; 512]).unwrap();

    let folder_b = base.join("folder_b").join("nested");
    fs::create_dir_all(&folder_b).unwrap();
    let mut f_deep = File::create(folder_b.join("deep.dat")).unwrap();
    f_deep.write_all(&vec![b'D'; 4096]).unwrap();

    let folder_empty = base.join("folder_empty");
    fs::create_dir_all(&folder_empty).unwrap();

    base
}

#[test]
fn test_scanner_and_browser_workflow() {
    let test_dir = create_test_directory();

    let mut filter = ExclusionFilter::new();
    // Exclude all .log files
    filter.add_pattern("*.log").unwrap();

    let config = ScanConfig {
        root_path: test_dir.clone(),
        filter,
        follow_symlinks: false,
        same_fs: true,
    };

    let cancel_flag = Arc::new(AtomicBool::new(false));
    let (tx, rx) = crossbeam_channel::unbounded();

    start_scan(config, cancel_flag, tx);

    // Wait for scanning completion
    let tree = loop {
        match rx.recv().expect("scanner channel disconnected") {
            ScanMessage::Progress(_) => {}
            ScanMessage::Finished(t) => break t,
            ScanMessage::Aborted(_) => panic!("Scanner aborted unexpectedly"),
        }
    };

    // Assertions on the scanned tree
    // Expected files included:
    // - file_root.txt: 1024
    // - folder_a/sub1.bin: 2048
    // - folder_b/nested/deep.dat: 4096
    // sub2.log (512) was excluded
    // Total size: 1024 + 2048 + 4096 = 7168 bytes
    assert_eq!(tree.total_size(), 7168);

    // Total files: 3 files + 4 folders (folder_a, folder_b, nested, folder_empty)
    assert_eq!(tree.total_items(), 7);

    // Initialize interactive browser
    let mut browser = BrowserState::new(tree);
    assert_eq!(browser.current_dir, 0);

    // Default is sort by size descending, directories first
    assert_eq!(browser.sort_col, SortColumn::Size);
    assert_eq!(browser.sort_order, SortOrder::Descending);

    // Toggling sort_by(Size) should invert order to Ascending
    browser.sort_by(SortColumn::Size);
    assert_eq!(browser.sort_order, SortOrder::Ascending);

    // Toggle back to Descending for inspection
    browser.sort_by(SortColumn::Size);
    assert_eq!(browser.sort_order, SortOrder::Descending);

    // Top item should be a directory with the largest total size (folder_b with 4096 bytes)
    let first_entry = browser.selected_entry().expect("expected entry");
    assert!(first_entry.is_dir());

    // Test entering directory
    let entered = browser.enter_selected();
    assert!(entered);
    assert_ne!(browser.current_dir, 0);

    // Test going back
    let backed = browser.go_back();
    assert!(backed);
    assert_eq!(browser.current_dir, 0);

    // Clean up temporary directory
    let _ = fs::remove_dir_all(&test_dir);
}
