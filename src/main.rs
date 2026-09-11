//! wncdu — Fast disk usage analyzer with TUI for Windows, inspired by ncdu.
//!
//! A modern, high-performance ncdu port for Windows written in Rust with ratatui.

use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use wncdu::browser::{BrowserState, SortColumn};
use wncdu::delete::delete_entry;
use wncdu::exclude::ExclusionFilter;
use wncdu::model::DirEntry;
use wncdu::scanner::{ScanConfig, ScanMessage, ScanProgress, start_scan};
use wncdu::ui;

/// WNCDU - Analisador de uso de disco para Windows com interface interativa (estilo ncdu)
#[derive(Parser, Debug)]
#[command(
    name = "wncdu",
    author,
    version,
    about = "Analisador de uso de disco interativo e de alto desempenho para Windows, inspirado no ncdu",
    long_about = None
)]
struct Cli {
    /// Caminho do diretório ou unidade para analisar
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Não cruzar limites de partições / sistemas de arquivos
    #[arg(short = 'x', long)]
    one_file_system: bool,

    /// Seguir links simbólicos e junções do Windows
    #[arg(short = 'L', long)]
    follow_symlinks: bool,

    /// Excluir arquivos e pastas que correspondam ao padrão glob
    #[arg(long = "exclude", value_name = "PADRÃO")]
    exclude: Vec<String>,

    /// Excluir pastas de cache contendo CACHEDIR.TAG
    #[arg(long)]
    exclude_caches: bool,

    /// Excluir arquivos e pastas ocultos
    #[arg(long)]
    exclude_hidden: bool,

    /// Usar unidades decimais SI (potências de 1000: KB, MB, GB) em vez de binárias (potências de 1024: KiB, MiB, GiB)
    #[arg(long)]
    si: bool,
}

/// Active modal dialog overlay.
#[derive(Debug, Clone)]
enum ModalState {
    None,
    Help,
    DeleteConfirm {
        item_name: String,
        is_dir: bool,
        size: u64,
        target_path: PathBuf,
    },
    Info {
        entry: DirEntry,
        full_path: PathBuf,
    },
}

/// Active top-level application state.
enum AppState {
    Scanning {
        progress: ScanProgress,
        rx: crossbeam_channel::Receiver<ScanMessage>,
        cancel_flag: Arc<AtomicBool>,
    },
    Browsing {
        browser: BrowserState,
        modal: ModalState,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Canonicalize or normalize root path
    let target_path = if cli.path.is_relative() {
        std::env::current_dir()?.join(cli.path)
    } else {
        cli.path
    };

    let target_path = target_path.canonicalize().unwrap_or(target_path);

    // Build exclusion filter
    let mut filter = ExclusionFilter::new();
    filter.exclude_caches = cli.exclude_caches;
    filter.exclude_hidden = cli.exclude_hidden;
    for pat in &cli.exclude {
        if let Err(e) = filter.add_pattern(pat) {
            eprintln!("Aviso: Padrão de exclusão inválido '{pat}': {e}");
        }
    }

    let scan_config = ScanConfig {
        root_path: target_path,
        filter,
        follow_symlinks: cli.follow_symlinks,
        same_fs: cli.one_file_system,
    };

    let cancel_flag = Arc::new(AtomicBool::new(false));
    let (tx, rx) = crossbeam_channel::unbounded();

    // Start background scan worker
    start_scan(scan_config, Arc::clone(&cancel_flag), tx);

    // Setup terminal with panic safety
    setup_panic_hook();
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut state = AppState::Scanning {
        progress: ScanProgress::default(),
        rx,
        cancel_flag,
    };

    let res = run_event_loop(&mut terminal, &mut state, cli.si);

    // Teardown terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Erro na aplicação: {err:#}");
    }

    Ok(())
}

