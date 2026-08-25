use crate::helpers::{log_error, normalize_address, open_file, send_notification};
use bluebubbles_api::BlueBubblesApi;
use bluebubbles_api::types::{
    Chat, ChatQuery, ContactData, Handle, Message, MessageQuery, SendText,
};
use ratatui::widgets::ListState;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::{Resize, ResizeEncodeRender};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::thread;
use std::time::{Duration, Instant};

pub(crate) const CHAT_RELOAD_INTERVAL: Duration = Duration::from_secs(30);
pub(crate) const MESSAGE_RELOAD_INTERVAL: Duration = Duration::from_secs(3);
pub(crate) const DEBOUNCE_DURATION: Duration = Duration::from_millis(200);
pub(crate) const MESSAGE_PAGE_SIZE: u64 = 50;
pub(crate) const STATUS_CLEAR_SECS: u64 = 3;

pub(crate) enum AppState {
    Login(LoginFields),
    Main(Box<MainState>),
}

pub(crate) struct LoginFields {
    pub(crate) host: String,
    pub(crate) password: String,
    pub(crate) use_private_api: bool,
    pub(crate) active_field: usize,
    pub(crate) error: Option<String>,
}

pub(crate) enum FocusedPane {
    Chats,
    Messages,
    AttachmentPicker,
    FileChooser,
    ChatSearch,
    EmojiPicker,
}

pub(crate) struct EmojiPickerState {
    pub(crate) query: String,
    pub(crate) selected: usize,
    pub(crate) results: Vec<&'static emojis::Emoji>,
}

pub(crate) struct AttachmentItem {
    pub(crate) guid: String,
    pub(crate) name: String,
    pub(crate) mime_type: Option<String>,
}

pub(crate) enum ImageCacheEntry {
    Loading,
    Ready(Box<StatefulProtocol>),
    Failed,
}

#[derive(Clone)]
pub(crate) struct TuiNotification {
    pub(crate) title: String,
    pub(crate) bodies: Vec<String>,
}

pub enum ApiResponse {
    ChatsLoaded(Result<Vec<Chat>, String>),
    MessagesLoaded {
        chat_guid: String,
        messages: Result<Vec<Message>, String>,
    },
    MoreMessagesLoaded {
        chat_guid: String,
        count: usize,
        older: Result<Vec<Message>, String>,
    },
    ContactsLoaded(Result<Vec<ContactData>, String>),
    SpecificContactsLoaded(Result<Vec<ContactData>, String>),
    AttachmentDownloaded {
        name: String,
        result: Result<PathBuf, String>,
    },
    AttachmentSent(Result<(), String>),
    TextSent(Result<(), String>),
    ImageDownloaded {
        guid: String,
        result: Result<Box<StatefulProtocol>, String>,
    },
}

fn safe_attachment_file_name(name: &str, guid: &str) -> String {
    let candidate = name.rsplit(['/', '\\']).next().unwrap_or("").trim();
    let candidate = if candidate.is_empty() || candidate == "." || candidate == ".." {
        guid
    } else {
        candidate
    };

    candidate
        .chars()
        .map(|c| if c == '\0' { '_' } else { c })
        .collect()
}

