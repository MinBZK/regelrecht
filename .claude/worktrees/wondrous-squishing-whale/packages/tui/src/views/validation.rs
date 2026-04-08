use crate::backend::corpus_scanner;
use crate::backend::process_runner::{ProcessMessage, ProcessMessageKind, ProcessRunner};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{
    Block, Borders, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation,
    ScrollbarState, Wrap,
};
use std::path::{Path, PathBuf};

const MAX_OUTPUT_LINES: usize = 10_000;

#[derive(Clone)]
struct ValidationFile {
    path: PathBuf,
    display_name: String,
    status: FileStatus,
}

#[derive(Clone, Copy, PartialEq)]
enum FileStatus {
    Unchecked,
    Valid,
    Invalid,
}

pub struct ValidationView {
    files: Vec<ValidationFile>,
    list_state: ListState,
    output: Vec<String>,
    output_scroll: usize,
    running: bool,
    scanned: bool,
}

impl ValidationView {
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            list_state: ListState::default(),
            output: Vec::new(),
            output_scroll: 0,
            running: false,
            scanned: false,
        }
    }

    pub fn scan_files(&mut self, project_root: &Path) {
        let yaml_files = corpus_scanner::corpus_yaml_files(project_root);
        self.files = yaml_files
            .into_iter()
            .map(|path| {
                let display_name = path
                    .strip_prefix(project_root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();
                ValidationFile {
                    path,
                    display_name,
                    status: FileStatus::Unchecked,
                }
            })
            .collect();
        self.scanned = true;
        if !self.files.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent, runner: &mut ProcessRunner) {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.move_selection(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_selection(-1),
            KeyCode::Char('g') => self.list_state.select(Some(0)),
            KeyCode::Char('G') => {
                if !self.files.is_empty() {
                    self.list_state.select(Some(self.files.len() - 1));
                }
            }
            KeyCode::Char('a') | KeyCode::Enter => {
                if !self.running && !runner.is_running() {
                    self.output.clear();
                    self.output_scroll = 0;
                    self.running = true;
                    // Reset all statuses
                    for f in &mut self.files {
                        f.status = FileStatus::Unchecked;
                    }
                    runner.run_just("validate:all".to_string(), "validate");
                }
            }
            _ => {}
        }
    }

    pub fn handle_process_message(&mut self, msg: ProcessMessage) {
        match msg.kind {
            ProcessMessageKind::Stdout(line) | ProcessMessageKind::Stderr(line) => {
                self.parse_validation_line(&line);
                self.output.push(line);
                if self.output.len() > MAX_OUTPUT_LINES {
                    self.output.drain(..self.output.len() - MAX_OUTPUT_LINES);
                }
                self.auto_scroll();
            }
            ProcessMessageKind::Done { exit_code } => {
                self.running = false;
                // If exit code 0, mark all unchecked as valid
                if exit_code == Some(0) {
                    for f in &mut self.files {
                        if f.status == FileStatus::Unchecked {
                            f.status = FileStatus::Valid;
                        }
                    }
                }
            }
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if area.width >= 80 {
            let layout =
                Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)])
                    .split(area);

            self.render_file_list(frame, layout[0]);
            self.render_output(frame, layout[1]);
        } else {
            self.render_file_list(frame, area);
        }
    }

    fn render_file_list(&self, frame: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self
            .files
            .iter()
            .map(|f| {
                let icon = match f.status {
                    FileStatus::Unchecked => "○",
                    FileStatus::Valid => "✓",
                    FileStatus::Invalid => "✗",
                };
                let style = match f.status {
                    FileStatus::Invalid => Style::default().add_modifier(Modifier::BOLD),
                    FileStatus::Valid => Style::default().add_modifier(Modifier::DIM),
                    FileStatus::Unchecked => Style::default(),
                };
                ListItem::new(Line::styled(format!(" {icon} {}", f.display_name), style))
            })
            .collect();

        let valid_count = self
            .files
            .iter()
            .filter(|f| f.status == FileStatus::Valid)
            .count();
        let invalid_count = self
            .files
            .iter()
            .filter(|f| f.status == FileStatus::Invalid)
            .count();
        let title = format!(
            " Validation — {} files  ✓{}  ✗{} ",
            self.files.len(),
            valid_count,
            invalid_count
        );

        let block = Block::default().borders(Borders::ALL).title(Span::styled(
            title,
            Style::default().add_modifier(Modifier::BOLD),
        ));

        let list = List::new(items)
            .block(block)
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        let mut state = self.list_state;
        frame.render_stateful_widget(list, area, &mut state);
    }

    fn render_output(&self, frame: &mut Frame, area: Rect) {
        let inner_height = area.height.saturating_sub(2) as usize;

        let visible: Vec<Line> = self
            .output
            .iter()
            .skip(self.output_scroll)
            .take(inner_height)
            .map(|l| Line::styled(l.as_str(), Style::default().add_modifier(Modifier::DIM)))
            .collect();

        let title = if self.running {
            " ● Validating... "
        } else {
            " Output — a/Enter:validate all "
        };

        let block = Block::default().borders(Borders::ALL).title(Span::styled(
            title,
            Style::default().add_modifier(Modifier::BOLD),
        ));

        let paragraph = Paragraph::new(visible)
            .block(block)
            .wrap(Wrap { trim: false });
        frame.render_widget(paragraph, area);

        if self.output.len() > inner_height {
            let mut state = ScrollbarState::new(self.output.len().saturating_sub(inner_height))
                .position(self.output_scroll);
            frame.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight),
                area.inner(Margin {
                    vertical: 1,
                    horizontal: 0,
                }),
                &mut state,
            );
        }
    }

    fn move_selection(&mut self, delta: i32) {
        let len = self.files.len();
        if len == 0 {
            return;
        }
        let current = self.list_state.selected().unwrap_or(0) as i32;
        let next = (current + delta).clamp(0, len as i32 - 1) as usize;
        self.list_state.select(Some(next));
    }

    fn parse_validation_line(&mut self, line: &str) {
        // Look for error patterns from the validator
        if line.contains("error") || line.contains("Error") || line.contains("FAILED") {
            // Try to match a file path in the error
            for f in &mut self.files {
                if line.contains(&f.display_name)
                    || line.contains(f.path.to_string_lossy().as_ref())
                {
                    f.status = FileStatus::Invalid;
                }
            }
        }
    }

    fn auto_scroll(&mut self) {
        if self.output.len() > 10 {
            self.output_scroll = self.output.len().saturating_sub(10);
        }
    }
}
