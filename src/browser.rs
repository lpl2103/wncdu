//! Interactive directory tree browser and navigation state.

use crate::model::{DirEntry, EntryType, TreeModel};
use crate::util::GraphStyle;
use std::cmp::Ordering;

/// Columns by which entries can be sorted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortColumn {
    /// Sort by apparent size (default in ncdu).
    #[default]
    Size,
    /// Sort by allocated disk size.
    DiskSize,
    /// Sort alphabetically by name.
    Name,
    /// Sort by total recursive items count.
    Items,
    /// Sort by last modification time.
    Mtime,
}

/// Ordering direction for sorting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortOrder {
    /// Ascending (smallest / A to Z first).
    Ascending,
    /// Descending (largest / Z to A first).
    #[default]
    Descending,
}

/// Complete navigation and view state for the interactive file manager.
#[derive(Debug, Clone)]
pub struct BrowserState {
    /// The analyzed directory tree arena.
    pub tree: TreeModel,
    /// Currently visited directory index in the arena.
    pub current_dir: usize,
    /// Cursor index inside `cached_children`.
    pub selected_index: usize,
    /// Viewport vertical scroll offset.
    pub scroll_offset: usize,
    /// Active sort column.
    pub sort_col: SortColumn,
    /// Active sort order.
    pub sort_order: SortOrder,
    /// Whether to always group directories at the top.
    pub sort_dirs_first: bool,
    /// Whether to display hidden files and directories.
    pub show_hidden: bool,
    /// Whether to render the visual usage graph.
    pub show_graph: bool,
    /// Whether to display percentage of parent size.
    pub show_percent: bool,
    /// Whether to display the item count column.
    pub show_items: bool,
    /// Whether to display the modification time column.
    pub show_mtime: bool,
    /// Graph bar visual style.
    pub graph_style: GraphStyle,
    /// Whether to format units with SI (1000) instead of binary (1024).
    pub use_si_units: bool,
    /// Cached and sorted children indices of `current_dir`.
    pub cached_children: Vec<usize>,
    /// Navigation history stack storing `(parent_dir_idx, previous_selected_index)`.
    pub history: Vec<(usize, usize)>,
}

impl BrowserState {
    /// Creates a new browser state pointing to the root directory.
    #[must_use]
    pub fn new(tree: TreeModel) -> Self {
        let mut state = Self {
            tree,
            current_dir: 0,
            selected_index: 0,
            scroll_offset: 0,
            sort_col: SortColumn::Size,
            sort_order: SortOrder::Descending,
            sort_dirs_first: true,
            show_hidden: true,
            show_graph: true,
            show_percent: false,
            show_items: false,
            show_mtime: false,
            graph_style: GraphStyle::Hash,
            use_si_units: false,
            cached_children: Vec::new(),
            history: Vec::new(),
        };
        state.refresh_cached_children();
        state
    }

    /// Re-filters and sorts the child entries of `current_dir`.
    pub fn refresh_cached_children(&mut self) {
        if self.current_dir >= self.tree.entries.len() {
            self.cached_children.clear();
            return;
        }

        let raw_children = &self.tree.entries[self.current_dir].children;
        let mut filtered: Vec<usize> = raw_children
            .iter()
            .copied()
            .filter(|&idx| {
                if !self.show_hidden && self.tree.entries[idx].is_hidden {
                    return false;
                }
                true
            })
            .collect();

        let entries = &self.tree.entries;
        let sort_col = self.sort_col;
        let sort_order = self.sort_order;
        let dirs_first = self.sort_dirs_first;

        filtered.sort_by(|&a_idx, &b_idx| {
            let a = &entries[a_idx];
            let b = &entries[b_idx];

            if dirs_first {
                match (a.is_dir(), b.is_dir()) {
                    (true, false) => return Ordering::Less,
                    (false, true) => return Ordering::Greater,
                    _ => {}
                }
            }

            let ord = match sort_col {
                SortColumn::Size => a.size.cmp(&b.size),
                SortColumn::DiskSize => a.disk_size.cmp(&b.disk_size),
                SortColumn::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                SortColumn::Items => a.items.cmp(&b.items),
                SortColumn::Mtime => a.mtime.cmp(&b.mtime),
            };

            let primary = match sort_order {
                SortOrder::Ascending => ord,
                SortOrder::Descending => ord.reverse(),
            };

            if primary == Ordering::Equal {
                a.name.cmp(&b.name)
            } else {
                primary
            }
        });

        self.cached_children = filtered;

        if self.selected_index >= self.cached_children.len() {
            self.selected_index = self.cached_children.len().saturating_sub(1);
        }
    }

    /// Returns a reference to the currently selected entry, if any.
    #[must_use]
    pub fn selected_entry(&self) -> Option<&DirEntry> {
        self.cached_children
            .get(self.selected_index)
            .map(|&idx| &self.tree.entries[idx])
    }

    /// Returns the arena index of the currently selected entry, if any.
    #[must_use]
    pub fn selected_entry_index(&self) -> Option<usize> {
        self.cached_children.get(self.selected_index).copied()
    }

