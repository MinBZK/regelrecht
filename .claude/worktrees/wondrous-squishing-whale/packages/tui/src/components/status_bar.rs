use crate::app::{App, Tab};
use ratatui::prelude::*;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let hints = match app.active_tab {
        Tab::Dashboard => "0-9:tabs",
        Tab::Bdd => "Enter/a:run all  j/k:nav  Tab:focus",
        Tab::Engine => "Enter:eval  j/k:nav  Tab:field",
        Tab::Corpus => "Enter:open  Space:expand  j/k:nav  Tab:focus",
        Tab::Pipeline => "r:refresh  j/k:nav",
        Tab::Validation => "Enter/a:validate all  j/k:nav",
        Tab::Trace => "Space:collapse  j/k:nav  e:expand  c:collapse",
        Tab::Dependencies => "j/k:nav  J/K:scroll detail",
        Tab::Logs => "e/w/i/d:filter  f:follow  j/k:nav",
        Tab::Actions => "f:fmt  l:lint  b:build  v:validate  c:check  t:test",
    };

    let dim = Style::default().add_modifier(Modifier::DIM);
    let bold = Style::default().add_modifier(Modifier::BOLD);

    // Build branch display
    let branch_display = match &app.corpus_branch {
        Some(corpus_branch) => format!(" {} corpus:{}", app.repo_branch, corpus_branch),
        None => format!(" {}", app.repo_branch),
    };

    let line = Line::from(vec![
        Span::styled(" q:quit ?:help ", dim),
        Span::styled("│ ", dim),
        Span::styled(hints, dim),
        // Right-align the branch info
        Span::styled("│ ", dim),
        Span::styled(branch_display, bold),
    ]);

    frame.render_widget(line, area);
}
