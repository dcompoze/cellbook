use std::collections::HashMap;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::{io, process, thread};

use anyhow::anyhow;
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent};
use crossterm::style::ResetColor;
use crossterm::terminal::{
    disable_raw_mode,
    enable_raw_mode,
    Clear,
    ClearType,
    EnterAlternateScreen,
    LeaveAlternateScreen,
};
use crossterm::{cursor, execute};
use duckdb::polars::frame::DataFrame;
use duckdb::Connection;
use parking_lot::Mutex;
use ratatui::layout::{Constraint, Direction, Layout, Position};
use ratatui::prelude::CrosstermBackend;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Terminal;
use signal_hook::consts::{SIGINT, SIGTERM};
use signal_hook::iterator::Signals;

mod errors;

use errors::Result;

#[derive(Clone, Debug)]
pub struct Context {
    pub outputs: Arc<Mutex<Vec<String>>>,
    pub values: Arc<HashMap<String, i64>>,
    pub frames: Arc<HashMap<String, DataFrame>>,
    pub database: Arc<Mutex<Connection>>,
}

impl Context {
    pub fn new(path: &str) -> Result<Context> {
        Ok(Context {
            outputs: Arc::new(Mutex::new(Vec::new())),
            values: Arc::new(HashMap::new()),
            frames: Arc::new(HashMap::new()),
            database: Arc::new(Mutex::new(Connection::open(path)?)),
        })
    }
}

type Cell = fn(Context) -> anyhow::Result<()>;

#[derive(Clone, Debug)]
pub struct Notebook {
    pub ctx: Context,
    cells: Vec<Cell>,
}

impl Notebook {
    pub fn new(ctx: Context) -> Result<Notebook> {
        Ok(Notebook {
            ctx,
            cells: Vec::new(),
        })
    }

    pub fn include(&mut self, cell: Cell) -> Result<()> {
        self.cells.push(cell);
        self.ctx.outputs.lock().push(String::new());
        Ok(())
    }

    pub fn execute(&mut self) -> Result<()> {
        let mut signals = Signals::new([SIGTERM, SIGINT])?;

        thread::spawn(move || -> Result<()> {
            let _ = signals.forever().next();
            disable_raw_mode()?;
            let stdout = io::stdout();
            let mut backend = CrosstermBackend::new(stdout);

            execute!(backend, ResetColor, LeaveAlternateScreen)?;
            process::exit(0);
        });

        enable_raw_mode()?;
        let stdout = io::stdout();
        let mut backend = CrosstermBackend::new(stdout);

        execute!(backend, Clear(ClearType::All), EnterAlternateScreen)?;

        let mut terminal = Terminal::new(backend)?;

        let mut input = String::new();
        let mut error_message = String::new();

        let mut cursor_position = Position {
            x: 0,
            y: terminal.size()?.height - 1,
        };

        loop {
            terminal.draw(|f| {
                f.set_cursor_position(cursor_position);

                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Fill(1), Constraint::Length(1)])
                    .split(f.area());

                let menu_items = vec![
                    ListItem::new(Line::from(vec![
                        Span::styled("[0]", Style::default().fg(Color::Cyan)),
                        Span::raw(" cell0"),
                    ])),
                    ListItem::new(Line::from(vec![
                        Span::styled("[1]", Style::default().fg(Color::Cyan)),
                        Span::raw(" cell1"),
                    ])),
                    ListItem::new(Line::from(vec![
                        Span::styled("[2]", Style::default().fg(Color::Cyan)),
                        Span::raw(" cell2"),
                    ])),
                    ListItem::new(Line::from(vec![
                        Span::styled("[3]", Style::default().fg(Color::Cyan)),
                        Span::raw(" cell3"),
                    ])),
                    ListItem::new(Line::from(vec![
                        Span::styled("[4]", Style::default().fg(Color::Cyan)),
                        Span::raw(" cell4"),
                    ])),
                    ListItem::new(Line::from(vec![
                        Span::styled("[5]", Style::default().fg(Color::Cyan)),
                        Span::raw(" cell5"),
                    ])),
                    ListItem::new(Line::from(vec![
                        Span::styled("[o]", Style::default().fg(Color::Cyan)),
                        Span::raw(" output"),
                    ])),
                ];

                let menu = List::new(menu_items).block(Block::default());
                f.render_widget(menu, chunks[0]);

                let input_block = Paragraph::new(input.clone()).block(Block::default());
                f.render_widget(input_block, chunks[1]);
            })?;

            if let Event::Key(KeyEvent { code, .. }) = event::read()? {
                match code {
                    KeyCode::Char(c) => {
                        input.push(c);
                        cursor_position.x = (cursor_position.x + 1).min(terminal.size()?.width);
                    }
                    KeyCode::Enter => {
                        error_message.clear();
                        cursor_position.x = 0;

                        match input.as_str() {
                            "0" => {
                                self.cells[0](self.ctx.clone())?;
                            }
                            "1" => {
                                self.cells[1](self.ctx.clone())?;
                            }
                            "2" => {
                                self.cells[2](self.ctx.clone())?;
                            }
                            "3" => {
                                self.cells[3](self.ctx.clone())?;
                            }
                            "4" => {
                                self.cells[4](self.ctx.clone())?;
                            }
                            "5" => {
                                self.cells[5](self.ctx.clone())?;
                            }
                            "o" => {
                                disable_raw_mode()?;
                                execute!(terminal.backend_mut(), ResetColor, LeaveAlternateScreen)?;
                                let mut child = Command::new("less").stdin(Stdio::piped()).spawn()?;
                                {
                                    let stdin = child.stdin.as_mut().ok_or(anyhow!("no child stdin"))?;
                                    stdin.write_all(self.ctx.outputs.lock()[0].as_bytes())?;
                                }
                                child.wait()?;
                                enable_raw_mode()?;
                                execute!(terminal.backend_mut(), EnterAlternateScreen)?;
                                terminal.clear()?;
                            }
                            _ => {
                                error_message = format!("Invalid option: {}", input);
                            }
                        }
                        input.clear();
                    }
                    KeyCode::Backspace => {
                        input.pop();
                        cursor_position.x = cursor_position.x.saturating_sub(1);
                    }
                    KeyCode::Esc => {
                        break;
                    }
                    _ => {}
                }
            }
        }

        disable_raw_mode()?;
        execute!(terminal.backend_mut(), ResetColor, LeaveAlternateScreen)?;
        process::exit(0);
    }
}

#[macro_export]
macro_rules! output {
    ($dst:expr, $($arg:tt)*) => {
        {
            $dst.clear();
            writeln!($dst, $($arg)*)
        }
    };
}