    /// Moves the selection cursor up by 1 entry.
    pub fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            if self.selected_index < self.scroll_offset {
                self.scroll_offset = self.selected_index;
            }
        }
    }

    /// Moves the selection cursor down by 1 entry.
    pub fn move_down(&mut self, max_visible: usize) {
        if !self.cached_children.is_empty() && self.selected_index + 1 < self.cached_children.len()
        {
            self.selected_index += 1;
            if self.selected_index >= self.scroll_offset + max_visible {
                self.scroll_offset = self.selected_index + 1 - max_visible;
            }
        }
    }

    /// Moves cursor up by a page.
    pub fn page_up(&mut self, page_size: usize) {
        self.selected_index = self.selected_index.saturating_sub(page_size);
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        }
    }

    /// Moves cursor down by a page.
    pub fn page_down(&mut self, page_size: usize, max_visible: usize) {
        if !self.cached_children.is_empty() {
            self.selected_index =
                (self.selected_index + page_size).min(self.cached_children.len() - 1);
            if self.selected_index >= self.scroll_offset + max_visible {
                self.scroll_offset = (self.selected_index + 1).saturating_sub(max_visible);
            }
        }
    }

    /// Jumps to the very first entry.
    pub fn home(&mut self) {
        self.selected_index = 0;
        self.scroll_offset = 0;
    }

    /// Jumps to the very last entry.
    pub fn end(&mut self, max_visible: usize) {
        if !self.cached_children.is_empty() {
            self.selected_index = self.cached_children.len() - 1;
            if self.selected_index >= max_visible {
                self.scroll_offset = self.selected_index + 1 - max_visible;
            } else {
                self.scroll_offset = 0;
            }
        }
    }

    /// Enters into the selected entry if it is a directory.
    /// Returns `true` if entered successfully.
    pub fn enter_selected(&mut self) -> bool {
        let is_dir = self
            .cached_children
            .get(self.selected_index)
            .is_some_and(|&idx| self.tree.entries[idx].entry_type == EntryType::Directory);

        if is_dir {
            let child_idx = self.cached_children[self.selected_index];
            self.history.push((self.current_dir, self.selected_index));
            self.current_dir = child_idx;
            self.selected_index = 0;
            self.scroll_offset = 0;
            self.refresh_cached_children();
            true
        } else {
            false
        }
    }

    /// Navigates up to the parent directory.
    /// Returns `true` if navigated up, or `false` if already at root.
    pub fn go_back(&mut self) -> bool {
        if let Some((prev_dir, prev_selected)) = self.history.pop() {
            self.current_dir = prev_dir;
            self.selected_index = prev_selected;
            self.scroll_offset = 0;
            self.refresh_cached_children();
            return true;
        }

        if let Some(parent_idx) = self.tree.entries[self.current_dir].parent {
            let old_dir = self.current_dir;
            self.current_dir = parent_idx;
            self.refresh_cached_children();
            self.selected_index = self
                .cached_children
                .iter()
                .position(|&idx| idx == old_dir)
                .unwrap_or(0);
            self.scroll_offset = 0;
            return true;
        }

        false
    }

    /// Sets the sort column or inverts sort order if already active.
    pub fn sort_by(&mut self, col: SortColumn) {
        if self.sort_col == col {
            self.toggle_sort_order();
        } else {
            self.sort_col = col;
            self.sort_order = match col {
                SortColumn::Name => SortOrder::Ascending,
                _ => SortOrder::Descending,
            };
            self.refresh_cached_children();
        }
    }

    /// Toggles between ascending and descending sort order.
    pub fn toggle_sort_order(&mut self) {
        self.sort_order = match self.sort_order {
            SortOrder::Ascending => SortOrder::Descending,
            SortOrder::Descending => SortOrder::Ascending,
        };
        self.refresh_cached_children();
    }

    /// Toggles directories-first grouping.
    pub fn toggle_dirs_first(&mut self) {
        self.sort_dirs_first = !self.sort_dirs_first;
        self.refresh_cached_children();
    }

    /// Toggles hidden file visibility.
    pub fn toggle_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
        self.refresh_cached_children();
    }

    /// Toggles usage graph visibility.
    pub fn toggle_graph(&mut self) {
        self.show_graph = !self.show_graph;
    }

    /// Toggles percentage display.
    pub fn toggle_percent(&mut self) {
        self.show_percent = !self.show_percent;
    }

    /// Toggles item count column.
    pub fn toggle_items(&mut self) {
        self.show_items = !self.show_items;
    }

    /// Toggles modification time column.
    pub fn toggle_mtime(&mut self) {
        self.show_mtime = !self.show_mtime;
    }

    /// Toggles graph style between hash `#` and unicode blocks.
    pub fn toggle_graph_style(&mut self) {
        self.graph_style = match self.graph_style {
            GraphStyle::Hash => GraphStyle::Blocks,
            GraphStyle::Blocks => GraphStyle::Hash,
        };
    }

    /// Removes the currently selected entry from both the arena and cached view.
    pub fn remove_selected(&mut self) -> Option<DirEntry> {
        let child_idx = self.selected_entry_index()?;
        let removed = self.tree.remove_entry(child_idx);
        self.refresh_cached_children();
        removed
    }
}
