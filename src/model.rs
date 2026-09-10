//! In-memory flat-arena directory tree model.
//!
//! Stores directory entries in a contiguous vector (`Vec<DirEntry>`) using index-based
//! parent/child relationships to maximize cache locality and avoid pointer fragmentation.

use compact_str::CompactString;
use std::path::PathBuf;

/// The type of file system entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryType {
    /// A directory that may contain children.
    Directory,
    /// A regular file.
    File,
    /// A symbolic link or junction point.
    Symlink,
    /// Other special file (device, socket, pipe).
    Other,
}

/// A single node in the directory tree arena.
#[derive(Debug, Clone)]
pub struct DirEntry {
    /// Name of the file or directory.
    pub name: CompactString,
    /// Apparent size in bytes (uncompressed file content).
    pub size: u64,
    /// Allocated disk size in bytes (clusters on filesystem).
    pub disk_size: u64,
    /// Total recursive item count (1 for files; sum of contents for dirs).
    pub items: u64,
    /// Modification time as UNIX timestamp (seconds since epoch).
    pub mtime: i64,
    /// File system entry type.
    pub entry_type: EntryType,
    /// Whether the file or directory has the hidden attribute.
    pub is_hidden: bool,
    /// Whether an error occurred reading this entry or any child.
    pub has_error: bool,
    /// Arena index of the parent directory (`None` for root).
    pub parent: Option<usize>,
    /// Arena indices of child entries (empty for files).
    pub children: Vec<usize>,
}

impl DirEntry {
    /// Creates a new directory entry with default zeroed metrics.
    #[must_use]
    pub fn new(
        name: impl Into<CompactString>,
        entry_type: EntryType,
        parent: Option<usize>,
    ) -> Self {
        Self {
            name: name.into(),
            size: 0,
            disk_size: 0,
            items: if entry_type == EntryType::Directory {
                0
            } else {
                1
            },
            mtime: 0,
            entry_type,
            is_hidden: false,
            has_error: false,
            parent,
            children: Vec::new(),
        }
    }

    /// Returns `true` if this entry is a directory.
    #[must_use]
    pub fn is_dir(&self) -> bool {
        self.entry_type == EntryType::Directory
    }
}

/// The entire analyzed directory tree stored in a flat arena.
#[derive(Debug, Clone)]
pub struct TreeModel {
    /// Contiguous arena of all entries. Root is always at index 0.
    pub entries: Vec<DirEntry>,
    /// Base absolute path of the root directory.
    pub root_path: PathBuf,
}

impl TreeModel {
    /// Initializes a new empty tree with the given root path.
    #[must_use]
    pub fn new(root_path: PathBuf) -> Self {
        let root_name = root_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(root_path.to_str().unwrap_or("root"));

        let root_entry = DirEntry::new(root_name, EntryType::Directory, None);
        Self {
            entries: vec![root_entry],
            root_path,
        }
    }

    /// Appends a new entry to the arena and attaches it to its parent.
    /// Returns the index of the newly created entry.
    pub fn add_child(&mut self, parent_idx: usize, mut entry: DirEntry) -> usize {
        let child_idx = self.entries.len();
        entry.parent = Some(parent_idx);
        self.entries.push(entry);
        self.entries[parent_idx].children.push(child_idx);
        child_idx
    }

    /// Recalculates `size`, `disk_size`, and `items` for all directory entries
    /// recursively from bottom to top (post-order traversal).
    pub fn recalculate_totals(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        self.recalculate_node(0);
    }

    fn recalculate_node(&mut self, idx: usize) -> (u64, u64, u64, bool) {
        if !self.entries[idx].is_dir() {
            return (
                self.entries[idx].size,
                self.entries[idx].disk_size,
                1,
                self.entries[idx].has_error,
            );
        }

        let children = self.entries[idx].children.clone();
        let mut total_size = 0u64;
        let mut total_disk_size = 0u64;
        let mut total_items = 0u64;
        let mut any_error = self.entries[idx].has_error;

        for &child_idx in &children {
            let (c_size, c_disk, c_items, c_err) = self.recalculate_node(child_idx);
            total_size = total_size.saturating_add(c_size);
            total_disk_size = total_disk_size.saturating_add(c_disk);
            total_items = total_items.saturating_add(c_items);
            if c_err {
                any_error = true;
            }
        }

        self.entries[idx].size = total_size;
        self.entries[idx].disk_size = total_disk_size;
        self.entries[idx].items = total_items;
        self.entries[idx].has_error = any_error;

        (total_size, total_disk_size, total_items, any_error)
    }

