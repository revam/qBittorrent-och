use crate::app_state::{AppMode, AppState, DetailPane, LoginField, View};
use crate::models::{ScriptState, SortDirection, SortField, Torrent};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

pub fn render_app(frame: &mut Frame, state: &AppState) {
    if state.mode == AppMode::Login {
        render_login(frame, state);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(frame.area());

    render_header(frame, state, chunks[0]);

    if state.view == View::Help {
        render_help(frame, state, chunks[1]);
    } else {
        match state.view {
            View::TorrentList => render_torrent_list(frame, state, chunks[1]),
            View::LogView => render_log_view(frame, state, chunks[1]),
            View::VerticalSplit => {
                let split_chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .split(chunks[1]);
                render_torrent_list(frame, state, split_chunks[0]);
                render_log_view(frame, state, split_chunks[1]);
            }
            View::HorizontalSplit => {
                let split_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .split(chunks[1]);
                render_torrent_list(frame, state, split_chunks[0]);
                render_log_view(frame, state, split_chunks[1]);
            }
            View::Help => {}
        }
    }
}

fn render_login(frame: &mut Frame, state: &AppState) {
    let size = frame.area();
    let box_width = 50;
    let box_height = 15;
    let x = (size.width.saturating_sub(box_width)) / 2;
    let y = (size.height.saturating_sub(box_height)) / 2;
    let area = Rect::new(x, y, box_width, box_height);

    let title = " qBittorrent Login ";
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    frame.render_widget(block, area);

    let inner = Rect::new(x + 2, y + 2, box_width - 4, box_height - 4);

    let host_label = "Host:";
    let host_value = &state.login_host;
    let host_style = if state.login_field == LoginField::Host {
        Style::default().fg(Color::Yellow).bold()
    } else {
        Style::default().fg(Color::Gray)
    };

    let username_label = "Username:";
    let username_value = &state.login_username;
    let username_style = if state.login_field == LoginField::Username {
        Style::default().fg(Color::Yellow).bold()
    } else {
        Style::default().fg(Color::Gray)
    };

    let password_label = "Password:";
    let password_value = "*".repeat(state.login_password.len());
    let password_style = if state.login_field == LoginField::Password {
        Style::default().fg(Color::Yellow).bold()
    } else {
        Style::default().fg(Color::Gray)
    };

    let lines = vec![
        Line::from(vec![Span::raw(" ")]),
        Line::from(vec![
            Span::raw(format!("{:<10}", host_label)),
            Span::styled(host_value, host_style),
        ]),
        Line::from(vec![Span::raw(" ")]),
        Line::from(vec![
            Span::raw(format!("{:<10}", username_label)),
            Span::styled(username_value, username_style),
        ]),
        Line::from(vec![Span::raw(" ")]),
        Line::from(vec![
            Span::raw(format!("{:<10}", password_label)),
            Span::styled(password_value, password_style),
        ]),
        Line::from(vec![Span::raw(" ")]),
        Line::from(vec![
            Span::raw(" "),
            Span::styled("Tab: Next field", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::raw(" "),
            Span::styled("Enter: Connect", Style::default().fg(Color::DarkGray)),
        ]),
    ];

    let paragraph = Paragraph::new(lines).scroll((0, 0));
    frame.render_widget(paragraph, inner);

    if let Some(ref error) = state.error_message {
        let error_para = Paragraph::new(error.as_str()).style(Style::default().fg(Color::Red));
        let error_area = Rect::new(x + 2, y + box_height - 3, box_width - 4, 1);
        frame.render_widget(error_para, error_area);
    }
}

fn render_torrent_list(frame: &mut Frame, state: &AppState, area: Rect) {
    let filtered: Vec<&Torrent> = state
        .torrents
        .iter()
        .filter(|t| (t.progress - 1.0).abs() < f64::EPSILON || t.progress >= 1.0)
        .filter(|t| {
            use crate::models::ScriptStateFilter;
            match state.filter {
                ScriptStateFilter::All => true,
                ScriptStateFilter::NotRan => t.script_state() == ScriptState::NotRan,
                ScriptStateFilter::Ok => t.script_state() == ScriptState::Ok,
                ScriptStateFilter::Fail => t.script_state() == ScriptState::Fail,
            }
        })
        .collect();

    if filtered.is_empty() {
        let paragraph = Paragraph::new("No completed torrents found.")
            .block(
                Block::default()
                    .title("Completed Torrents")
                    .borders(Borders::ALL),
            )
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(paragraph, area);
        return;
    }

    // Split area into list area and (optional) details area
    let selected_torrent = state
        .filtered_torrents()
        .into_iter()
        .nth(state.selected_index)
        .cloned();

    let (list_area, details_area) = if state.show_details && selected_torrent.is_some() {
        // Details height: min(50% of area height, 20), but at least 2 lines if area is too small
        let max_details = ((area.height as usize) / 2).min(20) as u16;
        let details_height = if area.height <= 4 {
            2
        } else {
            max_details.max(2)
        };
        let list_height = area.height.saturating_sub(details_height);
        let la = Rect::new(area.x, area.y, area.width, list_height);
        let da = Rect::new(area.x, area.y + list_height, area.width, details_height);
        (la, Some(da))
    } else {
        (area, None)
    };

    let header_height = 2;
    let list_border_height = 1;
    let available_for_list = list_area.height;
    let list_content_height = available_for_list
        .saturating_sub(header_height + list_border_height)
        .max(1);

    let header_area = Rect::new(list_area.x, list_area.y, list_area.width, header_height + 1);
    let inner_list_area = Rect::new(
        list_area.x,
        list_area.y + header_height,
        list_area.width,
        list_content_height + list_border_height,
    );

    let max_visible = list_content_height as usize;
    let max_scroll = filtered.len().saturating_sub(max_visible);
    let scroll = state.scroll_offset.min(max_scroll);

    let completed_width = 19;
    let added_width = 19;
    let name_width = (list_area.width as usize)
        .saturating_sub(completed_width + added_width + 6)
        .max(10);

    let sort = &state.sort_config;
    let header_style = Style::default().bg(Color::DarkGray).fg(Color::White).bold();
    let active_sort_style = Style::default()
        .bg(Color::DarkGray)
        .fg(Color::Yellow)
        .bold();

    let name_indicator = if sort.field == SortField::Name {
        if sort.direction == SortDirection::Asc {
            " ↑"
        } else {
            " ↓"
        }
    } else {
        "  "
    };
    let completed_indicator = if sort.field == SortField::CompletedAt {
        if sort.direction == SortDirection::Asc {
            " ↑"
        } else {
            " ↓"
        }
    } else {
        "  "
    };
    let added_indicator = if sort.field == SortField::AddedAt {
        if sort.direction == SortDirection::Asc {
            " ↑"
        } else {
            " ↓"
        }
    } else {
        "  "
    };

    let header_line = Line::from(vec![
        Span::styled(" |", header_style),
        Span::styled(
            format!(
                "{:<name_width$}",
                format!(" Name{}", name_indicator),
                name_width = name_width
            ),
            if sort.field == SortField::Name {
                active_sort_style
            } else {
                header_style
            },
        ),
        Span::styled("|", header_style),
        Span::styled(
            format!("{:<19}", format!("Completed{}", completed_indicator)),
            if sort.field == SortField::CompletedAt {
                active_sort_style
            } else {
                header_style
            },
        ),
        Span::styled("|", header_style),
        Span::styled(
            format!("{:<19}", format!("Added{}", added_indicator)),
            if sort.field == SortField::AddedAt {
                active_sort_style
            } else {
                header_style
            },
        ),
    ]);
    let header_para = Paragraph::new(header_line)
        .block(Block::default().borders(Borders::LEFT | Borders::RIGHT | Borders::TOP));
    frame.render_widget(header_para, header_area);

    let visible: Vec<ListItem> = filtered
        .iter()
        .skip(scroll)
        .take(max_visible)
        .enumerate()
        .map(|(i, torrent)| {
            let idx = scroll + i;
            let is_selected = idx == state.selected_index;

            let state_symbol = torrent.script_state().symbol();
            let state_color = torrent.script_state().color();

            let name = truncate_string(&torrent.name, name_width.saturating_sub(3));
            let name_padded = format!("{:<name_width$}", name, name_width = name_width);
            let added = timestamp_to_date(torrent.added_on as i64);
            let completed_at = torrent
                .completion_on
                .filter(|&c| c > 0)
                .map(timestamp_to_date)
                .unwrap_or_else(|| "-".to_string());

            let style = if is_selected {
                Style::default().bg(Color::Blue).fg(Color::White)
            } else if idx % 2 == 0 {
                Style::default()
            } else {
                Style::default().bg(Color::Black)
            };

            ListItem::new(Line::from(vec![
                Span::styled(state_symbol, Style::default().fg(state_color)),
                Span::raw(" "),
                Span::styled(name_padded, style),
                Span::raw(" "),
                Span::styled(format!("{:<19}", completed_at), style),
                Span::raw(" "),
                Span::styled(format!("{:<19}", added), style),
            ]))
        })
        .collect();

    let list = List::new(visible)
        .block(Block::default().borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM))
        .highlight_style(Style::default().bg(Color::Blue).fg(Color::White));

    frame.render_widget(list, inner_list_area);

    if let (Some(da), Some(torrent)) = (details_area, selected_torrent) {
        render_details(frame, state, &torrent, da);
    }
}

fn render_details(frame: &mut Frame, state: &AppState, torrent: &Torrent, area: Rect) {
    let panes = [
        DetailPane::Info,
        DetailPane::Paths,
        DetailPane::Transfer,
        DetailPane::Files,
    ];
    let pane_labels = ["Info", "Paths", "Transfer", "Files"];

    // Build tab title
    let mut title_spans = vec![Span::raw(" ")];
    for (i, (pane, label)) in panes.iter().zip(pane_labels.iter()).enumerate() {
        if i > 0 {
            title_spans.push(Span::raw(" | "));
        }
        if *pane == state.detail_pane {
            title_spans.push(Span::styled(
                *label,
                Style::default().fg(Color::Yellow).bold(),
            ));
        } else {
            title_spans.push(Span::styled(*label, Style::default().fg(Color::DarkGray)));
        }
    }
    title_spans.push(Span::raw(" "));
    let title = Line::from(title_spans);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines: Vec<Line> = match state.detail_pane {
        DetailPane::Info => {
            let state_str = torrent_state_label(&torrent.state);
            let size = if torrent.size == torrent.size_total {
                format!("{}", format_bytes(torrent.size))
            } else {
                format!(
                    "{} ({})",
                    format_bytes(torrent.size),
                    format_bytes(torrent.size_total)
                )
            };
            let ratio = format!("{:.2}", torrent.ratio);
            let seeds = format!("{} ({})", torrent.num_seeds, torrent.num_complete);
            let peers = format!("{} ({})", torrent.num_leeches, torrent.num_incomplete);
            let added = timestamp_to_date(torrent.added_on as i64);
            let completed = torrent
                .completion_on
                .filter(|&c| c > 0)
                .map(timestamp_to_date)
                .unwrap_or_else(|| "-".to_string());
            let category = if torrent.category.is_empty() {
                "-".to_string()
            } else {
                torrent.category.clone()
            };
            let tags = if torrent.tags.is_empty() {
                "-".to_string()
            } else {
                torrent.tags.clone()
            };
            let tracker = if torrent.tracker.is_empty() {
                "-".to_string()
            } else {
                torrent.tracker.clone()
            };

            detail_rows(&[
                ("Name", &torrent.name),
                (
                    "Info Hash v1",
                    &torrent
                        .infohash_v1
                        .clone()
                        .unwrap_or_else(|| "N/A".to_string()),
                ),
                (
                    "Info Hash v2",
                    &torrent
                        .infohash_v2
                        .clone()
                        .unwrap_or_else(|| "N/A".to_string()),
                ),
                ("State", &state_str),
                ("Size", &size),
                ("Ratio", &ratio),
                ("Seeds", &seeds),
                ("Peers", &peers),
                ("Added", &added),
                ("Completed", &completed),
                ("Category", &category),
                ("Tags", &tags),
                ("Tracker", &tracker),
            ])
        }
        DetailPane::Paths => detail_rows(&[
            ("Save path", &torrent.save_path),
            ("Content path", &torrent.content_path),
            ("Hash", &torrent.hash),
        ]),
        DetailPane::Transfer => {
            let uploaded = format_bytes(torrent.uploaded);
            let downloaded = format_bytes(torrent.downloaded);
            let ul_speed = format!("{}/s", format_bytes(torrent.speed_up));
            let dl_speed = format!("{}/s", format_bytes(torrent.speed_down));
            let active_time = format_duration(torrent.time_active);
            let seeding_time = torrent
                .seeding_time
                .map(format_duration)
                .unwrap_or_else(|| "-".to_string());

            detail_rows(&[
                ("Uploaded", &uploaded),
                ("Downloaded", &downloaded),
                ("Upload speed", &ul_speed),
                ("DL speed", &dl_speed),
                ("Active time", &active_time),
                ("Seeding time", &seeding_time),
            ])
        }
        DetailPane::Files => {
            if state.files_loading.as_deref() == Some(torrent.hash.as_str()) {
                vec![Line::from(Span::styled(
                    "Loading...",
                    Style::default().fg(Color::DarkGray),
                ))]
            } else if let Some(files) = state.file_cache.get(&torrent.hash) {
                if files.is_empty() {
                    vec![Line::from(Span::styled(
                        "No files.",
                        Style::default().fg(Color::DarkGray),
                    ))]
                } else {
                    files
                        .iter()
                        .map(|f| {
                            let size = format_bytes(f.size);
                            let pct = format!("{:>3.0}%", f.progress * 100.0);
                            // Strip leading torrent-name prefix if present
                            let name = f.name.splitn(2, '/').last().unwrap_or(&f.name);
                            Line::from(vec![
                                Span::styled(
                                    format!("{} ", pct),
                                    Style::default().fg(Color::DarkGray),
                                ),
                                Span::styled(
                                    format!("{:>9} ", size),
                                    Style::default().fg(Color::DarkGray),
                                ),
                                Span::raw(name.to_string()),
                            ])
                        })
                        .collect()
                }
            } else {
                vec![Line::from(Span::styled(
                    "Unable to load files.",
                    Style::default().fg(Color::DarkGray),
                ))]
            }
        }
    };

    let paragraph = Paragraph::new(lines).wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

fn detail_rows(rows: &[(&str, &str)]) -> Vec<Line<'static>> {
    rows.iter()
        .map(|(label, value)| {
            Line::from(vec![
                Span::styled(
                    format!("{:<14}", label),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw(value.to_string()),
            ])
        })
        .collect()
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;
    if bytes >= TB {
        format!("{:.2} TiB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GiB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MiB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0} KiB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

fn format_duration(secs: u64) -> String {
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let mins = (secs % 3600) / 60;
    if days > 0 {
        format!("{}d {}h {}m", days, hours, mins)
    } else if hours > 0 {
        format!("{}h {}m", hours, mins)
    } else {
        format!("{}m", mins)
    }
}

fn torrent_state_label(state: &str) -> String {
    match state {
        "uploading" => "Seeding",
        "stalledUP" => "Seeding (stalled)",
        "downloading" => "Downloading",
        "stalledDL" => "Downloading (stalled)",
        "pausedUP" | "stoppedUP" => "Paused (complete)",
        "pausedDL" | "stoppedDL" => "Paused",
        "checkingUP" | "checkingDL" => "Checking",
        "moving" => "Moving",
        "error" => "Error",
        "missingFiles" => "Missing files",
        other => other,
    }
    .to_string()
}

fn render_log_view(frame: &mut Frame, state: &AppState, area: Rect) {
    if state.logs.is_empty() {
        let paragraph = Paragraph::new("No script execution logs yet.\n\nSelect a torrent and press 'r' to run the on-complete script.")
            .block(Block::default()
                .title("Script Output Log")
                .borders(Borders::ALL)
            )
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(paragraph, area);
        return;
    }

    let start_idx = state.log_scroll.min(state.logs.len().saturating_sub(1));
    let visible_logs = &state.logs[start_idx..];

    let torrent_map: std::collections::HashMap<String, String> = state
        .torrents
        .iter()
        .map(|t| (t.hash.clone(), t.name.clone()))
        .collect();

    let items: Vec<ListItem> = visible_logs
        .iter()
        .map(|log| {
            let timestamp = log.started_at.format("%H:%M:%S");
            let status = if log.success { "✓" } else { "✗" };

            let display_name = torrent_map.get(&log.hash).cloned().unwrap_or_else(|| {
                if log.name.is_empty() {
                    log.hash.clone()
                } else {
                    log.name.clone()
                }
            });

            let name_style = if torrent_map.contains_key(&log.hash) {
                Style::default().fg(Color::Yellow).bold()
            } else {
                Style::default().fg(Color::DarkGray).italic()
            };

            let mut lines = vec![Line::from(vec![
                Span::raw(format!("[{}] {} ", timestamp, status)),
                Span::styled(display_name.clone(), name_style),
            ])];

            let duration = (log.completed_at - log.started_at).num_milliseconds();
            let duration_str = if duration < 1000 {
                format!("{}ms", duration)
            } else {
                format!("{:.2}s", duration as f64 / 1000.0)
            };
            lines.push(Line::from(vec![
                Span::raw("  Duration: "),
                Span::styled(duration_str, Style::default().fg(Color::Cyan)),
            ]));

            if let Some(code) = log.exit_code {
                lines.push(Line::from(vec![
                    Span::raw("  Exit Code: "),
                    Span::styled(
                        code.to_string(),
                        if code == 0 { Color::Green } else { Color::Red },
                    ),
                ]));
            }

            if !log.stdout.is_empty() {
                lines.push(Line::from(vec![
                    Span::raw("  STDOUT: "),
                    Span::styled(
                        truncate_string(&log.stdout.trim(), 100),
                        Style::default().fg(Color::Green),
                    ),
                ]));
            }

            if !log.stderr.is_empty() {
                lines.push(Line::from(vec![
                    Span::raw("  STDERR: "),
                    Span::styled(
                        truncate_string(&log.stderr.trim(), 100),
                        Style::default().fg(Color::Red),
                    ),
                ]));
            }

            ListItem::new(lines)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title("Script Output Log")
                .borders(Borders::ALL),
        )
        .style(Style::default());

    frame.render_widget(list, area);
}

fn render_help(frame: &mut Frame, state: &AppState, area: Rect) {
    let help_lines = vec![
        Line::from(vec![Span::styled(" Keyboard Shortcuts",Style::default().bold().fg(Color::Cyan))]),
        Line::from(vec![Span::raw("")]),
        Line::from(vec![Span::styled("  Global",Style::default().bold().fg(Color::Yellow))]),
        Line::from(vec![Span::raw("")]),
        Line::from(vec![Span::raw("    ↑/↓          Move up/down")]),
        Line::from(vec![Span::raw("    PgUp/PgDn    Page up/down")]),
        Line::from(vec![Span::raw("    Home/End     Jump to first/last")]),
        Line::from(vec![Span::raw("")]),
        Line::from(vec![Span::raw("    h            Open this help page")]),
        Line::from(vec![Span::raw("    q or Esc     Quit / Go back")]),
        Line::from(vec![Span::raw("")]),
        Line::from(vec![Span::styled("  Torrent List",Style::default().bold().fg(Color::Yellow))]),
        Line::from(vec![Span::raw("")]),
        Line::from(vec![Span::raw("    s            Change sort field (Name / Completed / Added)")]),
        Line::from(vec![Span::raw("    S            Toggle ascending/descending sort")]),
        Line::from(vec![Span::raw("")]),
        Line::from(vec![Span::raw("    Tab/⇑+Tab    Cycle filters (All / Not Ran / Ok / Fail)")]),
        Line::from(vec![Span::raw("    v/V          Cycle view mode (Torrent List / Log / Vertical Split /Horizontal Split)")]),
        Line::from(vec![Span::raw("")]),
        Line::from(vec![Span::raw("    r            Run on-complete script on selected torrent")]),
        Line::from(vec![Span::raw("    R            Run on-complete script on all completed torrents")]),
        Line::from(vec![Span::raw("    t            Toggle torrent details pane")]),
        Line::from(vec![Span::raw("    ←/→          Switch details sub-pane (Info / Paths / Transfer / Files)")]),
        Line::from(vec![Span::raw("    Enter        Open torrent details")]),
        Line::from(vec![Span::raw("")]),
        Line::from(vec![Span::styled("  Horizontal / Vertical Split",Style::default().bold().fg(Color::Yellow))]),
        Line::from(vec![Span::raw("")]),
        Line::from(vec![Span::raw("    ⇑+↑/↓        Move log up/down")]),
        Line::from(vec![Span::raw("    ⇑+PgUp/PgDn  Log page up/down")]),
        Line::from(vec![Span::raw("    ⇑+Home/End   Jump to first/last log entry")]),
        Line::from(vec![Span::raw("")]),
        Line::from(vec![Span::styled(" State Indicators", Style::default().bold().fg(Color::Cyan))]),
        Line::from(vec![Span::raw("")]),
        Line::from(vec![
            Span::styled("  ?  ", Style::default().fg(Color::Blue)),
            Span::raw("Script has not been executed yet (no tag)"),
        ]),
        Line::from(vec![
            Span::styled("  ✓  ", Style::default().fg(Color::Green)),
            Span::raw("Script completed successfully (tagged oc_ok)"),
        ]),
        Line::from(vec![
            Span::styled("  !  ", Style::default().fg(Color::Red)),
            Span::raw("Script execution failed (tagged oc_fail)"),
        ]),
        Line::from(vec![Span::raw("")]),
        Line::from(vec![Span::styled(" View Modes", Style::default().bold().fg(Color::Cyan))]),
        Line::from(vec![Span::raw("")]),
        Line::from(vec![Span::raw("  Only Torrents     Full view of completed torrents")]),
        Line::from(vec![Span::raw("  Only Logs         Full view of script execution logs")]),
        Line::from(vec![Span::raw("  Vertical Split    Torrent list on left, log on right")]),
        Line::from(vec![Span::raw("  Horizontal Split  Torrent list on top, log on bottom")]),
        Line::from(vec![Span::raw("")]),
        Line::from(vec![Span::styled(" Tips", Style::default().bold().fg(Color::Cyan))]),
        Line::from(vec![Span::raw("")]),
        Line::from(vec![Span::raw("  - Click column headers to sort by that field")]),
        Line::from(vec![Span::raw("  - Use filters to show only torrents with specific script states")]),
        Line::from(vec![Span::raw("  - The script is configured in qBittorrent's WebUI")]),
        Line::from(vec![Span::raw("    (Tools > Options > Downloads > Run external program)")]),
        Line::from(vec![Span::raw("  - Successful runs are tagged 'oc_ok', failures are tagged 'oc_fail'")]),
        Line::from(vec![Span::raw("")]),
        Line::from(vec![Span::styled(" Press Esc or q to go back ", Style::default().fg(Color::DarkGray))]),
    ];

    let total_lines = help_lines.len() as u16;
    let visible_height = area.height.saturating_sub(2);
    let max_scroll = (total_lines as isize - visible_height as isize).max(0) as usize;
    let scroll = state.help_scroll.min(max_scroll);

    let block = Block::default()
        .title(" Help ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(help_lines)
        .block(block)
        .scroll((scroll as u16, 0));

    frame.render_widget(paragraph, area);
}

fn render_header(frame: &mut Frame, state: &AppState, area: Rect) {
    let filtered = state.filtered_torrents();
    let filtered_count = filtered.len();
    let start = state.scroll_offset.saturating_add(1);
    let end = (state.scroll_offset + 20).min(filtered_count);

    let log_length = state.logs.len();
    let page_info = match state.view {
        View::TorrentList => format!(
            "{}-{} of {} torrents ({}) | Press h for help",
            start.min(filtered_count),
            end,
            filtered_count,
            state.filter.label().to_string(),
        ),
        View::LogView => format!(
            "{} of {} log entries | Press h for help",
            if log_length > 0 {
                state.log_scroll + 1
            } else {
                0
            },
            log_length,
        ),
        View::VerticalSplit | View::HorizontalSplit => format!(
            "{}-{} of {} torrents ({}) | {} of {} log entries | Press h for help",
            start.min(filtered_count),
            end,
            filtered_count,
            state.filter.label(),
            if log_length > 0 {
                state.log_scroll + 1
            } else {
                0
            },
            log_length,
        ),
        View::Help => "Press q or Esc to go back".to_string(),
    };

    let version = state.qbit_version.as_deref().unwrap_or("...");
    let title = format!(" qBittorrent {} ", version);
    let widget = Paragraph::new(page_info)
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(widget, area);
    let text_area = Rect::new(area.x + 1, area.y, title.len() as u16, 1);
    let paragraph = Paragraph::new(title).style(Style::default().fg(Color::Cyan));
    frame.render_widget(paragraph, text_area);
}

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

fn timestamp_to_date(timestamp: i64) -> String {
    use chrono::{TimeZone, Utc};
    if timestamp <= 0 {
        return "-".to_string();
    }
    Utc.timestamp_opt(timestamp, 0)
        .single()
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| "-".to_string())
}
