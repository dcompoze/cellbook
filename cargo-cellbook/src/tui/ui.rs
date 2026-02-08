//! TUI rendering.

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};

use super::state::{App, BuildStatus, CellStatus};

/// Render the entire UI.
pub fn render(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Ratio(2, 3), // Cells
            Constraint::Ratio(1, 3), // Context
            Constraint::Length(1),   // Status bar
        ])
        .split(frame.area());

    render_cells(frame, app, chunks[0]);
    render_context(frame, app, chunks[1]);
    render_status_bar(frame, app, chunks[2]);
}

fn render_cells(frame: &mut Frame, app: &mut App, area: Rect) {
    let inner_width = area.width as usize;

    let items: Vec<ListItem> = app
        .cells
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let cell_num = format!("[{}] ", i + 1);

            // Count indicator.
            let count = app.get_count(name);
            let count_span = if count == 0 {
                Span::styled(format!("[{}]", count), Style::default().fg(Color::DarkGray))
            } else {
                Span::styled(format!("[{}]", count), Style::default().fg(Color::Yellow))
            };

            // Output indicator.
            let output_span = if app.has_output(name) {
                Span::styled("[output]", Style::default().fg(Color::Blue))
            } else {
                Span::styled("[none]", Style::default().fg(Color::DarkGray))
            };

            // Status indicator.
            let status_span = match &app.cell_statuses[i] {
                CellStatus::Pending => Span::styled("[none]", Style::default().fg(Color::DarkGray)),
                CellStatus::Running => Span::styled(
                    "[running]",
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                ),
                CellStatus::Success => Span::styled("[success]", Style::default().fg(Color::Green)),
                CellStatus::Error(_) => Span::styled("[error]", Style::default().fg(Color::Red)),
            };

            // Calculate right side width.
            let count_text = format!("[{}]", count);
            let output_text = if app.has_output(name) {
                "[output]"
            } else {
                "[none]"
            };
            let status_text = match &app.cell_statuses[i] {
                CellStatus::Pending => "[none]",
                CellStatus::Running => "[running]",
                CellStatus::Success => "[success]",
                CellStatus::Error(_) => "[error]",
            };
            let right_len = count_text.len() + 1 + output_text.len() + 1 + status_text.len();
            let left_len = cell_num.len();

            let name_max_len = inner_width.saturating_sub(right_len + left_len + 1);
            let display_name: String = name.chars().take(name_max_len).collect();
            let padding = inner_width.saturating_sub(left_len + display_name.len() + right_len);

            let line = Line::from(vec![
                Span::styled(cell_num, Style::default().fg(Color::DarkGray)),
                Span::raw(display_name),
                Span::raw(" ".repeat(padding)),
                count_span,
                Span::raw(" "),
                output_span,
                Span::raw(" "),
                status_span,
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(Color::White))
                .title("Cells "),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(35, 37, 42))
                .add_modifier(Modifier::BOLD),
        );

    frame.render_stateful_widget(list, area, &mut app.list_state);
}

fn render_context(frame: &mut Frame, app: &App, area: Rect) {
    let items: Vec<Span> = if app.context_items.is_empty() {
        vec![]
    } else {
        app.context_items
            .iter()
            .flat_map(|(key, type_name)| {
                vec![
                    Span::styled(key, Style::default().fg(Color::Cyan)),
                    Span::raw(": "),
                    Span::styled(type_name, Style::default().fg(Color::Yellow)),
                    Span::raw("  "),
                ]
            })
            .collect()
    };

    let context = Paragraph::new(Line::from(items))
        .block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(Color::White))
                .title("Store "),
        )
        .wrap(Wrap { trim: true });

    frame.render_widget(context, area);
}

fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let help = vec![
        Span::styled("[Enter]", Style::default().fg(Color::Cyan)),
        Span::raw(" Run  "),
        Span::styled("[o]", Style::default().fg(Color::Cyan)),
        Span::raw(" Output  "),
        Span::styled("[e]", Style::default().fg(Color::Cyan)),
        Span::raw(" Error  "),
        Span::styled("[E]", Style::default().fg(Color::Cyan)),
        Span::raw(" Edit  "),
        Span::styled("[x]", Style::default().fg(Color::Cyan)),
        Span::raw(" Clear  "),
        Span::styled("[r]", Style::default().fg(Color::Cyan)),
        Span::raw(" Reload  "),
        Span::styled("[q]", Style::default().fg(Color::Cyan)),
        Span::raw(" Quit  "),
    ];

    let help_width: usize = help.iter().map(|s| s.width()).sum();

    let status = match &app.build_status {
        BuildStatus::Idle => Span::styled("Ready", Style::default().fg(Color::Green)),
        BuildStatus::Building => Span::styled(
            "Building",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ),
        BuildStatus::Reloading => Span::styled("Reloading", Style::default().fg(Color::Cyan)),
        BuildStatus::BuildError(_) => Span::styled(
            "Build Error",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
    };

    let cell_count = Span::styled(
        format!(" [{} cells]", app.cells.len()),
        Style::default().fg(Color::DarkGray),
    );

    let bar_style = Style::default().bg(Color::Rgb(35, 37, 42));

    // Left side: help keys.
    let left = Paragraph::new(Line::from(help)).style(bar_style);

    // Right side: status and cell count.
    let right = Paragraph::new(Line::from(vec![status, cell_count]))
        .alignment(Alignment::Right)
        .style(bar_style);

    // Prioritize commands over status when space is limited.
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(help_width as u16), Constraint::Fill(1)])
        .split(area);

    frame.render_widget(left, chunks[0]);
    frame.render_widget(right, chunks[1]);
}