    /// Reconstructs the full absolute filesystem path for an entry by walking up its parents.
    #[must_use]
    pub fn get_path(&self, mut idx: usize) -> PathBuf {
        let mut components = Vec::new();
        while let Some(parent) = self.entries[idx].parent {
            components.push(self.entries[idx].name.as_str());
            idx = parent;
        }

        let mut path = self.root_path.clone();
        for comp in components.iter().rev() {
            path.push(comp);
        }
        path
    }

    /// Removes an entry from the tree (and detaches it from its parent).
    /// Updates size, disk_size, and items on all ancestor directories.
    pub fn remove_entry(&mut self, idx: usize) -> Option<DirEntry> {
        if idx >= self.entries.len() || idx == 0 {
            // Cannot remove root or out of bounds
            return None;
        }

        let entry = self.entries[idx].clone();
        let sub_size = entry.size;
        let sub_disk = entry.disk_size;
        let sub_items = entry.items;

        // Remove from parent's children list
        if let Some(parent_idx) = entry.parent {
            if let Some(pos) = self.entries[parent_idx]
                .children
                .iter()
                .position(|&c| c == idx)
            {
                self.entries[parent_idx].children.remove(pos);
            }

            // Propagate reduction up to ancestors
            let mut curr = Some(parent_idx);
            while let Some(p) = curr {
                self.entries[p].size = self.entries[p].size.saturating_sub(sub_size);
                self.entries[p].disk_size = self.entries[p].disk_size.saturating_sub(sub_disk);
                self.entries[p].items = self.entries[p].items.saturating_sub(sub_items);
                curr = self.entries[p].parent;
            }
        }

        Some(entry)
    }

    /// Returns the total size of the root directory.
    #[must_use]
    pub fn total_size(&self) -> u64 {
        self.entries.first().map_or(0, |r| r.size)
    }

    /// Returns the total disk usage of the root directory.
    #[must_use]
    pub fn total_disk_size(&self) -> u64 {
        self.entries.first().map_or(0, |r| r.disk_size)
    }

    /// Returns the total items of the root directory.
    #[must_use]
    pub fn total_items(&self) -> u64 {
        self.entries.first().map_or(0, |r| r.items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_tree_construction_and_recalculation() {
        let mut tree = TreeModel::new(PathBuf::from("C:\\test"));
        assert_eq!(tree.entries.len(), 1);

        // Add folder1
        let dir1 = DirEntry::new("folder1", EntryType::Directory, None);
        let dir1_idx = tree.add_child(0, dir1);

        // Add file1 (1000 bytes)
        let mut file1 = DirEntry::new("file1.txt", EntryType::File, None);
        file1.size = 1000;
        file1.disk_size = 4096;
        tree.add_child(dir1_idx, file1);

        // Add file2 (2000 bytes)
        let mut file2 = DirEntry::new("file2.txt", EntryType::File, None);
        file2.size = 2000;
        file2.disk_size = 4096;
        tree.add_child(dir1_idx, file2);

        tree.recalculate_totals();

        assert_eq!(tree.entries[dir1_idx].size, 3000);
        assert_eq!(tree.entries[dir1_idx].disk_size, 8192);
        assert_eq!(tree.entries[dir1_idx].items, 2);

        assert_eq!(tree.total_size(), 3000);
        assert_eq!(tree.total_disk_size(), 8192);
        assert_eq!(tree.total_items(), 2);
    }

    #[test]
    fn test_remove_entry_propagates() {
        let mut tree = TreeModel::new(PathBuf::from("C:\\test"));
        let dir1 = DirEntry::new("folder1", EntryType::Directory, None);
        let dir1_idx = tree.add_child(0, dir1);

        let mut file1 = DirEntry::new("file1.txt", EntryType::File, None);
        file1.size = 1000;
        file1.disk_size = 4096;
        let file1_idx = tree.add_child(dir1_idx, file1);

        tree.recalculate_totals();
        assert_eq!(tree.total_size(), 1000);

        tree.remove_entry(file1_idx);
        assert_eq!(tree.total_size(), 0);
        assert_eq!(tree.total_items(), 0);
        assert!(tree.entries[dir1_idx].children.is_empty());
    }

    #[test]
    fn test_get_path() {
        let mut tree = TreeModel::new(PathBuf::from("C:\\test"));
        let dir1 = DirEntry::new("sub", EntryType::Directory, None);
        let dir1_idx = tree.add_child(0, dir1);
        let file = DirEntry::new("doc.pdf", EntryType::File, None);
        let file_idx = tree.add_child(dir1_idx, file);

        let path = tree.get_path(file_idx);
        assert_eq!(path, Path::new("C:\\test\\sub\\doc.pdf"));
    }
}