fn run_event_loop<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    state: &mut AppState,
    is_si: bool,
) -> Result<()> {
    loop {
        match state {
            AppState::Scanning {
                progress,
                rx,
                cancel_flag,
            } => {
                let mut finished_tree = None;
                // Drain any incoming progress messages
                while let Ok(msg) = rx.try_recv() {
                    match msg {
                        ScanMessage::Progress(new_prog) => {
                            *progress = new_prog;
                        }
                        ScanMessage::Finished(tree) | ScanMessage::Aborted(tree) => {
                            finished_tree = Some(tree);
                            break;
                        }
                    }
                }

                if let Some(tree) = finished_tree {
                    let mut browser = BrowserState::new(tree);
                    browser.use_si_units = is_si;
                    *state = AppState::Browsing {
                        browser,
                        modal: ModalState::None,
                    };
                    continue;
                }

                // Draw scan screen
                terminal.draw(|f| {
                    ui::render_scan_screen(f, progress, is_si);
                })?;

                // Check for user keyboard cancellation
                if event::poll(Duration::from_millis(30))? {
                    let is_quit = matches!(
                        event::read()?,
                        Event::Key(key)
                            if key.kind == KeyEventKind::Press
                                && (key.code == KeyCode::Char('q')
                                    || (key.modifiers.contains(KeyModifiers::CONTROL)
                                        && key.code == KeyCode::Char('c')))
                    );
                    if is_quit {
                        cancel_flag.store(true, Ordering::Relaxed);
                    }
                }
            }

            AppState::Browsing { browser, modal } => {
                let max_visible_rows = terminal.size()?.height.saturating_sub(5) as usize;

                // Render main browser and any active modal
                terminal.draw(|f| {
                    ui::render_browser_screen(f, browser);

                    match modal {
                        ModalState::None => {}
                        ModalState::Help => {
                            ui::render_help_modal(f);
                        }
                        ModalState::DeleteConfirm {
                            item_name,
                            is_dir,
                            size,
                            ..
                        } => {
                            ui::render_delete_modal(f, item_name, *is_dir, *size, is_si);
                        }
                        ModalState::Info { entry, full_path } => {
                            ui::render_info_modal(f, entry, full_path, is_si);
                        }
                    }
                })?;

                if !event::poll(Duration::from_millis(50))? {
                    continue;
                }

                let Event::Key(key) = event::read()? else {
                    continue;
                };

                if key.kind != KeyEventKind::Press {
                    continue;
                }

                // Modal key handling takes precedence
                match modal {
                    ModalState::Help => {
                        *modal = ModalState::None;
                        continue;
                    }
                    ModalState::Info { .. } => {
                        match key.code {
                            KeyCode::Enter
                            | KeyCode::Esc
                            | KeyCode::Char('i')
                            | KeyCode::Char('q') => {
                                *modal = ModalState::None;
                            }
                            _ => {}
                        }
                        continue;
                    }
                    ModalState::DeleteConfirm { target_path, .. } => {
                        match key.code {
                            KeyCode::Char('s')
                            | KeyCode::Char('S')
                            | KeyCode::Char('y')
                            | KeyCode::Char('Y') => {
                                let path_to_del = target_path.clone();
                                *modal = ModalState::None;
                                if let Err(e) = delete_entry(&path_to_del) {
                                    tracing::error!("Falha ao excluir {path_to_del:?}: {e}");
                                } else {
                                    browser.remove_selected();
                                }
                            }
                            KeyCode::Char('n')
                            | KeyCode::Char('N')
                            | KeyCode::Esc
                            | KeyCode::Char('q') => {
                                *modal = ModalState::None;
                            }
                            _ => {}
                        }
                        continue;
                    }
                    ModalState::None => {}
                }

                // Main navigation key handling
                match key.code {
                    // Navigation
                    KeyCode::Up | KeyCode::Char('k') => browser.move_up(),
                    KeyCode::Down | KeyCode::Char('j') => {
                        browser.move_down(max_visible_rows);
                    }
                    KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => {
                        browser.enter_selected();
                    }
                    KeyCode::Backspace | KeyCode::Left | KeyCode::Char('h') => {
                        browser.go_back();
                    }
                    KeyCode::PageUp => browser.page_up(max_visible_rows),
                    KeyCode::PageDown => browser.page_down(max_visible_rows, max_visible_rows),
                    KeyCode::Home | KeyCode::Char('g') => browser.home(),
                    KeyCode::End | KeyCode::Char('G') => browser.end(max_visible_rows),

                    // Sorting & Column toggles
                    KeyCode::Char('s') => browser.sort_by(SortColumn::Size),
                    KeyCode::Char('n') => browser.sort_by(SortColumn::Name),
                    KeyCode::Char('c') => browser.toggle_items(),
                    KeyCode::Char('C') => browser.sort_by(SortColumn::Items),
                    KeyCode::Char('m') => browser.toggle_mtime(),
                    KeyCode::Char('M') => browser.sort_by(SortColumn::Mtime),
                    KeyCode::Char('a') => {
                        if browser.sort_col == SortColumn::Size {
                            browser.sort_by(SortColumn::DiskSize);
                        } else {
                            browser.sort_by(SortColumn::Size);
                        }
                    }
                    KeyCode::Char('t') => browser.toggle_dirs_first(),

                    // View toggles
                    KeyCode::Char('e') => browser.toggle_hidden(),
                    KeyCode::Char('p') => browser.toggle_percent(),
                    KeyCode::Char('u') | KeyCode::Char('v') => browser.toggle_graph(),
                    KeyCode::Char('b') => browser.toggle_graph_style(),

                    // Modals
                    KeyCode::Char('?') => {
                        *modal = ModalState::Help;
                    }
                    KeyCode::Char('i') => {
                        if let Some(entry) = browser.selected_entry() {
                            let full_path = browser
                                .tree
                                .get_path(browser.selected_entry_index().unwrap_or(0));
                            *modal = ModalState::Info {
                                entry: entry.clone(),
                                full_path,
                            };
                        }
                    }
                    KeyCode::Char('d') => {
                        if let (Some(entry), Some(idx)) =
                            (browser.selected_entry(), browser.selected_entry_index())
                        {
                            let full_path = browser.tree.get_path(idx);
                            *modal = ModalState::DeleteConfirm {
                                item_name: entry.name.to_string(),
                                is_dir: entry.is_dir(),
                                size: entry.size,
                                target_path: full_path,
                            };
                        }
                    }

                    // Quit
                    KeyCode::Char('q') => return Ok(()),
                    _ if key.modifiers.contains(KeyModifiers::CONTROL)
                        && key.code == KeyCode::Char('c') =>
                    {
                        return Ok(());
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Sets up a panic hook to properly clean up the terminal before spewing out panic traces.
fn setup_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        default_hook(panic_info);
    }));
}
