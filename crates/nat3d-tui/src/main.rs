/*
 * NAT3D - Next-generation Advanced Technology for 3D
 * Professional 3D Modeling, CAD, Physics Simulation and Rendering Suite
 * 
 * Copyright (C) 2023-2026 Francisco Molina <pako.molina@gmail.com>
 * 
 * This software is dual-licensed:
 * 1. Open Source: GNU Affero General Public License v3.0 or later (AGPL-3.0-or-later)
 * 2. Commercial: For commercial use, please contact <fmolina@avermex.com>
 * 
 * For research information, visit: https://research.avermex.com
 * For collaborations, contact: <pako.molina@gmail.com>
 * 
 * DOI: [PENDING]
 */

// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Francisco Molina-Burgos, Avermex Research Division

//! NAT3D TUI - Interactive Terminal User Interface
//!
//! This provides a menu-driven interface for NAT3D that stays open
//! and allows users to navigate commands with arrow keys.

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io;
use std::process::Command as ProcessCommand;
use std::env;

/// Main application state
struct App {
    /// Selected menu item
    selected: usize,
    /// Available commands
    commands: Vec<Command>,
    /// Status message
    status: String,
    /// Whether to quit
    should_quit: bool,
}

/// A NAT3D command
#[derive(Clone)]
struct Command {
    name: &'static str,
    description: &'static str,
    action: CommandAction,
}

#[derive(Clone, Copy)]
enum CommandAction {
    Generate,
    Convert,
    Batch,
    Info,
    Validate,
    Render,
    Script,
    Benchmark,
}

impl App {
    fn new() -> Self {
        Self {
            selected: 0,
            commands: vec![
                Command {
                    name: "Generate Primitive",
                    description: "Create geometric primitives (cube, sphere, etc.)",
                    action: CommandAction::Generate,
                },
                Command {
                    name: "Convert File",
                    description: "Convert between 3D file formats (OBJ, STL, glTF, etc.)",
                    action: CommandAction::Convert,
                },
                Command {
                    name: "Batch Process",
                    description: "Process multiple files (optimize, preview, validate)",
                    action: CommandAction::Batch,
                },
                Command {
                    name: "File Info",
                    description: "Analyze 3D file and show statistics",
                    action: CommandAction::Info,
                },
                Command {
                    name: "Validate File",
                    description: "Check file integrity and auto-fix issues",
                    action: CommandAction::Validate,
                },
                Command {
                    name: "Render Scene",
                    description: "Render scene to image (CPU raytracer)",
                    action: CommandAction::Render,
                },
                Command {
                    name: "Run Script",
                    description: "Execute Python script",
                    action: CommandAction::Script,
                },
                Command {
                    name: "Benchmark",
                    description: "Run performance benchmarks",
                    action: CommandAction::Benchmark,
                },
            ],
            status: "Use ↑↓ to navigate, Enter to select, Q to quit".to_string(),
            should_quit: false,
        }
    }

    fn next(&mut self) {
        self.selected = (self.selected + 1) % self.commands.len();
    }

    fn previous(&mut self) {
        if self.selected == 0 {
            self.selected = self.commands.len() - 1;
        } else {
            self.selected -= 1;
        }
    }

