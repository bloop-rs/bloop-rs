use crate::config::{default_send_method, save_config_with_method, try_login};
use crate::helpers::{char_to_byte, word_left, word_right};
use crate::state::{ApiResponse, AppState, FocusedPane, MainState};
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui_image::picker::Picker;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

fn percent_decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut bytes = s.bytes().peekable();
    while let Some(b) = bytes.next() {
        if b == b'%' {
            let h1 = bytes.next().and_then(|c| (c as char).to_digit(16));
            let h2 = bytes.next().and_then(|c| (c as char).to_digit(16));
            match (h1, h2) {
                (Some(hi), Some(lo)) => out.push((((hi << 4) | lo) as u8) as char),
                _ => out.push('%'),
            }
        } else {
            out.push(b as char);
        }
    }
    out
}

// Parse one or more space-separated quoted paths as pasted by terminals on file drag-and-drop.
// Accepts bare paths, single-quoted paths, double-quoted paths, and file:// URIs.
// Returns only paths that actually exist as files; returns empty vec if nothing matched.
fn extract_dropped_paths(text: &str) -> Vec<std::path::PathBuf> {
    let mut paths = Vec::new();
    let mut rest = text;
    loop {
        rest = rest.trim_start();
        if rest.is_empty() {
            break;
        }
        let raw = if let Some(inner) = rest.strip_prefix('\'') {
            let end = inner.find('\'').unwrap_or(inner.len());
            rest = inner[end..].trim_start_matches('\'');
            &inner[..end]
        } else if let Some(inner) = rest.strip_prefix('"') {
            let end = inner.find('"').unwrap_or(inner.len());
            rest = inner[end..].trim_start_matches('"');
            &inner[..end]
        } else {
            let end = rest.find(' ').unwrap_or(rest.len());
            let token = &rest[..end];
            rest = &rest[end..];
            token
        };
        let bare = raw
            .strip_prefix("file://")
            .map(percent_decode)
            .unwrap_or_else(|| raw.to_string());
        let p = std::path::Path::new(&bare);
        if p.is_absolute() && p.is_file() {
            paths.push(p.to_path_buf());
        } else {
            return Vec::new();
        }
    }
    paths
}

/// Terminals without keyboard enhancement report Shift+2 as '@' with no modifiers.
/// Terminals with enhancement report '2' + SHIFT. This handles both.
fn resolve_char(c: char, modifiers: KeyModifiers) -> char {
    if c == '2' && modifiers.contains(KeyModifiers::SHIFT) {
        '@'
    } else {
        c
    }
}

/// Returns true if the application should quit.
pub(crate) fn handle_key(
    state: &mut AppState,
    code: KeyCode,
    modifiers: KeyModifiers,
    tx: Sender<ApiResponse>,
    picker: &Picker,
) -> bool {
    match state {
        AppState::Login(fields) => match code {
            KeyCode::Tab | KeyCode::Down => {
                fields.active_field = (fields.active_field + 1) % 3;
            }
            KeyCode::Up => {
                fields.active_field = (fields.active_field + 2) % 3;
            }
            KeyCode::Enter => {
                let host = fields.host.clone();
                let password = fields.password.clone();
                let send_method = if fields.use_private_api {
                    "private-api".to_string()
                } else {
                    default_send_method()
                };
                match try_login(
                    host.clone(),
                    password.clone(),
                    send_method.clone(),
                    picker.clone(),
                ) {
                    Ok(mut main_state) => {
                        save_config_with_method(&host, &password, &send_method);
                        main_state.load_contacts(tx.clone());
                        if !main_state.chats.is_empty() {
                            main_state.load_messages(0, tx.clone());
                        }
                        *state = AppState::Main(Box::new(main_state));
                    }
                    Err(msg) => {
                        if let AppState::Login(f) = state {
                            f.error = Some(msg);
                        }
                    }
                }
            }
            KeyCode::Char(' ') if fields.active_field == 2 => {
                fields.use_private_api = !fields.use_private_api;
            }
            KeyCode::Char(c) => {
                let actual_c = resolve_char(c, modifiers);
                if fields.active_field == 0 {
                    fields.host.push(actual_c);
                } else if fields.active_field == 1 {
                    fields.password.push(actual_c);
                }
            }
            KeyCode::Backspace => {
                if fields.active_field == 0 {
                    fields.host.pop();
                } else if fields.active_field == 1 {
                    fields.password.pop();
                }
            }
            KeyCode::Esc => return true,
            _ => {}
        },
        AppState::Main(main) => return handle_main_key(main, code, modifiers, tx),
    }
    false
}

