//! Terminal User Interface rendering using Ratatui.
//!
//! Provides pixel-perfect visual parity with ncdu, localized in Brazilian Portuguese (pt-BR):
//! - Real-time progress screen during scanning
//! - Interactive browser with proportional bar charts, alignment, and color highlights
//! - Contextual modals for Help, File Deletion Confirmation, and Item Information

use crate::browser::{BrowserState, SortColumn, SortOrder};
use crate::model::{DirEntry, EntryType};
use crate::scanner::ScanProgress;
use crate::util::{
    format_count, format_graph, format_mtime, format_percent, format_size, truncate_path,
};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Cell, Clear, Paragraph, Row, Table};
use std::path::Path;

/// Renders the scanning screen while filesystem walk is in progress.
pub fn render_scan_screen(frame: &mut Frame, progress: &ScanProgress, is_si: bool) {
    let size = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Top banner
            Constraint::Min(8),    // Center card
            Constraint::Length(1), // Bottom hints
        ])
        .split(size);

    // Header
    let header_text = Line::from(vec![
        Span::styled(
            " wncdu ",
            Style::default().bg(Color::Cyan).fg(Color::Black).bold(),
        ),
        Span::raw(" Escaneando diretório..."),
    ]);
    frame.render_widget(Paragraph::new(header_text), chunks[0]);

    // Center Card
    let card_area = centered_rect(70, 50, chunks[1]);
    let block = Block::default()
        .title(" Escaneamento de Disco em Andamento ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan));

    let inner_area = block.inner(card_area);
    frame.render_widget(block, card_area);

    let info_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Current directory
            Constraint::Length(1), // Spacer
            Constraint::Length(1), // Items found
            Constraint::Length(1), // Bytes found
            Constraint::Length(1), // Dirs scanned
            Constraint::Length(1), // Errors
            Constraint::Min(1),    // Spacer
            Constraint::Length(1), // Spinner/activity
        ])
        .split(inner_area);

    let curr_path_truncated = truncate_path(
        &progress.current_path,
        inner_area.width.saturating_sub(4) as usize,
    );
    let path_line = Line::from(vec![
        Span::styled("Caminho: ", Style::default().fg(Color::Yellow).bold()),
        Span::raw(curr_path_truncated),
    ]);
    frame.render_widget(Paragraph::new(path_line), info_chunks[0]);

    let items_line = Line::from(vec![
        Span::styled(
            "Total de itens:  ",
            Style::default().fg(Color::White).bold(),
        ),
        Span::styled(
            format_count(progress.items_scanned),
            Style::default().fg(Color::Green),
        ),
    ]);
    frame.render_widget(Paragraph::new(items_line), info_chunks[2]);

    let bytes_line = Line::from(vec![
        Span::styled(
            "Tamanho total:   ",
            Style::default().fg(Color::White).bold(),
        ),
        Span::styled(
            format_size(progress.bytes_scanned, is_si),
            Style::default().fg(Color::Green),
        ),
    ]);
    frame.render_widget(Paragraph::new(bytes_line), info_chunks[3]);

    let dirs_line = Line::from(vec![
        Span::styled(
            "Pastas lidas:    ",
            Style::default().fg(Color::White).bold(),
        ),
        Span::raw(format_count(progress.dirs_scanned)),
    ]);
    frame.render_widget(Paragraph::new(dirs_line), info_chunks[4]);

    let err_color = if progress.errors > 0 {
        Color::Red
    } else {
        Color::DarkGray
    };
    let errors_line = Line::from(vec![
        Span::styled(
            "Erros de leitura:",
            Style::default().fg(Color::White).bold(),
        ),
        Span::styled(
            format_count(progress.errors),
            Style::default().fg(err_color),
        ),
    ]);
    frame.render_widget(Paragraph::new(errors_line), info_chunks[5]);

    let hint_line = Line::from(vec![
        Span::styled(
            " [q] ",
            Style::default().fg(Color::Black).bg(Color::Yellow).bold(),
        ),
        Span::raw(" Cancelar e inspecionar resultados parciais"),
    ]);
    frame.render_widget(
        Paragraph::new(hint_line).alignment(Alignment::Center),
        info_chunks[7],
    );

    // Bottom footer
    let bottom_text = Line::from(vec![Span::raw(
        " Escaneando em segundo plano com API Win32 nativa de alto desempenho...",
    )]);
    frame.render_widget(
        Paragraph::new(bottom_text).style(Style::default().fg(Color::DarkGray)),
        chunks[2],
    );
}