    fn execute_command(&mut self) {
        let cmd = &self.commands[self.selected];

        // Find nat3d CLI binary (should be in same directory as TUI or in PATH)
        let cli_path = env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|parent| parent.join("nat3d.exe")))
            .unwrap_or_else(|| "nat3d.exe".into());

        // Execute command based on action type (BATCH 24: TUI command wiring)
        let result = match cmd.action {
            CommandAction::Generate => {
                // nat3d generate cube --output cube.obj
                ProcessCommand::new(&cli_path)
                    .args(["generate", "cube", "--output", "cube.obj"])
                    .output()
            },
            CommandAction::Convert => {
                // nat3d convert input.obj output.stl
                self.status = "Convert: Specify input/output files via CLI directly".to_string();
                return;
            },
            CommandAction::Batch => {
                // nat3d batch *.obj --operation analyze
                self.status = "Batch: Specify glob pattern via CLI directly".to_string();
                return;
            },
            CommandAction::Info => {
                // nat3d info file.obj
                self.status = "Info: Specify file via CLI directly".to_string();
                return;
            },
            CommandAction::Validate => {
                // nat3d validate file.obj --auto-fix
                self.status = "Validate: Specify file via CLI directly".to_string();
                return;
            },
            CommandAction::Render => {
                // nat3d render scene.obj --output render.png
                ProcessCommand::new(&cli_path)
                    .args(["render", "scene.obj", "--output", "render.png", "--samples", "64"])
                    .output()
            },
            CommandAction::Script => {
                // nat3d script script.py
                self.status = "Script: Specify Python file via CLI directly".to_string();
                return;
            },
            CommandAction::Benchmark => {
                // nat3d benchmark --bench-type all
                ProcessCommand::new(&cli_path)
                    .args(["benchmark", "--bench-type", "all", "--iterations", "5"])
                    .output()
            },
        };

        match result {
            Ok(output) => {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    self.status = format!("{}: Success - {}", cmd.name, stdout.lines().next().unwrap_or("Done"));
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    self.status = format!("{}: Error - {}", cmd.name, stderr.lines().next().unwrap_or("Unknown error"));
                }
            },
            Err(e) => {
                self.status = format!("{}: Failed to execute - {}", cmd.name, e);
            }
        }
    }
}

fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new();

    // Run event loop
    let res = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {err}");
    }

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    handle_key_event(key, app);
                }
            }
        }

        if app.should_quit {
            break;
        }
    }
    Ok(())
}

fn handle_key_event(key: KeyEvent, app: &mut App) {
    match key.code {
        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
            app.should_quit = true;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.next();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.previous();
        }
        KeyCode::Enter => {
            app.execute_command();
        }
        _ => {}
    }
}

fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Title
            Constraint::Min(0),     // Main content
            Constraint::Length(3),  // Status
        ])
        .split(f.area());

    // Title
    let title = Paragraph::new("NAT3D Interactive Terminal UI")
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    // Main content area
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),  // Menu
            Constraint::Percentage(50),  // Description
        ])
        .split(chunks[1]);

    // Command menu
    let items: Vec<ListItem> = app
        .commands
        .iter()
        .enumerate()
        .map(|(i, cmd)| {
            let style = if i == app.selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(cmd.name).style(style)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .title("Commands (↑↓ to navigate, Enter to select)")
            .borders(Borders::ALL),
    );
    f.render_widget(list, main_chunks[0]);

    // Description panel
    let selected_cmd = &app.commands[app.selected];
    let description_text = vec![
        Line::from(vec![
            Span::styled("Command: ", Style::default().fg(Color::Yellow)),
            Span::styled(
                selected_cmd.name,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Description:",
            Style::default().fg(Color::Yellow),
        )]),
        Line::from(selected_cmd.description),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Usage:",
            Style::default().fg(Color::Yellow),
        )]),
        Line::from(get_usage_text(selected_cmd.action)),
    ];

    let description = Paragraph::new(description_text)
        .block(Block::default().title("Details").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    f.render_widget(description, main_chunks[1]);

    // Status bar
    let status = Paragraph::new(app.status.as_str())
        .style(Style::default().fg(Color::Green))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(status, chunks[2]);
}

fn get_usage_text(action: CommandAction) -> &'static str {
    match action {
        CommandAction::Generate => "nat3d generate <primitive> --output <file>",
        CommandAction::Convert => "nat3d convert <input> <output>",
        CommandAction::Batch => "nat3d batch --operation <op> <pattern>",
        CommandAction::Info => "nat3d info <file>",
        CommandAction::Validate => "nat3d validate <file>",
        CommandAction::Render => "nat3d render <scene> --output <image>",
        CommandAction::Script => "nat3d script <file.py>",
        CommandAction::Benchmark => "nat3d benchmark [suite]",
    }
}