pub(crate) struct MainState {
    pub(crate) api: BlueBubblesApi,
    pub(crate) chats: Vec<Chat>,
    pub(crate) chat_list_state: ListState,
    pub(crate) messages: Vec<Message>,
    pub(crate) selected_chat_index: Option<usize>,
    pub(crate) contacts: HashMap<String, String>,
    pub(crate) focused_pane: FocusedPane,
    pub(crate) message_selected: Option<usize>,
    pub(crate) compose_mode: bool,
    pub(crate) compose_text: String,
    pub(crate) compose_cursor: usize,
    pub(crate) send_error: Option<String>,
    pub(crate) last_chat_reload: Instant,
    pub(crate) last_message_reload: Instant,
    pub(crate) pending_load: Option<usize>,
    pub(crate) last_nav: Instant,
    pub(crate) attachment_items: Vec<AttachmentItem>,
    pub(crate) attachment_selected: usize,
    pub(crate) attachment_status: Option<String>,
    pub(crate) has_more_messages: bool,
    pub(crate) status_expires: Option<Instant>,
    pub(crate) pending_attachment_open: Option<(String, String)>,
    pub(crate) loading_attachment: bool,
    pub(crate) pending_send: bool,
    pub(crate) pending_at: bool,
    pub(crate) pending_file_attachments: Vec<PathBuf>,
    pub(crate) file_chooser_dir: PathBuf,
    pub(crate) file_chooser_entries: Vec<PathBuf>,
    pub(crate) file_chooser_selected: usize,
    pub(crate) file_chooser_filter: String,
    pub(crate) chat_search_query: String,
    pub(crate) chat_search_results: Vec<usize>,
    pub(crate) chat_search_selected: usize,
    pub(crate) emoji_picker_state: Option<EmojiPickerState>,
    pub(crate) send_method: String,
    pub(crate) last_activity: Instant,
    pub(crate) last_seen_guids: HashMap<String, String>,
    pub(crate) unread_chats: HashSet<String>,
    pub(crate) terminal_focused: bool,
    pub(crate) drafts: HashMap<String, (String, usize)>,
    pub(crate) api_requests_in_flight: usize,
    pub(crate) tui_notifications: HashMap<String, TuiNotification>,
    pub(crate) notification_fade_start: Option<Instant>,
    pub(crate) image_picker: Picker,
    pub(crate) image_cache: HashMap<String, ImageCacheEntry>,
    pub(crate) pending_image_downloads: Vec<(String, String)>,
}

impl MainState {
    // ── Navigation ────────────────────────────────────────────────────────

    pub(crate) fn navigate(&mut self, index: usize) {
        // Save draft for the chat we're leaving.
        if let Some(current_guid) = self
            .selected_chat_index
            .and_then(|i| self.chats.get(i))
            .map(|c| c.guid.clone())
        {
            if self.compose_text.is_empty() {
                self.drafts.remove(&current_guid);
            } else {
                self.drafts.insert(
                    current_guid,
                    (self.compose_text.clone(), self.compose_cursor),
                );
            }
        }

        self.chat_list_state.select(Some(index));
        self.selected_chat_index = Some(index);
        self.messages = Vec::new();
        self.message_selected = None;
        self.attachment_status = None;
        self.has_more_messages = true;
        self.pending_load = Some(index);
        self.last_nav = Instant::now();

        // Restore draft for the chat we're entering.
        if let Some(chat) = self.chats.get(index) {
            self.unread_chats.remove(&chat.guid);
            if let Some((text, cursor)) = self.drafts.get(&chat.guid).cloned() {
                self.compose_text = text;
                self.compose_cursor = cursor;
            } else {
                self.compose_text.clear();
                self.compose_cursor = 0;
            }
            self.compose_mode = false;
        }
    }

