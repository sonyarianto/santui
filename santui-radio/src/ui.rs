use crate::state::{PlayState, RadioState};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Gauge, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

pub fn draw_radio(f: &mut Frame, area: Rect, state: &RadioState) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 3), Constraint::Ratio(2, 3)])
        .split(area);

    draw_station_list(f, chunks[0], state);
    draw_now_playing(f, chunks[1], state);

    if state.show_help {
        draw_help_popup(f);
    }
}

fn draw_station_list(f: &mut Frame, area: Rect, state: &RadioState) {
    let items: Vec<ListItem> = state
        .filtered
        .iter()
        .enumerate()
        .map(|(i, &station_idx)| {
            let station = state.stations[station_idx];
            let is_selected = i == state.selected;
            let is_current = state.current_station == Some(station_idx);
            let style = if is_selected {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else if is_current {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default().fg(Color::White)
            };
            let icon = if is_current { " ♫ " } else { "   " };
            ListItem::new(Line::from(Span::styled(
                format!("{icon}{}", station.name),
                style,
            )))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Stations ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .highlight_style(Style::default().fg(Color::Black).bg(Color::Cyan));

    f.render_widget(list, area);
}

fn draw_now_playing(f: &mut Frame, area: Rect, state: &RadioState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(6), Constraint::Min(6), Constraint::Length(12)])
        .split(area);

    draw_info_panel(f, chunks[0], state);
    draw_lyrics(f, chunks[1]);
    draw_volume_gauge(f, chunks[2], state);
}

fn draw_info_panel(f: &mut Frame, area: Rect, state: &RadioState) {
    let block = Block::default()
        .title(" Now Playing ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let (station_line, status_line) = match &state.play_state {
        PlayState::Stopped => (
            Line::from(Span::styled(
                "No station selected",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(Span::styled("⏹  Stopped", Style::default().fg(Color::Red))),
        ),
        PlayState::Playing(name) => {
            let title = if state.song_title.is_empty() {
                "Buffering...".to_string()
            } else {
                state.song_title.clone()
            };
            (
                Line::from(Span::styled(name.clone(), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))),
                Line::from(Span::styled(format!("♫  {title}"), Style::default().fg(Color::White))),
            )
        }
        PlayState::Error(e) => (
            Line::from(Span::styled("Error", Style::default().fg(Color::Red))),
            Line::from(Span::styled(format!("⚠  {e}"), Style::default().fg(Color::Red))),
        ),
    };

    let lines = vec![station_line, status_line];
    let p = Paragraph::new(lines).block(block).wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

fn draw_lyrics(f: &mut Frame, area: Rect) {
    let block = Block::default()
        .title(" Info ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let text = vec![
        Line::from(Span::styled(
            "↑/↓  Select station",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            "Enter  Play selected",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            "s  Stop",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            "+/-  Volume",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            "/  Filter stations",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            "?  Toggle help",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            "Esc  Back to menu",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let p = Paragraph::new(text).block(block);
    f.render_widget(p, area);
}

fn draw_volume_gauge(f: &mut Frame, area: Rect, state: &RadioState) {
    let block = Block::default()
        .title(" Volume ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0)])
        .margin(1)
        .split(area);

    let label = format!("{}%", state.volume);
    let gauge = Gauge::default()
        .gauge_style(
            Style::default()
                .fg(Color::Green)
                .bg(Color::DarkGray),
        )
        .percent(state.volume as u16)
        .label(label);

    f.render_widget(block, area);
    f.render_widget(gauge, inner[0]);
}

fn draw_help_popup(f: &mut Frame) {
    let area = f.area();
    let popup = Rect {
        x: area.width / 4,
        y: area.height / 4,
        width: area.width / 2,
        height: 14,
    };

    let text = vec![
        Line::from(Span::styled("Help", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from("↑/↓      Navigate station list"),
        Line::from("Enter    Play selected station"),
        Line::from("s         Stop playback"),
        Line::from("+/-       Adjust volume"),
        Line::from("/         Filter stations by name/genre"),
        Line::from("?         Toggle this help"),
        Line::from("Esc       Back to Santui menu"),
        Line::from("q         Quit"),
        Line::from(""),
        Line::from(Span::styled("Press any key to close", Style::default().fg(Color::DarkGray))),
    ];

    let block = Block::default()
        .title(" Help ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let p = Paragraph::new(text).block(block).alignment(ratatui::layout::Alignment::Center);
    f.render_widget(Clear, popup);
    f.render_widget(p, popup);
}