/// Renders the main interactive browser view.
pub fn render_browser_screen(frame: &mut Frame, state: &BrowserState) {
    let size = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Top title line
            Constraint::Length(1), // Breadcrumb path line
            Constraint::Min(3),    // File list table
            Constraint::Length(1), // Stats footer
            Constraint::Length(1), // Keybinds footer
        ])
        .split(size);

    // 1. Top Title Bar
    let current_dir_entry = &state.tree.entries[state.current_dir];
    let total_cur_size = format_size(
        if state.sort_col == SortColumn::DiskSize {
            current_dir_entry.disk_size
        } else {
            current_dir_entry.size
        },
        state.use_si_units,
    );

    let title_line = Line::from(vec![
        Span::styled(
            " wncdu 0.1.0 ",
            Style::default().bg(Color::Cyan).fg(Color::Black).bold(),
        ),
        Span::raw(" ~ "),
        Span::styled(
            truncate_path(&state.tree.root_path.to_string_lossy(), 40),
            Style::default().fg(Color::White),
        ),
        Span::raw(" "),
        Span::styled(
            format!("[{total_cur_size}]"),
            Style::default().fg(Color::Green).bold(),
        ),
    ]);
    frame.render_widget(Paragraph::new(title_line), chunks[0]);

    // 2. Breadcrumb Path Bar
    let full_cur_path = state.tree.get_path(state.current_dir);
    let path_str = full_cur_path.to_string_lossy();
    let breadcrumb = Line::from(vec![
        Span::styled("--- ", Style::default().fg(Color::DarkGray)),
        Span::styled(path_str.as_ref(), Style::default().fg(Color::Yellow).bold()),
        Span::styled(" ---", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(breadcrumb), chunks[1]);

    // 3. File Table
    render_file_table(frame, state, chunks[2]);

    // 4. Summary Stats Footer
    let root_entry = &state.tree.entries[0];
    let summary_line = Line::from(vec![
        Span::styled(" Uso em disco: ", Style::default().bold()),
        Span::styled(
            format_size(root_entry.disk_size, state.use_si_units),
            Style::default().fg(Color::Green),
        ),
        Span::styled("   Tamanho aparente: ", Style::default().bold()),
        Span::styled(
            format_size(root_entry.size, state.use_si_units),
            Style::default().fg(Color::Green),
        ),
        Span::styled("   Itens: ", Style::default().bold()),
        Span::styled(
            format_count(root_entry.items),
            Style::default().fg(Color::Cyan),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(summary_line).style(Style::default().bg(Color::DarkGray).fg(Color::White)),
        chunks[3],
    );

    // 5. Help Keybinds Footer
    let binds_line = Line::from(vec![
        Span::styled(
            " ? ",
            Style::default().bg(Color::Yellow).fg(Color::Black).bold(),
        ),
        Span::raw("Ajuda "),
        Span::styled(
            " Enter ",
            Style::default().bg(Color::White).fg(Color::Black),
        ),
        Span::raw("Abrir "),
        Span::styled(
            " Backspace ",
            Style::default().bg(Color::White).fg(Color::Black),
        ),
        Span::raw("Voltar "),
        Span::styled(
            " d ",
            Style::default().bg(Color::Red).fg(Color::White).bold(),
        ),
        Span::raw("Excluir "),
        Span::styled(" s ", Style::default().bg(Color::White).fg(Color::Black)),
        Span::raw("Ordenar "),
        Span::styled(" i ", Style::default().bg(Color::White).fg(Color::Black)),
        Span::raw("Info "),
        Span::styled(" q ", Style::default().bg(Color::White).fg(Color::Black)),
        Span::raw("Sair"),
    ]);
    frame.render_widget(Paragraph::new(binds_line), chunks[4]);
}

fn render_file_table(frame: &mut Frame, state: &BrowserState, area: Rect) {
    if area.height <= 1 {
        return;
    }

    let max_visible_rows = area.height.saturating_sub(1) as usize; // Sub 1 for header
    let parent_size = state.tree.entries[state.current_dir].size.max(1);

    // Determine max child size for relative graph scaling
    let max_child_size = state
        .cached_children
        .iter()
        .map(|&idx| state.tree.entries[idx].size)
        .max()
        .unwrap_or(1)
        .max(1);

    // Build Table Header
    let mut header_cells = vec![Cell::from(" ")]; // Cursor column

    let sort_indicator = match state.sort_order {
        SortOrder::Ascending => "▲",
        SortOrder::Descending => "▼",
    };

    let size_header = if state.sort_col == SortColumn::DiskSize {
        format!("UsoDisco {sort_indicator}")
    } else if state.sort_col == SortColumn::Size {
        format!("Tamanho {sort_indicator}")
    } else {
        "Tamanho".to_string()
    };
    header_cells.push(Cell::from(size_header));

    if state.show_graph {
        header_cells.push(Cell::from("Gráfico"));
    }

    if state.show_percent {
        header_cells.push(Cell::from("Percentual"));
    }

    if state.show_items {
        let items_header = if state.sort_col == SortColumn::Items {
            format!("Itens {sort_indicator}")
        } else {
            "Itens".to_string()
        };
        header_cells.push(Cell::from(items_header));
    }

    if state.show_mtime {
        let mtime_header = if state.sort_col == SortColumn::Mtime {
            format!("Modificado {sort_indicator}")
        } else {
            "Modificado".to_string()
        };
        header_cells.push(Cell::from(mtime_header));
    }

    let name_header = if state.sort_col == SortColumn::Name {
        format!("Nome {sort_indicator}")
    } else {
        "Nome".to_string()
    };
    header_cells.push(Cell::from(name_header));

    let header_row = Row::new(header_cells)
        .style(Style::default().fg(Color::Yellow).bold())
        .bottom_margin(0);

    // Build Rows
    let visible_indices = state
        .cached_children
        .iter()
        .skip(state.scroll_offset)
        .take(max_visible_rows)
        .enumerate();

    let mut rows = Vec::new();

    for (rel_i, &child_idx) in visible_indices {
        let abs_i = state.scroll_offset + rel_i;
        let is_selected = abs_i == state.selected_index;
        let entry = &state.tree.entries[child_idx];

        let mut cells = Vec::new();

        // Cursor
        let cursor = if is_selected { "▶" } else { " " };
        cells.push(Cell::from(cursor));

        // Size
        let entry_size = if state.sort_col == SortColumn::DiskSize {
            entry.disk_size
        } else {
            entry.size
        };
        let formatted_size = format!("{:>10}", format_size(entry_size, state.use_si_units));
        cells.push(Cell::from(formatted_size));

        // Graph
        if state.show_graph {
            let frac = (entry.size as f64) / (max_child_size as f64);
            let graph_str = format_graph(frac, 12, state.graph_style);
            cells.push(Cell::from(graph_str));
        }

        // Percent
        if state.show_percent {
            let pct_str = format_percent(entry.size, parent_size);
            cells.push(Cell::from(pct_str));
        }

        // Items
        if state.show_items {
            let items_str = format!("{:>8}", format_count(entry.items));
            cells.push(Cell::from(items_str));
        }

        // Mtime
        if state.show_mtime {
            let mtime_str = format_mtime(entry.mtime);
            cells.push(Cell::from(mtime_str));
        }

        // Name + Prefix
        let prefix = match entry.entry_type {
            EntryType::Directory => "/",
            EntryType::Symlink => "@",
            EntryType::File | EntryType::Other => " ",
        };

        let err_tag = if entry.has_error { "!" } else { " " };
        let full_name = format!("{err_tag}{prefix}{}", entry.name);

        let name_cell = if entry.is_dir() {
            Cell::from(full_name).style(Style::default().fg(Color::Cyan).bold())
        } else if entry.entry_type == EntryType::Symlink {
            Cell::from(full_name).style(Style::default().fg(Color::Magenta))
        } else {
            Cell::from(full_name)
        };
        cells.push(name_cell);

        let row_style = if is_selected {
            Style::default()
                .bg(Color::Blue)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else if entry.is_hidden {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default()
        };

        rows.push(Row::new(cells).style(row_style));
    }

    // Column constraints
    let mut constraints = vec![Constraint::Length(1)]; // Cursor
    constraints.push(Constraint::Length(12)); // Size

    if state.show_graph {
        constraints.push(Constraint::Length(14)); // Graph
    }
    if state.show_percent {
        constraints.push(Constraint::Length(11)); // Percent
    }
    if state.show_items {
        constraints.push(Constraint::Length(10)); // Items
    }
    if state.show_mtime {
        constraints.push(Constraint::Length(18)); // Mtime
    }
    constraints.push(Constraint::Min(20)); // Name

    let table = Table::new(rows, constraints)
        .header(header_row)
        .block(Block::default());

    frame.render_widget(table, area);
}

/// Renders the help modal popup dialog in Brazilian Portuguese.
pub fn render_help_modal(frame: &mut Frame) {
    let area = centered_rect(65, 78, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" Ajuda do WNCDU - Teclas de Atalho ")
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(Color::Yellow));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let text = vec![
        Line::from(vec![Span::styled(
            "Navegação:",
            Style::default().fg(Color::Cyan).bold(),
        )]),
        Line::from("  ↑, k            Mover cursor para cima"),
        Line::from("  ↓, j            Mover cursor para baixo"),
        Line::from("  enter, →, l     Entrar no diretório selecionado"),
        Line::from("  backspace, ←, h Subir para o diretório pai"),
        Line::from("  home, g         Ir para o primeiro item da lista"),
        Line::from("  end, G          Ir para o último item da lista"),
        Line::from("  pgup, pgdn      Rolar uma página para cima / baixo"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Ordenação:",
            Style::default().fg(Color::Cyan).bold(),
        )]),
        Line::from("  s               Ordenar por tamanho (alterna cresc/decresc)"),
        Line::from("  n               Ordenar por nome (alterna A-Z / Z-A)"),
        Line::from("  C               Ordenar por quantidade de itens"),
        Line::from("  M               Ordenar por data de modificação"),
        Line::from("  a               Alternar tamanho aparente vs uso em disco"),
        Line::from("  t               Alternar pastas agrupadas no topo"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Exibição e Colunas:",
            Style::default().fg(Color::Cyan).bold(),
        )]),
        Line::from("  c               Exibir / ocultar coluna de contagem de itens"),
        Line::from("  m               Exibir / ocultar coluna de data de modificação"),
        Line::from("  g               Alternar estilo do gráfico (# vs blocos Unicode)"),
        Line::from("  p               Exibir / ocultar coluna de percentual"),
        Line::from("  e               Exibir / ocultar arquivos e pastas ocultos"),
        Line::from("  i               Exibir detalhes e propriedades do item"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Ações:",
            Style::default().fg(Color::Cyan).bold(),
        )]),
        Line::from("  d               Excluir item selecionado (com confirmação)"),
        Line::from("  ?               Abrir / fechar esta janela de ajuda"),
        Line::from("  q               Sair do programa"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Pressione qualquer tecla para fechar esta janela de ajuda",
            Style::default().fg(Color::Yellow),
        )]),
    ];

    let paragraph = Paragraph::new(text);
    frame.render_widget(paragraph, inner);
}