    pub(crate) fn next_chat(&mut self) {
        if self.chats.is_empty() {
            return;
        }
        let i = match self.chat_list_state.selected() {
            Some(i) => {
                if i >= self.chats.len() - 1 {
                    return;
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.navigate(i);
    }

    pub(crate) fn previous_chat(&mut self) {
        if self.chats.is_empty() {
            return;
        }
        let i = match self.chat_list_state.selected() {
            Some(i) => {
                if i == 0 {
                    return;
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.navigate(i);
    }

    pub(crate) fn enter_messages_pane(&mut self) {
        if self.messages.is_empty() {
            return;
        }
        if self.message_selected.is_none() {
            self.message_selected = Some(self.messages.len().saturating_sub(1));
        }
        self.focused_pane = FocusedPane::Messages;
    }

    pub(crate) fn navigate_message_up(&mut self, tx: Sender<ApiResponse>) {
        if let Some(sel) = self.message_selected {
            if sel > 0 {
                self.message_selected = Some(sel - 1);
            } else if self.has_more_messages {
                self.load_more_messages(tx);
            }
        }
    }

    pub(crate) fn navigate_message_down(&mut self) {
        if let Some(sel) = self.message_selected
            && sel + 1 < self.messages.len()
        {
            self.message_selected = Some(sel + 1);
        }
    }

    // ── Message loading ───────────────────────────────────────────────────

    pub(crate) fn load_messages(&mut self, index: usize, tx: Sender<ApiResponse>) {
        if let Some(chat) = self.chats.get(index) {
            self.api_requests_in_flight += 1;
            let api = self.api.clone();
            let chat_guid = chat.guid.clone();
            let query = MessageQuery {
                chat_guid: Some(chat.guid.clone()),
                limit: Some(MESSAGE_PAGE_SIZE),
                sort: Some("DESC".to_string()),
                with_related: Some(vec!["attachment".to_string()]),
                ..Default::default()
            };
            thread::spawn(move || {
                let res = api.query_messages(query).map_err(|e| e.to_string());
                let _ = tx.send(ApiResponse::MessagesLoaded {
                    chat_guid,
                    messages: res,
                });
            });
        }
    }

    fn load_more_messages(&mut self, tx: Sender<ApiResponse>) {
        let Some(idx) = self.selected_chat_index else {
            return;
        };
        let Some(chat) = self.chats.get(idx) else {
            return;
        };
        let Some(oldest) = self.messages.first() else {
            return;
        };

        self.api_requests_in_flight += 1;
        let before_ts = oldest.date_created;
        let api = self.api.clone();
        let chat_guid = chat.guid.clone();
        let query = MessageQuery {
            chat_guid: Some(chat.guid.clone()),
            limit: Some(MESSAGE_PAGE_SIZE),
            sort: Some("DESC".to_string()),
            with_related: Some(vec!["attachment".to_string()]),
            before: Some(before_ts),
            ..Default::default()
        };

        thread::spawn(move || {
            let res = api.query_messages(query).map_err(|e| e.to_string());
            let count = match &res {
                Ok(m) => m.len(),
                Err(_) => 0,
            };
            let _ = tx.send(ApiResponse::MoreMessagesLoaded {
                chat_guid,
                count,
                older: res,
            });
        });
    }

    pub(crate) fn reload_chats(&mut self, tx: Sender<ApiResponse>) {
        self.api_requests_in_flight += 1;
        let api = self.api.clone();
        thread::spawn(move || {
            let res = api
                .query_chats(ChatQuery {
                    limit: Some(1000),
                    with_related: Some(vec![
                        "lastMessage".to_string(),
                        "participants".to_string(),
                        "sms".to_string(),
                        "archived".to_string(),
                    ]),
                    sort: Some("lastmessage".to_string()),
                    ..Default::default()
                })
                .map_err(|e| e.to_string());
            let _ = tx.send(ApiResponse::ChatsLoaded(res));
        });
    }

    fn check_new_messages_and_notify(&mut self) {
        let app_focused = self.terminal_focused;

        let mut seeds: Vec<(String, String)> = Vec::new();
        let mut updates: Vec<(String, String, bool, String, String, bool)> = Vec::new();

        for chat in &self.chats {
            let Some(last_msg) = &chat.last_message else {
                continue;
            };
            let new_guid = &last_msg.guid;

            match self.last_seen_guids.get(&chat.guid) {
                None => {
                    seeds.push((chat.guid.clone(), new_guid.clone()));
                }
                Some(old_guid) if old_guid != new_guid => {
                    let from_contact = !last_msg.is_from_me;
                    let title = self.chat_display_name(chat);
                    let body = last_msg
                        .text
                        .as_deref()
                        .filter(|s| !s.is_empty())
                        .unwrap_or("New attachment")
                        .to_string();
                    updates.push((
                        chat.guid.clone(),
                        new_guid.clone(),
                        !app_focused && from_contact,
                        title,
                        body,
                        from_contact,
                    ));
                }
                _ => {}
            }
        }

        for (chat_guid, msg_guid) in seeds {
            self.last_seen_guids.insert(chat_guid, msg_guid);
        }

        let now = Instant::now();
        let is_idle = now.duration_since(self.last_activity) > Duration::from_secs(15);

        let current_chat_guid = self
            .selected_chat_index
            .and_then(|i| self.chats.get(i))
            .map(|c| c.guid.clone());

        for (chat_guid, new_msg_guid, notify, title, body, from_contact) in updates {
            self.last_seen_guids.insert(chat_guid.clone(), new_msg_guid);
            if notify {
                self.unread_chats.insert(chat_guid.clone());
                send_notification(&title, &body);
            }
            let is_current_chat = current_chat_guid.as_ref() == Some(&chat_guid);
            if from_contact && is_idle && self.notification_fade_start.is_none() && !is_current_chat
            {
                let entry = self.tui_notifications.entry(chat_guid).or_insert_with(|| {
                    crate::state::TuiNotification {
                        title,
                        bodies: Vec::new(),
                    }
                });
                entry.bodies.push(body);
            }
        }
    }

    pub(crate) fn load_contacts(&mut self, tx: Sender<ApiResponse>) {
        self.api_requests_in_flight += 1;
        let api = self.api.clone();
        let tx_contacts = tx.clone();
        thread::spawn(move || {
            let res = api.get_contacts().map_err(|e| e.to_string());
            let _ = tx_contacts.send(ApiResponse::ContactsLoaded(res));
        });

        let mut address_set = HashSet::new();
        for chat in &self.chats {
            if let Some(participants) = &chat.participants {
                for handle in participants {
                    address_set.insert(handle.address.clone());
                }
            }
        }
        let addresses: Vec<String> = address_set.into_iter().collect();
        if addresses.is_empty() {
            return;
        }

        self.api_requests_in_flight += 1;
        let api_clone = self.api.clone();
        thread::spawn(move || {
            let res = api_clone
                .query_contacts(addresses)
                .map_err(|e| e.to_string());
            let _ = tx.send(ApiResponse::SpecificContactsLoaded(res));
        });
    }

    pub(crate) fn lookup_contact(&self, address: &str) -> Option<&String> {
        self.contacts
            .get(address)
            .or_else(|| self.contacts.get(&normalize_address(address)))
    }

    // ── Attachment handling ───────────────────────────────────────────────

    pub(crate) fn queue_selected_message_attachment(&mut self) {
        let Some(sel) = self.message_selected else {
            return;
        };

        let attachments: Vec<(String, String, Option<String>)> = self
            .messages
            .get(sel)
            .and_then(|m| m.attachments.as_ref())
            .map(|atts| {
                atts.iter()
                    .map(|a| {
                        (
                            a.guid.clone(),
                            a.transfer_name.clone().unwrap_or_default(),
                            a.mime_type.clone(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();

        if attachments.is_empty() {
            return;
        }

        if attachments.len() == 1 {
            let (guid, name, _) = attachments.into_iter().next().unwrap();
            self.pending_attachment_open = Some((guid, name));
            self.loading_attachment = true;
        } else {
            self.attachment_items = attachments
                .into_iter()
                .map(|(guid, name, mime_type)| AttachmentItem {
                    guid,
                    name,
                    mime_type,
                })
                .collect();
            self.attachment_selected = 0;
            self.attachment_status = None;
            self.focused_pane = FocusedPane::AttachmentPicker;
        }
    }

    pub(crate) fn queue_picker_attachment(&mut self) {
        let Some(item) = self.attachment_items.get(self.attachment_selected) else {
            return;
        };
        let guid = item.guid.clone();
        let name = item.name.clone();
        self.pending_attachment_open = Some((guid, name));
        self.loading_attachment = true;
    }

    pub(crate) fn execute_download(&mut self, guid: &str, name: &str, tx: Sender<ApiResponse>) {
        self.api_requests_in_flight += 1;
        let api = self.api.clone();
        let guid_clone = guid.to_string();
        let name_clone = name.to_string();
        thread::spawn(move || {
            let res = api
                .download_attachment(&guid_clone)
                .map_err(|e| e.to_string());
            let final_res = match res {
                Ok(bytes) => {
                    let tmp_dir = std::env::temp_dir().join("bloop");
                    let _ = std::fs::create_dir_all(&tmp_dir);
                    let safe_name = safe_attachment_file_name(&name_clone, &guid_clone);
                    let file_path = tmp_dir.join(safe_name);
                    match std::fs::write(&file_path, &bytes) {
                        Ok(_) => Ok(file_path),
                        Err(e) => Err(format!("Error saving: {}", e)),
                    }
                }
                Err(e) => Err(format!("Download failed: {}", e)),
            };
            let _ = tx.send(ApiResponse::AttachmentDownloaded {
                name: name_clone,
                result: final_res,
            });
        });
    }

    pub(crate) fn execute_image_download(
        &mut self,
        guid: String,
        _name: String,
        tx: Sender<ApiResponse>,
    ) {
        if self.image_cache.contains_key(&guid) {
            return;
        }
        self.image_cache
            .insert(guid.clone(), ImageCacheEntry::Loading);
        let api = self.api.clone();
        let picker = self.image_picker.clone();
        thread::spawn(move || {
            let result = api
                .download_attachment(&guid)
                .map_err(|e| e.to_string())
                .and_then(|bytes| image::load_from_memory(&bytes).map_err(|e| e.to_string()))
                .map(|img| {
                    let mut proto = picker.new_resize_protocol(img);
                    // Pre-encode at estimated render area so the render thread doesn't block.
                    // Mirrors the 30/70 layout split and 70% message width from render.rs.
                    if let Ok((term_cols, _)) = crossterm::terminal::size() {
                        let right_width = (term_cols as f32 * 0.70) as u16;
                        let inner_width = right_width.saturating_sub(2);
                        let msg_width = ((inner_width as f32 * 0.70) as u16).max(1);
                        let area = ratatui::layout::Rect::new(0, 0, msg_width, 10);
                        proto.resize_encode(
                            &Resize::Fit(Some(image::imageops::FilterType::Lanczos3)),
                            area,
                        );
                    }
                    Box::new(proto)
                });
            let _ = tx.send(ApiResponse::ImageDownloaded { guid, result });
        });
    }

    fn queue_image_downloads(&mut self, messages: &[Message]) {
        for msg in messages {
            if let Some(atts) = &msg.attachments {
                for att in atts {
                    if !self.image_cache.contains_key(&att.guid) && is_image_attachment(att) {
                        let name = att
                            .transfer_name
                            .clone()
                            .unwrap_or_else(|| att.guid.clone());
                        self.pending_image_downloads.push((att.guid.clone(), name));
                    }
                }
            }
        }
    }

    // ── File Chooser ──────────────────────────────────────────────────────

    pub(crate) fn reload_file_chooser(&mut self) {
        self.file_chooser_entries.clear();
        let mut dirs = Vec::new();
        let mut files = Vec::new();

        let filter = self.file_chooser_filter.to_lowercase();
        if (filter.is_empty() || "..".contains(&filter)) && self.file_chooser_dir.parent().is_some()
        {
            self.file_chooser_entries
                .push(self.file_chooser_dir.join(".."));
        }

        if let Ok(entries) = std::fs::read_dir(&self.file_chooser_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_lowercase();
                if !filter.is_empty() && !name.contains(&filter) {
                    continue;
                }
                if path.is_dir() {
                    dirs.push(path);
                } else {
                    files.push(path);
                }
            }
        }
        dirs.sort();
        files.sort();
        self.file_chooser_entries.extend(dirs);
        self.file_chooser_entries.extend(files);
        self.file_chooser_selected = 0;
    }

    pub(crate) fn execute_send_file(&mut self, path: PathBuf, tx: Sender<ApiResponse>) {
        let Some(chat) = self.selected_chat_index.and_then(|i| self.chats.get(i)) else {
            self.send_error = Some("No chat selected".to_string());
            return;
        };
        let chat_guid = chat.guid.clone();

        let file_name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let content = match std::fs::read(&path) {
            Ok(c) => c,
            Err(e) => {
                self.send_error = Some(format!("Failed to read file: {}", e));
                return;
            }
        };

        let temp_guid = format!(
            "temp-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );

        self.api_requests_in_flight += 1;
        let api = self.api.clone();
        let send_method = self.send_method.clone();
        thread::spawn(move || {
            let res = api
                .send_attachment(
                    &chat_guid,
                    &file_name,
                    content,
                    Some(&temp_guid),
                    Some(&send_method),
                    false,
                )
                .map(|_| ())
                .map_err(|e| e.to_string());
            let _ = tx.send(ApiResponse::AttachmentSent(res));
        });
    }

    // ── Sending ───────────────────────────────────────────────────────────

    pub(crate) fn execute_send(&mut self, tx: Sender<ApiResponse>) {
        let text = self.compose_text.trim().to_string();
        if text.is_empty() {
            self.compose_mode = false;
            return;
        }

        let chat_guid = match self.selected_chat_index.and_then(|i| self.chats.get(i)) {
            Some(chat) => chat.guid.clone(),
            None => {
                self.send_error = Some("No chat selected".to_string());
                self.compose_mode = false;
                return;
            }
        };

        // Log the in-flight message for recovery purposes
        let log_file = std::env::temp_dir().join("bloop-inflight.log");
        use std::io::Write;
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)
        {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let _ = writeln!(file, "[{}] To {}: {}", timestamp, chat_guid, text);
        }

        // Clear immediately so the user can keep drafting from zero
        self.compose_text.clear();
        self.compose_cursor = 0;
        self.compose_mode = true;

        let temp_guid = format!(
            "temp-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );

        self.api_requests_in_flight += 1;
        let api = self.api.clone();
        let send_text = SendText {
            chat_guid,
            message: text,
            method: self.send_method.clone(),
            temp_guid: Some(temp_guid),
            subject: None,
            effect_id: None,
            selected_message_guid: None,
        };

        thread::spawn(move || {
            let res = api
                .send_text(send_text)
                .map(|_| ())
                .map_err(|e| e.to_string());
            let _ = tx.send(ApiResponse::TextSent(res));
        });
    }

    fn after_send_success(&mut self) {
        self.send_error = None;
        if let Some(guid) = self
            .selected_chat_index
            .and_then(|i| self.chats.get(i))
            .map(|c| c.guid.clone())
        {
            self.drafts.remove(&guid);
        }
        // Instead of reloading synchronously, we will just rely on the next message poll,
        // or trigger a reload async but without blocking. For simplicity we don't trigger it right here.
        // Actually wait, let's let the polling logic catch it, or if we really need it we can pass `tx`.
        // Let's just set the message reload timer so it fetches immediately on the next loop tick.
        self.last_message_reload = Instant::now() - MESSAGE_RELOAD_INTERVAL;
    }

    // ── Display names ─────────────────────────────────────────────────────

    pub(crate) fn update_chat_search(&mut self) {
        let query = self.chat_search_query.to_lowercase();
        self.chat_search_results = self
            .chats
            .iter()
            .enumerate()
            .filter_map(|(idx, chat)| {
                let name = self.chat_display_name(chat).to_lowercase();
                if name.contains(&query) {
                    return Some(idx);
                }
                if let Some(participants) = &chat.participants {
                    for p in participants {
                        if p.address.to_lowercase().contains(&query) {
                            return Some(idx);
                        }
                        if let Some(contact_name) = self.lookup_contact(&p.address)
                            && contact_name.to_lowercase().contains(&query)
                        {
                            return Some(idx);
                        }
                    }
                }
                None
            })
            .collect();

        if self.chat_search_selected >= self.chat_search_results.len() {
            self.chat_search_selected = self.chat_search_results.len().saturating_sub(1);
        }
    }

    pub(crate) fn update_emoji_search(&mut self) {
        if let Some(state) = &mut self.emoji_picker_state {
            let q = state.query.to_lowercase();
            state.results = emojis::iter()
                .filter(|e| {
                    e.name().to_lowercase().contains(&q)
                        || e.shortcode()
                            .map(|s| s.to_lowercase().contains(&q))
                            .unwrap_or(false)
                })
                .collect();
            if state.selected >= state.results.len() {
                state.selected = state.results.len().saturating_sub(1);
            }
        }
    }

    fn group_display_name(&self, participants: &[Handle]) -> String {
        let mut known: Vec<String> = Vec::new();
        let mut unknown_count: usize = 0;

        for handle in participants {
            if let Some(name) = self.lookup_contact(&handle.address) {
                let first = name.split_whitespace().next().unwrap_or(name).to_string();
                known.push(first);
            } else {
                unknown_count += 1;
            }
        }

        if unknown_count == 0 {
            match known.len() {
                0 => String::new(),
                1 => known.remove(0),
                2 => format!("{} & {}", known[0], known[1]),
                3 => format!("{}, {}, & {}", known[0], known[1], known[2]),
                4 => format!("{}, {}, {}, & {}", known[0], known[1], known[2], known[3]),
                n => format!(
                    "{}, {}, {}, & {} others",
                    known[0],
                    known[1],
                    known[2],
                    n - 3
                ),
            }
        } else {
            let others = unknown_count + known.len().saturating_sub(3);
            match known.len().min(3) {
                0 => format!("{} others", others),
                1 => format!("{} & {} others", known[0], others),
                2 => format!("{}, {} & {} others", known[0], known[1], others),
                _ => format!(
                    "{}, {}, {} & {} others",
                    known[0], known[1], known[2], others
                ),
            }
        }
    }

    pub(crate) fn chat_display_name(&self, chat: &Chat) -> String {
        chat.display_name
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                let participants = chat.participants.as_deref().unwrap_or(&[]);
                if participants.len() >= 2 {
                    let name = self.group_display_name(participants);
                    if !name.is_empty() {
                        return name;
                    }
                }
                participants
                    .first()
                    .map(|h| {
                        self.lookup_contact(&h.address)
                            .cloned()
                            .unwrap_or_else(|| h.address.clone())
                    })
                    .unwrap_or_else(|| chat.chat_identifier.clone())
            })
    }

    pub(crate) fn selected_message_has_attachments(&self) -> bool {
        self.message_selected
            .and_then(|i| self.messages.get(i))
            .and_then(|m| m.attachments.as_ref())
            .map(|a| !a.is_empty())
            .unwrap_or(false)
    }

    pub(crate) fn handle_api_response(&mut self, response: ApiResponse) {
        if !matches!(response, ApiResponse::ImageDownloaded { .. }) {
            self.api_requests_in_flight = self.api_requests_in_flight.saturating_sub(1);
        }
        match response {
            ApiResponse::ChatsLoaded(res) => {
                match res {
                    Ok(chats) => {
                        let current_guid = self
                            .selected_chat_index
                            .and_then(|i| self.chats.get(i))
                            .map(|c| c.guid.clone());

                        self.chats = chats;
                        let new_idx =
                            current_guid.and_then(|g| self.chats.iter().position(|c| c.guid == g));
                        self.selected_chat_index = new_idx;
                        self.chat_list_state.select(new_idx);
                        self.check_new_messages_and_notify();
                        // Note: we might want to trigger load_contacts if we don't have them yet,
                        // but doing it every chat reload isn't strictly necessary or we can handle it differently.
                    }
                    Err(e) => {
                        log_error(&format!("[bluebubbles] reload_chats error: {}", e));
                    }
                }
            }
            ApiResponse::MessagesLoaded {
                chat_guid,
                messages,
            } => match messages {
                Ok(mut msgs) => {
                    let current_guid = self
                        .selected_chat_index
                        .and_then(|i| self.chats.get(i))
                        .map(|c| c.guid.clone());

                    if current_guid.as_ref() == Some(&chat_guid) {
                        msgs.reverse();

                        // If the user has loaded historical messages beyond one page, preserve
                        // them. A periodic MessagesLoaded response only carries the latest page
                        // and must not discard history or clamp message_selected to the bottom.
                        if self.messages.len() > MESSAGE_PAGE_SIZE as usize {
                            let existing: HashSet<String> =
                                self.messages.iter().map(|m| m.guid.clone()).collect();
                            let new_msgs: Vec<Message> = msgs
                                .into_iter()
                                .filter(|m| !existing.contains(&m.guid))
                                .collect();
                            if !new_msgs.is_empty() {
                                self.queue_image_downloads(&new_msgs);
                                self.messages.extend(new_msgs);
                            }
                            // has_more_messages stays as set by MoreMessagesLoaded.
                        } else {
                            self.has_more_messages = msgs.len() as u64 >= MESSAGE_PAGE_SIZE;
                            self.queue_image_downloads(&msgs);
                            self.messages = msgs;
                            if let Some(sel) = self.message_selected {
                                if self.messages.is_empty() {
                                    self.message_selected = None;
                                } else if sel >= self.messages.len() {
                                    self.message_selected = Some(self.messages.len() - 1);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    log_error(&format!("[bluebubbles] load_messages error: {}", e));
                    let current_guid = self
                        .selected_chat_index
                        .and_then(|i| self.chats.get(i))
                        .map(|c| c.guid.clone());

                    if current_guid.as_ref() == Some(&chat_guid) {
                        self.messages = Vec::new();
                        self.message_selected = None;
                    }
                }
            },
            ApiResponse::MoreMessagesLoaded {
                chat_guid,
                count,
                older,
            } => match older {
                Ok(mut older_msgs) => {
                    let current_guid = self
                        .selected_chat_index
                        .and_then(|i| self.chats.get(i))
                        .map(|c| c.guid.clone());

                    if current_guid.as_ref() == Some(&chat_guid) {
                        older_msgs.reverse();
                        if (count as u64) < MESSAGE_PAGE_SIZE {
                            self.has_more_messages = false;
                        }
                        // The API's `before` param can return the boundary message again.
                        // Deduplicate so the oldest visible message isn't repeated.
                        let existing: HashSet<String> =
                            self.messages.iter().map(|m| m.guid.clone()).collect();
                        older_msgs.retain(|m| !existing.contains(&m.guid));
                        let prepend_count = older_msgs.len();
                        if let Some(sel) = self.message_selected {
                            self.message_selected = Some(sel + prepend_count);
                        }
                        self.queue_image_downloads(&older_msgs);
                        older_msgs.append(&mut self.messages);
                        self.messages = older_msgs;
                    }
                }
                Err(e) => {
                    log_error(&format!("[bluebubbles] load_more_messages error: {}", e));
                    let current_guid = self
                        .selected_chat_index
                        .and_then(|i| self.chats.get(i))
                        .map(|c| c.guid.clone());

                    if current_guid.as_ref() == Some(&chat_guid) {
                        self.has_more_messages = false;
                    }
                }
            },
            ApiResponse::ContactsLoaded(res) => {
                if let Ok(all_contacts) = res {
                    for contact in &all_contacts {
                        if let Some(name) = contact.best_name() {
                            for p in &contact.phone_numbers {
                                let norm = normalize_address(&p.address);
                                self.contacts.insert(p.address.clone(), name.clone());
                                self.contacts.insert(norm, name.clone());
                            }
                            for e in &contact.emails {
                                let norm = normalize_address(&e.address);
                                self.contacts.insert(e.address.clone(), name.clone());
                                self.contacts.insert(norm, name.clone());
                            }
                        }
                    }
                }
            }
            ApiResponse::SpecificContactsLoaded(res) => {
                if let Ok(contacts) = res {
                    for contact in contacts {
                        if let Some(name) = contact.best_name() {
                            for p in &contact.phone_numbers {
                                let norm = normalize_address(&p.address);
                                self.contacts
                                    .entry(p.address.clone())
                                    .or_insert_with(|| name.clone());
                                self.contacts.entry(norm).or_insert_with(|| name.clone());
                            }
                            for e in &contact.emails {
                                let norm = normalize_address(&e.address);
                                self.contacts
                                    .entry(e.address.clone())
                                    .or_insert_with(|| name.clone());
                                self.contacts.entry(norm).or_insert_with(|| name.clone());
                            }
                        }
                    }
                }
            }
            ApiResponse::AttachmentDownloaded { name, result } => {
                self.loading_attachment = false;
                match result {
                    Ok(file_path) => {
                        open_file(&file_path);
                        self.attachment_status = Some(format!("Opened {}", name));
                        self.status_expires =
                            Some(Instant::now() + Duration::from_secs(STATUS_CLEAR_SECS));
                    }
                    Err(err) => {
                        self.attachment_status = Some(err);
                    }
                }
            }
            ApiResponse::AttachmentSent(res) => match res {
                Ok(_) => self.after_send_success(),
                Err(err) => self.send_error = Some(format!("Send attachment failed: {}", err)),
            },
            ApiResponse::TextSent(res) => match res {
                Ok(_) => {
                    self.after_send_success();
                }
                Err(e) => {
                    self.send_error = Some(format!("Send failed: {}", e));
                }
            },
            ApiResponse::ImageDownloaded { guid, result } => match result {
                Ok(proto) => {
                    self.image_cache.insert(guid, ImageCacheEntry::Ready(proto));
                }
                Err(_) => {
                    self.image_cache.insert(guid, ImageCacheEntry::Failed);
                }
            },
        }
    }
}

pub(crate) fn is_image_attachment(att: &bluebubbles_api::types::Attachment) -> bool {
    if let Some(mime) = &att.mime_type {
        return mime.starts_with("image/");
    }
    if let Some(name) = &att.transfer_name {
        let lower = name.to_lowercase();
        return lower.ends_with(".jpg")
            || lower.ends_with(".jpeg")
            || lower.ends_with(".png")
            || lower.ends_with(".gif")
            || lower.ends_with(".webp")
            || lower.ends_with(".bmp")
            || lower.ends_with(".tiff");
    }
    false
}

#[cfg(test)]
mod tests {
    use super::safe_attachment_file_name;

    #[test]
    fn attachment_file_name_drops_path_components() {
        assert_eq!(
            safe_attachment_file_name("../../outside.txt", "guid-1"),
            "outside.txt"
        );
        assert_eq!(safe_attachment_file_name("/etc/passwd", "guid-1"), "passwd");
        assert_eq!(
            safe_attachment_file_name("C:\\Users\\me\\photo.jpg", "guid-1"),
            "photo.jpg"
        );
    }

    #[test]
    fn attachment_file_name_falls_back_to_guid_for_empty_names() {
        assert_eq!(safe_attachment_file_name("", "guid-1"), "guid-1");
        assert_eq!(safe_attachment_file_name("../", "guid-1"), "guid-1");
        assert_eq!(safe_attachment_file_name("..", "guid-1"), "guid-1");
    }
}