pub(crate) fn handle_paste(state: &mut AppState, text: String) {
    match state {
        AppState::Login(fields) => {
            if fields.active_field == 0 {
                fields.host.push_str(&text);
            } else if fields.active_field == 1 {
                fields.password.push_str(&text);
            }
        }
        AppState::Main(main) => {
            main.last_activity = Instant::now();
            if !main.tui_notifications.is_empty() && main.notification_fade_start.is_none() {
                main.notification_fade_start = Some(Instant::now());
            }
            let files: Vec<PathBuf> = extract_dropped_paths(text.trim());
            if !files.is_empty() {
                main.pending_file_attachments.extend(files);
                return;
            }
            if main.compose_mode {
                main.pending_at = false;
                let byte_off = char_to_byte(&main.compose_text, main.compose_cursor);
                main.compose_text.insert_str(byte_off, &text);
                main.compose_cursor += text.chars().count();
            }
        }
    }
}

pub(crate) fn handle_main_key(
    main: &mut MainState,
    code: KeyCode,
    modifiers: KeyModifiers,
    tx: Sender<ApiResponse>,
) -> bool {
    let now = Instant::now();
    let is_paste_heuristic = now.duration_since(main.last_activity) < Duration::from_millis(10);
    main.last_activity = now;
    if !main.tui_notifications.is_empty() && main.notification_fade_start.is_none() {
        main.notification_fade_start = Some(Instant::now());
    }

    if matches!(main.focused_pane, FocusedPane::EmojiPicker) {
        match code {
            KeyCode::Esc => {
                main.focused_pane = FocusedPane::Messages;
                main.emoji_picker_state = None;
            }
            KeyCode::Up => {
                if let Some(state) = &mut main.emoji_picker_state
                    && state.selected > 0
                {
                    state.selected -= 1;
                }
            }
            KeyCode::Down => {
                if let Some(state) = &mut main.emoji_picker_state
                    && state.selected + 1 < state.results.len()
                {
                    state.selected += 1;
                }
            }
            KeyCode::Enter => {
                if let Some(state) = main.emoji_picker_state.take() {
                    if let Some(emoji) = state.results.get(state.selected) {
                        let emoji_char = emoji.as_str();
                        let query_len = state.query.chars().count();
                        for _ in 0..=query_len {
                            if main.compose_cursor > 0 {
                                let byte_off =
                                    char_to_byte(&main.compose_text, main.compose_cursor - 1);
                                main.compose_text.remove(byte_off);
                                main.compose_cursor -= 1;
                            }
                        }
                        let byte_off = char_to_byte(&main.compose_text, main.compose_cursor);
                        main.compose_text.insert_str(byte_off, emoji_char);
                        main.compose_cursor += emoji_char.chars().count();
                    }
                    main.focused_pane = FocusedPane::Messages;
                }
            }
            KeyCode::Backspace => {
                if let Some(state) = &mut main.emoji_picker_state {
                    if main.compose_cursor > 0 {
                        let byte_off = char_to_byte(&main.compose_text, main.compose_cursor - 1);
                        main.compose_text.remove(byte_off);
                        main.compose_cursor -= 1;
                    }
                    if state.query.is_empty() {
                        main.emoji_picker_state = None;
                        main.focused_pane = FocusedPane::Messages;
                    } else {
                        state.query.pop();
                        main.update_emoji_search();
                    }
                }
            }
            KeyCode::Char(c) => {
                let actual_c = resolve_char(c, modifiers);
                if actual_c == ' ' {
                    main.emoji_picker_state = None;
                    main.focused_pane = FocusedPane::Messages;
                }
                let byte_off = char_to_byte(&main.compose_text, main.compose_cursor);
                main.compose_text.insert(byte_off, actual_c);
                main.compose_cursor += 1;

                if let Some(state) = &mut main.emoji_picker_state
                    && actual_c != ' '
                {
                    state.query.push(actual_c);
                    main.update_emoji_search();
                }
            }
            _ => {}
        }
        return false;
    }

    if modifiers.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('k') {
        main.focused_pane = FocusedPane::ChatSearch;
        main.chat_search_query.clear();
        main.chat_search_selected = 0;
        main.update_chat_search();
        main.compose_mode = false;
        return false;
    }

    if main.compose_mode || main.pending_send {
        if main.pending_send {
            return false;
        }
        match code {
            KeyCode::Esc => {
                main.pending_at = false;
                main.compose_mode = false;
            }
            KeyCode::Enter if modifiers.contains(KeyModifiers::ALT) || is_paste_heuristic => {
                main.pending_at = false;
                let byte_off = char_to_byte(&main.compose_text, main.compose_cursor);
                main.compose_text.insert(byte_off, '\n');
                main.compose_cursor += 1;
            }
            KeyCode::Enter => {
                if main.pending_at {
                    main.pending_at = false;
                    let byte_off = char_to_byte(&main.compose_text, main.compose_cursor);
                    main.compose_text.insert(byte_off, '@');
                    main.compose_cursor += 1;
                }
                main.pending_send = true;
            }
            KeyCode::Left if modifiers.contains(KeyModifiers::CONTROL) => {
                main.pending_at = false;
                main.compose_cursor = word_left(&main.compose_text, main.compose_cursor);
            }
            KeyCode::Right if modifiers.contains(KeyModifiers::CONTROL) => {
                main.pending_at = false;
                main.compose_cursor = word_right(&main.compose_text, main.compose_cursor);
            }
            KeyCode::Left => {
                main.pending_at = false;
                if main.compose_cursor > 0 {
                    main.compose_cursor -= 1;
                }
            }
            KeyCode::Right => {
                main.pending_at = false;
                let len = main.compose_text.chars().count();
                if main.compose_cursor < len {
                    main.compose_cursor += 1;
                }
            }
            KeyCode::Char(c) => {
                let actual_c = resolve_char(c, modifiers);
                if actual_c == ':' {
                    main.focused_pane = FocusedPane::EmojiPicker;
                    main.emoji_picker_state = Some(crate::state::EmojiPickerState {
                        query: String::new(),
                        selected: 0,
                        results: emojis::iter().collect(),
                    });
                    let byte_off = char_to_byte(&main.compose_text, main.compose_cursor);
                    main.compose_text.insert(byte_off, ':');
                    main.compose_cursor += 1;
                    return false;
                }
                if actual_c == '@' {
                    if main.pending_at {
                        main.pending_at = false;
                        main.focused_pane = FocusedPane::FileChooser;
                        main.file_chooser_dir =
                            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                        main.file_chooser_filter.clear();
                        main.reload_file_chooser();
                        main.compose_mode = false;
                        return false;
                    } else {
                        main.pending_at = true;
                        return false;
                    }
                }
                if main.pending_at {
                    main.pending_at = false;
                    let byte_off = char_to_byte(&main.compose_text, main.compose_cursor);
                    main.compose_text.insert(byte_off, '@');
                    main.compose_cursor += 1;
                }
                let byte_off = char_to_byte(&main.compose_text, main.compose_cursor);
                main.compose_text.insert(byte_off, actual_c);
                main.compose_cursor += 1;
            }
            KeyCode::Backspace => {
                if main.pending_at {
                    main.pending_at = false;
                } else if main.compose_cursor > 0 {
                    let byte_off = char_to_byte(&main.compose_text, main.compose_cursor - 1);
                    main.compose_text.remove(byte_off);
                    main.compose_cursor -= 1;
                }
            }
            _ => {}
        }
        return false;
    }

    match &main.focused_pane {
        FocusedPane::FileChooser => match code {
            KeyCode::Esc => {
                main.focused_pane = FocusedPane::Messages;
                main.compose_mode = true;
                main.file_chooser_filter.clear();
            }
            KeyCode::Up if main.file_chooser_selected > 0 => {
                main.file_chooser_selected -= 1;
            }
            KeyCode::Down if main.file_chooser_selected + 1 < main.file_chooser_entries.len() => {
                main.file_chooser_selected += 1;
            }
            KeyCode::Enter => {
                if let Some(path) = main
                    .file_chooser_entries
                    .get(main.file_chooser_selected)
                    .cloned()
                {
                    if path.is_dir() {
                        main.file_chooser_dir = std::fs::canonicalize(&path).unwrap_or(path);
                        main.file_chooser_filter.clear();
                        main.reload_file_chooser();
                    } else {
                        main.pending_file_attachments.push(path);
                        main.focused_pane = FocusedPane::Messages;
                        main.compose_mode = false;
                    }
                }
            }
            KeyCode::Backspace if !main.file_chooser_filter.is_empty() => {
                main.file_chooser_filter.pop();
                main.reload_file_chooser();
            }
            KeyCode::Char(c) => {
                let actual_c = resolve_char(c, modifiers);
                main.file_chooser_filter.push(actual_c);
                main.reload_file_chooser();
            }
            _ => {}
        },

        FocusedPane::AttachmentPicker => match code {
            KeyCode::Esc => {
                main.focused_pane = FocusedPane::Messages;
                main.attachment_status = None;
            }
            KeyCode::Up if main.attachment_selected > 0 => {
                main.attachment_selected -= 1;
            }
            KeyCode::Down if main.attachment_selected + 1 < main.attachment_items.len() => {
                main.attachment_selected += 1;
            }
            KeyCode::Enter => main.queue_picker_attachment(),
            _ => {}
        },

        FocusedPane::Messages => match code {
            KeyCode::Esc | KeyCode::Left | KeyCode::Char('h') => {
                main.focused_pane = FocusedPane::Chats;
                main.attachment_status = None;
            }
            KeyCode::Up | KeyCode::Char('k') => main.navigate_message_up(tx),
            KeyCode::Down | KeyCode::Char('j') => main.navigate_message_down(),
            KeyCode::Enter => main.queue_selected_message_attachment(),
            KeyCode::Char('i') => {
                main.compose_mode = true;
                main.compose_cursor = main.compose_text.chars().count();
                main.send_error = None;
                main.attachment_status = None;
            }
            KeyCode::Char('q') => {
                main.focused_pane = FocusedPane::Chats;
                main.attachment_status = None;
            }
            _ => {}
        },

        FocusedPane::Chats => match code {
            KeyCode::Char('q') | KeyCode::Esc => return true,
            KeyCode::Up | KeyCode::Char('k') => main.previous_chat(),
            KeyCode::Down | KeyCode::Char('j') => main.next_chat(),
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Enter => main.enter_messages_pane(),
            KeyCode::Char('i') => {
                main.compose_mode = true;
                main.compose_cursor = main.compose_text.chars().count();
                main.send_error = None;
            }
            _ => {}
        },

        FocusedPane::EmojiPicker => {}

        FocusedPane::ChatSearch => match code {
            KeyCode::Esc => {
                main.focused_pane = FocusedPane::Chats;
            }
            KeyCode::Up if main.chat_search_selected > 0 => {
                main.chat_search_selected -= 1;
            }
            KeyCode::Down if main.chat_search_selected + 1 < main.chat_search_results.len() => {
                main.chat_search_selected += 1;
            }
            KeyCode::Enter => {
                if let Some(&chat_idx) = main.chat_search_results.get(main.chat_search_selected) {
                    main.navigate(chat_idx);
                    main.focused_pane = FocusedPane::Messages;
                }
            }
            KeyCode::Backspace => {
                main.chat_search_query.pop();
                main.update_chat_search();
            }
            KeyCode::Char(c) => {
                let actual_c = resolve_char(c, modifiers);
                main.chat_search_query.push(actual_c);
                main.update_chat_search();
            }
            _ => {}
        },
    }
    false
}