/// Renders the file/directory delete confirmation dialog in Brazilian Portuguese.
pub fn render_delete_modal(
    frame: &mut Frame,
    item_name: &str,
    is_dir: bool,
    size: u64,
    is_si: bool,
) {
    let area = centered_rect(58, 30, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" Confirmar Exclusão Permanente ")
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(Color::Red).bold());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let item_type = if is_dir {
        "este diretório e todo o seu conteúdo"
    } else {
        "este arquivo"
    };
    let text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("ATENÇÃO: ", Style::default().fg(Color::Red).bold()),
            Span::raw(format!("Tem certeza que deseja excluir {item_type}?")),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" Alvo:    ", Style::default().fg(Color::White).bold()),
            Span::styled(item_name, Style::default().fg(Color::Yellow).bold()),
        ]),
        Line::from(vec![
            Span::styled(" Tamanho: ", Style::default().fg(Color::White).bold()),
            Span::styled(format_size(size, is_si), Style::default().fg(Color::Green)),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Esta ação é definitiva e NÃO PODERÁ ser desfeita!",
            Style::default().fg(Color::Red),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                " [s / y] ",
                Style::default().bg(Color::Red).fg(Color::White).bold(),
            ),
            Span::raw(" Sim, Excluir Definitivamente    "),
            Span::styled(
                " [n / Esc] ",
                Style::default().bg(Color::Green).fg(Color::Black).bold(),
            ),
            Span::raw(" Cancelar"),
        ]),
    ];

    let paragraph = Paragraph::new(text).alignment(Alignment::Center);
    frame.render_widget(paragraph, inner);
}

