use crate::state::{AppState, FocusedPane, ImageCacheEntry, LoginFields, MainState};
use chrono::{DateTime, Local};
use image::imageops::FilterType;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui_image::{Resize, StatefulImage};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

const IMAGE_PREVIEW_ROWS: u16 = 10;

struct MsgItem {
    is_me: bool,
    lines: Vec<Line<'static>>,
    lines_height: usize,
    height: usize,
    is_selected: bool,
    is_system: bool,
    image_guids: Vec<(String, String)>, // (guid, display_name)
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

pub(crate) fn render(f: &mut Frame, state: &mut AppState) {
    match state {
        AppState::Login(fields) => render_login(f, fields),
        AppState::Main(main) => render_main(f, main),
    }
}

fn render_login(f: &mut Frame, fields: &LoginFields) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .margin(5)
        .split(area);

    let host_style = if fields.active_field == 0 {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    let pass_style = if fields.active_field == 1 {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    let checkbox_style = if fields.active_field == 2 {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    let checkbox_mark = if fields.use_private_api { "[x]" } else { "[ ]" };

    f.render_widget(
        Paragraph::new(fields.host.as_str())
            .style(host_style)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Host (e.g. http://localhost:1234) "),
            ),
        chunks[0],
    );
    f.render_widget(
        Paragraph::new("*".repeat(fields.password.len()))
            .style(pass_style)
            .block(Block::default().borders(Borders::ALL).title(" Password ")),
        chunks[1],
    );
    f.render_widget(
        Paragraph::new(format!("{} Private API server", checkbox_mark))
            .style(checkbox_style)
            .block(Block::default().borders(Borders::ALL)),
        chunks[2],
    );
    if let Some(error) = &fields.error {
        f.render_widget(
            Paragraph::new(error.as_str())
                .style(Style::default().fg(Color::Red))
                .wrap(Wrap { trim: true }),
            chunks[3],
        );
    }
    f.render_widget(
        Paragraph::new("Enter to login · Tab to switch fields · Space to toggle · Esc to quit"),
        chunks[4],
    );

    match fields.active_field {
        0 => f.set_cursor_position((
            chunks[0].x + fields.host.width() as u16 + 1,
            chunks[0].y + 1,
        )),
        1 => f.set_cursor_position((
            chunks[1].x + fields.password.width() as u16 + 1,
            chunks[1].y + 1,
        )),
        _ => {}
    }
}

fn compose_box_height(main: &MainState, right_width: u16) -> u16 {
    if !main.compose_mode {
        return 3;
    }
    let inner_w = right_width.saturating_sub(2) as usize;
    if inner_w == 0 {
        return 3;
    }
    let num_visual_rows: usize = main
        .compose_text
        .split('\n')
        .map(|line| {
            let n = line.chars().count();
            if n == 0 { 1 } else { n.div_ceil(inner_w) }
        })
        .sum::<usize>()
        .max(1);
    (num_visual_rows as u16 + 2).max(3)
}

fn render_main(f: &mut Frame, main: &mut MainState) {
    let outer = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(f.area());

    let compose_h = compose_box_height(main, outer[1].width);

    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(compose_h)])
        .split(outer[1]);

    render_chat_list(f, outer[0], main);
    render_messages(f, right[0], main);
    render_status_bar(f, right[1], main);

    if matches!(main.focused_pane, FocusedPane::ChatSearch) {
        render_chat_search_overlay(f, f.area(), main);
    }

    if matches!(main.focused_pane, FocusedPane::EmojiPicker) {
        render_emoji_picker_overlay(f, f.area(), main);
    }

    if main.api_requests_in_flight > 0 {
        let spinner_chars = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        let time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let spinner_idx = (time / 100) as usize % spinner_chars.len();
        let spinner_char = spinner_chars[spinner_idx];

        let area = f.area();
        if area.width >= 5 && area.height >= 1 {
            let spinner_area = Rect::new(area.x + area.width - 5, area.y, 3, 1);
            f.render_widget(
                Paragraph::new(format!("{} ", spinner_char))
                    .style(Style::default().fg(Color::Yellow)),
                spinner_area,
            );
        }
    }

    if !main.tui_notifications.is_empty() {
        render_notifications_overlay(f, f.area(), main);
    }

    let area = f.area();
    if area.width >= 2 && area.height >= 1 {
        let is_idle = std::time::Instant::now().duration_since(main.last_activity)
            > std::time::Duration::from_secs(15);
        let status_color = if !is_idle {
            Color::Green
        } else if !main.tui_notifications.is_empty() {
            Color::Red
        } else {
            Color::Yellow
        };
        let status_area = Rect::new(area.x + area.width - 2, area.y, 2, 1);
        f.render_widget(
            Paragraph::new(Span::styled("●", Style::default().fg(status_color))),
            status_area,
        );
    }
}

fn render_notifications_overlay(f: &mut Frame, area: Rect, main: &MainState) {
    let width = 40;
    if area.width < width + 4 || area.height < 5 {
        return;
    }
    let mut current_y = area.y + 1;
    let x = area.x + area.width - width - 2;

    for notif in main.tui_notifications.values() {
        let mut lines = vec![];
        for body in &notif.bodies {
            let wrapped = textwrap::fill(body, width as usize - 4);
            for line in wrapped.lines() {
                lines.push(Line::from(line.to_string()));
            }
        }
        let height = lines.len() as u16 + 2;
        if current_y + height > area.y + area.height {
            break;
        }

        let notif_area = Rect::new(x, current_y, width, height);
        f.render_widget(Clear, notif_area);

        // Add a "drop shadow" by rendering a darker block slightly offset if there's room
        if x + 1 < area.x + area.width && current_y + 1 + height <= area.y + area.height {
            let shadow_area = Rect::new(x + 1, current_y + 1, width, height);
            f.render_widget(
                Block::default().style(Style::default().bg(Color::Indexed(236))), // Dark grey
                shadow_area,
            );
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .title(format!(" {} ", notif.title))
            .style(Style::default().bg(Color::Reset).fg(Color::White));

        f.render_widget(Paragraph::new(lines).block(block), notif_area);

        current_y += height + 1;
    }
}

fn render_chat_list(f: &mut Frame, area: Rect, main: &MainState) {
    let chats_focused = matches!(main.focused_pane, FocusedPane::Chats);
    let list = List::new(
        main.chats
            .iter()
            .enumerate()
            .map(|(chat_idx, c)| {
                let name = main.chat_display_name(c);
                let timestamp = if let Some(msg) = &c.last_message {
                    let ms = msg.date_created;
                    let dt: DateTime<Local> = DateTime::from_timestamp(
                        (ms / 1000) as i64,
                        (ms % 1000) as u32 * 1_000_000,
                    )
                    .unwrap_or_default()
                    .into();
                    dt.format("%I:%M %p %m/%d/%y").to_string()
                } else {
                    String::new()
                };

                let is_selected = main.selected_chat_index == Some(chat_idx);
                let name_style = if !is_selected && main.unread_chats.contains(&c.guid) {
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                let available_width = (area.width as usize).saturating_sub(5);
                let time_width = timestamp.width();

                let mut display_name = name;
                let max_name_width = available_width.saturating_sub(time_width).saturating_sub(1);
                if display_name.width() > max_name_width && max_name_width > 0 {
                    let mut truncated = String::new();
                    let mut curr_width = 0;
                    for ch in display_name.chars() {
                        let w = ch.width().unwrap_or(0);
                        if curr_width + w + 1 > max_name_width {
                            truncated.push('…');
                            break;
                        }
                        truncated.push(ch);
                        curr_width += w;
                    }
                    display_name = truncated;
                }

                let name_width = display_name.width();
                let padding = available_width
                    .saturating_sub(name_width)
                    .saturating_sub(time_width);
                let spaces = " ".repeat(padding);

                let line = Line::from(vec![
                    Span::styled(display_name, name_style),
                    Span::raw(spaces),
                    Span::styled(timestamp, Style::default().fg(Color::Gray)),
                ]);

                ListItem::new(line)
            })
            .collect::<Vec<_>>(),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Chats ")
            .border_style(if chats_focused {
                Style::default()
            } else {
                Style::default().fg(Color::DarkGray)
            }),
    )
    .highlight_style(if chats_focused {
        Style::default()
            .bg(Color::Blue)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().bg(Color::DarkGray)
    })
    .highlight_symbol(">> ");

    f.render_stateful_widget(list, area, &mut main.chat_list_state.clone());
}

fn render_messages(f: &mut Frame, area: Rect, main: &mut MainState) {
    let msgs_focused = matches!(
        main.focused_pane,
        FocusedPane::Messages | FocusedPane::AttachmentPicker
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Messages ")
        .border_style(if msgs_focused {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut items: Vec<MsgItem> = Vec::new();
    let mut last_dt: Option<DateTime<Local>> = None;
    let mut selected_item_idx: Option<usize> = None;

    // Estimate msg_width for image height calculations in the build phase.
    let msg_width_estimate = ((inner.width as f32 * 0.7) as u16).max(1);

    for (i, msg) in main.messages.iter().enumerate() {
        let is_me = msg.is_from_me;
        let is_selected = main.message_selected == Some(i);
        let is_system = msg.handle.is_none() && !msg.is_from_me;

        let ms = msg.date_created;
        let dt: DateTime<Local> =
            DateTime::from_timestamp((ms / 1000) as i64, (ms % 1000) as u32 * 1_000_000)
                .unwrap_or_default()
                .into();

        if let Some(prev) = last_dt {
            if prev.date_naive() != dt.date_naive() {
                let line = Line::from(Span::styled(
                    "─".repeat(inner.width as usize),
                    Style::default().fg(Color::DarkGray),
                ));
                items.push(MsgItem {
                    is_me: false,
                    lines: vec![line],
                    lines_height: 1,
                    height: 1,
                    is_selected: false,
                    is_system: true,
                    image_guids: vec![],
                });
            } else if dt.signed_duration_since(prev).num_minutes() >= 60 {
                let line = Line::from(Span::styled("···", Style::default().fg(Color::DarkGray)));
                items.push(MsgItem {
                    is_me: false,
                    lines: vec![line],
                    lines_height: 1,
                    height: 1,
                    is_selected: false,
                    is_system: true,
                    image_guids: vec![],
                });
            }
        }
        last_dt = Some(dt);

        if is_system {
            if let Some(text) = msg.text.as_deref().filter(|t| !t.is_empty()) {
                let line = Line::from(Span::styled(
                    text.to_string(),
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC),
                ));
                if main.message_selected == Some(i) {
                    selected_item_idx = Some(items.len());
                }
                items.push(MsgItem {
                    is_me: false,
                    lines: vec![line],
                    lines_height: 1,
                    height: 1,
                    is_selected,
                    is_system: true,
                    image_guids: vec![],
                });
            }
            continue;
        }

        let sender = if is_me {
            "Me".to_string()
        } else {
            msg.handle
                .as_ref()
                .map(|h| {
                    main.lookup_contact(&h.address)
                        .cloned()
                        .unwrap_or_else(|| h.address.clone())
                })
                .unwrap_or_else(|| "Unknown".to_string())
        };

        let text = msg.text.clone().unwrap_or_default();
        let max_width = ((inner.width as f32 * 0.6) as usize).max(1);
        let wrapped = textwrap::fill(&text, max_width);
        let text_lines = if text.is_empty() {
            0
        } else {
            wrapped.lines().count().max(1)
        };

        let timestamp = dt.format("%I:%M:%S %p %m/%d/%y").to_string();
        let header = format!("{} \u{00B7} {}", sender, timestamp);

        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(Line::from(Span::styled(
            header,
            Style::default()
                .add_modifier(Modifier::ITALIC)
                .fg(Color::Gray),
        )));
        for line in wrapped.lines() {
            lines.push(Line::from(line.to_string()));
        }

        let mut image_guids: Vec<(String, String)> = Vec::new();
        let mut img_height: usize = 0;
        let mut lines_height = 1 + text_lines;
        if let Some(atts) = &msg.attachments {
            for att in atts {
                if crate::state::is_image_attachment(att) {
                    let img_rows = match main.image_cache.get(&att.guid) {
                        Some(ImageCacheEntry::Ready(proto)) => {
                            let fitted = proto.size_for(
                                Resize::Fit(Some(FilterType::Lanczos3)),
                                Rect::new(0, 0, msg_width_estimate, IMAGE_PREVIEW_ROWS),
                            );
                            (fitted.height as usize).max(1)
                        }
                        _ => 1,
                    };
                    let name = att
                        .transfer_name
                        .clone()
                        .unwrap_or_else(|| "image".to_string());
                    image_guids.push((att.guid.clone(), name));
                    img_height += img_rows;
                } else {
                    lines.push(Line::from(Span::styled(
                        format!("[{}]", att.transfer_name.as_deref().unwrap_or("attachment")),
                        Style::default().fg(Color::DarkGray),
                    )));
                    lines_height += 1;
                }
            }
        }

        let height = (lines_height + img_height).max(1);
        if main.message_selected == Some(i) {
            selected_item_idx = Some(items.len());
        }
        items.push(MsgItem {
            is_me,
            lines,
            lines_height,
            height,
            is_selected,
            is_system: false,
            image_guids,
        });
    }

    let available = inner.height as usize;

    // Bottom-fit: find how many messages fill the pane from the bottom.
    let mut fit: usize = 0;
    let mut bottom_start = items.len();
    for i in (0..items.len()).rev() {
        let needed = items[i].height + 1;
        if fit + needed > available {
            break;
        }
        fit += needed;
        bottom_start = i;
    }

    let (start, y_base) = match selected_item_idx {
        Some(sel_item) if sel_item < bottom_start => {
            // User has scrolled above the default bottom window — use a stable anchor.
            // Fill downward from sel_item first so we know how much space is left above.
            // This prevents the view from jumping when images below the selection load.
            let mut below: usize = 0;
            for item in items.iter().skip(sel_item) {
                let needed = item.height + 1;
                below += needed;
                if below >= available {
                    below = available;
                    break;
                }
            }
            let space_above = available.saturating_sub(below);
            let mut s = sel_item;
            let mut used_above: usize = 0;
            for idx in (0..sel_item).rev() {
                let needed = items[idx].height + 1;
                if used_above + needed > space_above {
                    break;
                }
                used_above += needed;
                s = idx;
            }
            (s, inner.y)
        }
        _ => {
            // Selection is within the default bottom window (or no selection).
            // Keep messages bottom-aligned so newest is at the bottom.
            let y_base = inner.y + inner.height.saturating_sub(fit as u16);
            (bottom_start, y_base)
        }
    };

    let mut y = y_base;
    for item in &items[start..] {
        if y >= inner.y + inner.height {
            break;
        }
        let render_height = (item.height as u16).min(inner.y + inner.height - y);

        if item.is_system {
            let msg_area = Rect::new(inner.x, y, inner.width, render_height);
            f.render_widget(
                Paragraph::new(item.lines.clone())
                    .alignment(Alignment::Center)
                    .style(Style::default()),
                msg_area,
            );
            y += item.height as u16 + 1;
            continue;
        }

        let msg_width = ((inner.width as f32 * 0.7) as u16).max(1);
        let x = if item.is_me {
            inner.x + inner.width.saturating_sub(msg_width)
        } else {
            inner.x
        };

        let bg = if item.is_selected && main.loading_attachment {
            Color::Yellow
        } else if item.is_selected {
            Color::Indexed(236)
        } else {
            Color::Reset
        };

        // Render text lines (header + message body + non-image attachment labels).
        let text_rows = (item.lines_height as u16).min(inner.y + inner.height - y);
        if text_rows > 0 {
            let text_area = Rect::new(x, y, msg_width, text_rows);
            f.render_widget(
                Paragraph::new(item.lines.clone())
                    .style(Style::default().bg(bg))
                    .wrap(Wrap { trim: false }),
                text_area,
            );
        }

        // Render image attachments below the text.
        let mut img_y = y + text_rows;
        for (guid, name) in &item.image_guids {
            if img_y >= inner.y + inner.height {
                break;
            }
            match main.image_cache.get_mut(guid) {
                Some(ImageCacheEntry::Ready(proto)) => {
                    // Only render when the full IMAGE_PREVIEW_ROWS are available.
                    // Passing a smaller area triggers Lanczos3 re-encode every frame → lag.
                    let rows_available = (inner.y + inner.height).saturating_sub(img_y);
                    if rows_available >= IMAGE_PREVIEW_ROWS {
                        let img_area = Rect::new(x, img_y, msg_width, IMAGE_PREVIEW_ROWS);
                        f.render_stateful_widget(
                            StatefulImage::<_>::new()
                                .resize(Resize::Fit(Some(FilterType::Lanczos3))),
                            img_area,
                            proto.as_mut(),
                        );
                    }
                    img_y += IMAGE_PREVIEW_ROWS;
                }
                Some(ImageCacheEntry::Loading) => {
                    f.render_widget(
                        Paragraph::new(Span::styled(
                            "[image loading…]",
                            Style::default().fg(Color::DarkGray),
                        )),
                        Rect::new(x, img_y, msg_width, 1),
                    );
                    img_y += 1;
                }
                Some(ImageCacheEntry::Failed) | None => {
                    f.render_widget(
                        Paragraph::new(Span::styled(
                            format!("[{}]", name),
                            Style::default().fg(Color::DarkGray),
                        )),
                        Rect::new(x, img_y, msg_width, 1),
                    );
                    img_y += 1;
                }
            }
        }

        y += item.height as u16 + 1;
    }

    if matches!(main.focused_pane, FocusedPane::AttachmentPicker) {
        render_attachment_overlay(f, area, main);
    } else if matches!(main.focused_pane, FocusedPane::FileChooser) {
        render_file_chooser_overlay(f, area, main);
    }
}

fn render_file_chooser_overlay(f: &mut Frame, area: Rect, main: &MainState) {
    let overlay = centered_rect(80, 70, area);
    f.render_widget(Clear, overlay);

    let items: Vec<ListItem> = main
        .file_chooser_entries
        .iter()
        .map(|p| {
            let name = if p.ends_with("..") {
                "..".to_string()
            } else {
                p.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            };
            let prefix = if p.is_dir() { "📁 " } else { "📄 " };
            ListItem::new(format!("{}{}", prefix, name))
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(main.file_chooser_selected));

    let title = format!(
        " Select File — {} (Filter: {}) ",
        main.file_chooser_dir.display(),
        main.file_chooser_filter
    );

    f.render_stateful_widget(
        List::new(items)
            .block(Block::default().borders(Borders::ALL).title(title))
            .highlight_style(
                Style::default()
                    .bg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> "),
        overlay,
        &mut list_state,
    );
}

fn render_attachment_overlay(f: &mut Frame, area: Rect, main: &MainState) {
    let overlay = centered_rect(80, 70, area);
    f.render_widget(Clear, overlay);

    let items: Vec<ListItem> = main
        .attachment_items
        .iter()
        .map(|att| {
            let mime = att.mime_type.as_deref().unwrap_or("unknown type");
            ListItem::new(format!("{}  ({})", att.name, mime))
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(main.attachment_selected));

    f.render_stateful_widget(
        List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Attachments — Enter to open · Esc to close "),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> "),
        overlay,
        &mut list_state,
    );
}

fn render_status_bar(f: &mut Frame, area: Rect, main: &MainState) {
    if main.pending_send {
        f.render_widget(
            Paragraph::new(main.compose_text.as_str())
                .style(Style::default().fg(Color::Black).bg(Color::Yellow))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Sending… ")
                        .border_style(Style::default().fg(Color::Yellow)),
                ),
            area,
        );
        return;
    }

    if main.compose_mode {
        let inner_w = area.width.saturating_sub(2) as usize;
        let cursor = main.compose_cursor;
        let display_text = if inner_w == 0 {
            String::new()
        } else {
            main.compose_text
                .split('\n')
                .flat_map(|line| {
                    let lchars: Vec<char> = line.chars().collect();
                    if lchars.is_empty() {
                        vec![String::new()]
                    } else {
                        lchars
                            .chunks(inner_w)
                            .map(|chunk| chunk.iter().collect::<String>())
                            .collect::<Vec<_>>()
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        let (cursor_row, cursor_col) = if inner_w == 0 {
            (0usize, 0usize)
        } else {
            let mut row: usize = 0;
            let mut logical_col: usize = 0;
            for (idx, ch) in main.compose_text.chars().enumerate() {
                if idx == cursor {
                    break;
                }
                if ch == '\n' {
                    let rows_used = if logical_col == 0 {
                        1
                    } else {
                        logical_col.div_ceil(inner_w)
                    };
                    row += rows_used;
                    logical_col = 0;
                } else {
                    logical_col += 1;
                }
            }
            (row + logical_col / inner_w, logical_col % inner_w)
        };
        f.render_widget(
            Paragraph::new(display_text)
                .style(Style::default().fg(Color::Yellow))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Compose — Enter to send · Alt+Enter newline · Esc to cancel · @@ file picker · : emoji picker "),
                ),
            area,
        );
        f.set_cursor_position((
            area.x + 1 + cursor_col as u16,
            area.y + 1 + cursor_row as u16,
        ));
        return;
    }

    if matches!(main.focused_pane, FocusedPane::AttachmentPicker) {
        let (text, style) = match &main.attachment_status {
            Some(s) => (s.as_str(), Style::default().fg(Color::Green)),
            None => (
                "↑↓ navigate · Enter to open · Esc to close",
                Style::default().fg(Color::DarkGray),
            ),
        };
        f.render_widget(
            Paragraph::new(text).style(style).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Attachments "),
            ),
            area,
        );
        return;
    }

    if let Some(err) = &main.send_error {
        f.render_widget(
            Paragraph::new(err.as_str())
                .style(Style::default().fg(Color::Red))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Error — press i to retry "),
                ),
            area,
        );
        return;
    }

    if matches!(main.focused_pane, FocusedPane::Messages) {
        let (text, style) = if let Some(s) = &main.attachment_status {
            (
                s.as_str(),
                if main.loading_attachment {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::Green)
                },
            )
        } else if main.selected_message_has_attachments() {
            (
                "↑↓ select · Enter open attachment · ← chats · i compose",
                Style::default().fg(Color::Yellow),
            )
        } else {
            (
                "↑↓ select · ← chats · i compose",
                Style::default().fg(Color::Yellow),
            )
        };
        f.render_widget(
            Paragraph::new(text)
                .style(style)
                .block(Block::default().borders(Borders::ALL).title(" Messages ")),
            area,
        );
        return;
    }

    f.render_widget(
        Paragraph::new("").block(
            Block::default()
                .borders(Borders::ALL)
                .title(" ↑↓ navigate · → messages · i compose · q quit ")
                .style(Style::default().fg(Color::DarkGray)),
        ),
        area,
    );
}

fn render_chat_search_overlay(f: &mut Frame, area: Rect, main: &MainState) {
    let overlay = centered_rect(80, 70, area);
    f.render_widget(Clear, overlay);

    let items: Vec<ListItem> = main
        .chat_search_results
        .iter()
        .map(|&idx| {
            let chat = &main.chats[idx];
            let name = main.chat_display_name(chat);
            ListItem::new(name)
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(main.chat_search_selected));

    let title = format!(" Search Chats: {} ", main.chat_search_query);

    f.render_stateful_widget(
        List::new(items)
            .block(Block::default().borders(Borders::ALL).title(title))
            .highlight_style(
                Style::default()
                    .bg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> "),
        overlay,
        &mut list_state,
    );
}

fn render_emoji_picker_overlay(f: &mut Frame, area: Rect, main: &MainState) {
    let overlay = centered_rect(60, 40, area);
    f.render_widget(Clear, overlay);

    if let Some(state) = &main.emoji_picker_state {
        let items: Vec<ListItem> = state
            .results
            .iter()
            .map(|e| {
                let shortcode = e
                    .shortcode()
                    .map(|s| format!(":{}:", s))
                    .unwrap_or_else(|| e.name().to_string());
                ListItem::new(format!("{}  {}", e.as_str(), shortcode))
            })
            .collect();

        let mut list_state = ListState::default();
        list_state.select(Some(state.selected));

        let title = format!(" Emoji Picker: {} ", state.query);

        f.render_stateful_widget(
            List::new(items)
                .block(Block::default().borders(Borders::ALL).title(title))
                .highlight_style(
                    Style::default()
                        .bg(Color::Blue)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol(">> "),
            overlay,
            &mut list_state,
        );
    }
}
