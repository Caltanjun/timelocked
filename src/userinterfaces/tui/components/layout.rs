use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;

use crate::userinterfaces::tui::app_state::{App, FooterContent};

use super::form::key_hints_line;
use super::theme::{base_style, muted_style, panel_block, plain_block};

const MIN_WIDTH_FOR_HELP_BLOCK: u16 = 72;
const MIN_WIDTH_FOR_HEADER_SUBTITLE: u16 = 60;

pub(crate) fn constrained_page_area(
    viewport: Rect,
    max_width: u16,
    min_horizontal_margin: u16,
) -> Rect {
    let available = viewport
        .width
        .saturating_sub(min_horizontal_margin.saturating_mul(2));
    let target_width = max_width.min(available.max(1));

    if viewport.width <= target_width + min_horizontal_margin.saturating_mul(2) {
        return viewport;
    }

    let x = viewport.x + (viewport.width - target_width) / 2;
    Rect {
        x,
        y: viewport.y,
        width: target_width,
        height: viewport.height,
    }
}

pub(crate) fn render_small_terminal(frame: &mut Frame, area: Rect, app: &App) {
    let block = panel_block("Timelocked", app);
    let paragraph = Paragraph::new(vec![
        Line::from("Terminal is too small for the TUI layout (min 46x20)."),
        Line::from("Resize and run `timelocked` again."),
    ])
    .wrap(Wrap { trim: true })
    .block(block);
    frame.render_widget(paragraph, area);
}

pub(crate) fn render_header(frame: &mut Frame, area: Rect, app: &App) {
    let mut spans = vec![Span::styled(
        "Timelocked",
        ratatui::style::Style::default().add_modifier(ratatui::style::Modifier::BOLD),
    )];
    if header_shows_subtitle(area.width) {
        spans.push(Span::styled(
            "  Create and unlock timed-release files",
            muted_style(app),
        ));
    }
    let title = Line::from(spans);
    let header = Paragraph::new(vec![title]).block(plain_block(app));
    frame.render_widget(header, area);
}

pub(crate) fn render_footer(frame: &mut Frame, area: Rect, footer: FooterContent, app: &App) {
    if !footer_shows_help_block(area.width) {
        let compact_footer = Paragraph::new(vec![key_hints_line(&footer.left, app)])
            .block(plain_block(app))
            .wrap(Wrap { trim: true });
        frame.render_widget(compact_footer, area);
        return;
    }

    let parts = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(area);

    let left = Paragraph::new(vec![key_hints_line(&footer.left, app)])
        .block(plain_block(app))
        .wrap(Wrap { trim: true });
    let center = Paragraph::new(footer.center)
        .block(plain_block(app))
        .wrap(Wrap { trim: true });

    frame.render_widget(left, parts[0]);
    frame.render_widget(center, parts[1]);
}

pub(crate) fn render_block_paragraph<'a>(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    lines: Vec<Line<'a>>,
    app: &App,
) {
    let paragraph = Paragraph::new(lines)
        .block(panel_block(title, app))
        .wrap(Wrap { trim: true })
        .style(base_style(app));
    frame.render_widget(paragraph, area);
}

fn footer_shows_help_block(width: u16) -> bool {
    width >= MIN_WIDTH_FOR_HELP_BLOCK
}

fn header_shows_subtitle(width: u16) -> bool {
    width >= MIN_WIDTH_FOR_HEADER_SUBTITLE
}

pub(crate) fn centered_rect(area: Rect, width_percent: u16, height_percent: u16) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - height_percent) / 2),
            Constraint::Percentage(height_percent),
            Constraint::Percentage((100 - height_percent) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - width_percent) / 2),
            Constraint::Percentage(width_percent),
            Constraint::Percentage((100 - width_percent) / 2),
        ])
        .split(vertical[1])[1]
}

#[cfg(test)]
mod tests {
    use super::{footer_shows_help_block, header_shows_subtitle};

    #[test]
    fn footer_hides_help_block_below_72_columns() {
        assert!(!footer_shows_help_block(50));
        assert!(!footer_shows_help_block(71));
        assert!(footer_shows_help_block(72));
    }

    #[test]
    fn header_hides_subtitle_below_60_columns() {
        assert!(!header_shows_subtitle(50));
        assert!(!header_shows_subtitle(59));
        assert!(header_shows_subtitle(60));
    }
}