/// Renders the detailed item info dialog in Brazilian Portuguese.
pub fn render_info_modal(frame: &mut Frame, entry: &DirEntry, full_path: &Path, is_si: bool) {
    let area = centered_rect(65, 45, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" Detalhes do Item ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let type_str = match entry.entry_type {
        EntryType::Directory => "Diretório / Pasta",
        EntryType::File => "Arquivo Comum",
        EntryType::Symlink => "Link Simbólico / Junção",
        EntryType::Other => "Outro / Dispositivo",
    };

    let text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(" Nome:              ", Style::default().bold()),
            Span::styled(entry.name.as_str(), Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::styled(" Caminho Completo:  ", Style::default().bold()),
            Span::raw(full_path.to_string_lossy()),
        ]),
        Line::from(vec![
            Span::styled(" Tipo:              ", Style::default().bold()),
            Span::styled(type_str, Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled(" Tamanho Aparente:  ", Style::default().bold()),
            Span::styled(
                format!(
                    "{} ({} bytes)",
                    format_size(entry.size, is_si),
                    format_count(entry.size)
                ),
                Style::default().fg(Color::Green),
            ),
        ]),
        Line::from(vec![
            Span::styled(" Uso em Disco:      ", Style::default().bold()),
            Span::styled(
                format!(
                    "{} ({} bytes)",
                    format_size(entry.disk_size, is_si),
                    format_count(entry.disk_size)
                ),
                Style::default().fg(Color::Green),
            ),
        ]),
        Line::from(vec![
            Span::styled(" Total de Itens:    ", Style::default().bold()),
            Span::styled(format_count(entry.items), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled(" Última Modificação:", Style::default().bold()),
            Span::raw(format_mtime(entry.mtime)),
        ]),
        Line::from(vec![
            Span::styled(" Arquivo Oculto:    ", Style::default().bold()),
            Span::raw(if entry.is_hidden { "Sim" } else { "Não" }),
        ]),
        Line::from(vec![
            Span::styled(" Possui Erros:      ", Style::default().bold()),
            Span::styled(
                if entry.has_error {
                    "Sim (Acesso Negado ou Ilegível)"
                } else {
                    "Não"
                },
                if entry.has_error {
                    Style::default().fg(Color::Red)
                } else {
                    Style::default().fg(Color::Green)
                },
            ),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Pressione [Enter], [i] ou [Esc] para fechar",
            Style::default().fg(Color::DarkGray),
        )]),
    ];

    let paragraph = Paragraph::new(text);
    frame.render_widget(paragraph, inner);
}

/// Helper to create a centered rectangle of a given percentage width and height.
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
