use crate::helpers::friendly_login_error;
use crate::state::{FocusedPane, LoginFields, MainState};
use bluebubbles_api::BlueBubblesApi;
use bluebubbles_api::types::ChatQuery;
use ratatui::widgets::ListState;
use ratatui_image::picker::Picker;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct SavedConfig {
    pub(crate) host: String,
    pub(crate) password: String,
    #[serde(default = "default_send_method")]
    pub(crate) send_method: String,
}

pub(crate) fn default_send_method() -> String {
    "apple-script".to_string()
}

fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".config")
        .join("bloop")
        .join("config.json")
}

pub(crate) fn load_saved_config() -> Option<SavedConfig> {
    let content = std::fs::read_to_string(config_path()).ok()?;
    serde_json::from_str(&content).ok()
}

#[cfg(unix)]
fn write_private_config(path: &Path, json: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(json.as_bytes())?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn write_private_config(path: &Path, json: &str) -> std::io::Result<()> {
    std::fs::write(path, json)
}

pub(crate) fn save_config_with_method(host: &str, password: &str, send_method: &str) {
    let config = SavedConfig {
        host: host.to_string(),
        password: password.to_string(),
        send_method: send_method.to_string(),
    };
    if let Ok(json) = serde_json::to_string(&config) {
        let path = config_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = write_private_config(&path, &json);
    }
}

pub(crate) fn try_login(
    host: String,
    password: String,
    send_method: String,
    picker: Picker,
) -> Result<MainState, String> {
    let api = BlueBubblesApi::new(host, password);
    api.ping()
        .map_err(|e| friendly_login_error(&e.to_string()))?;

    let chats = api
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
        .map_err(|e| format!("Failed to load chats: {}", e))?;

    let mut chat_list_state = ListState::default();
    let selected_chat_index = if !chats.is_empty() {
        chat_list_state.select(Some(0));
        Some(0)
    } else {
        None
    };

    let mut s = MainState {
        api,
        chats,
        chat_list_state,
        messages: Vec::new(),
        selected_chat_index,
        contacts: HashMap::new(),
        focused_pane: FocusedPane::Chats,
        message_selected: None,
        compose_mode: false,
        compose_text: String::new(),
        compose_cursor: 0,
        send_error: None,
        last_chat_reload: Instant::now(),
        last_message_reload: Instant::now(),
        pending_load: None,
        last_nav: Instant::now(),
        attachment_items: Vec::new(),
        attachment_selected: 0,
        attachment_status: None,
        has_more_messages: true,
        status_expires: None,
        pending_attachment_open: None,
        loading_attachment: false,
        pending_send: false,
        pending_at: false,
        pending_file_attachments: Vec::new(),
        file_chooser_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        file_chooser_entries: Vec::new(),
        file_chooser_selected: 0,
        file_chooser_filter: String::new(),
        chat_search_query: String::new(),
        chat_search_results: Vec::new(),
        chat_search_selected: 0,
        emoji_picker_state: None,
        send_method,
        last_activity: Instant::now(),
        last_seen_guids: HashMap::new(),
        unread_chats: HashSet::new(),
        terminal_focused: false,
        drafts: HashMap::new(),
        api_requests_in_flight: 0,
        tui_notifications: HashMap::new(),
        notification_fade_start: None,
        image_picker: picker,
        image_cache: HashMap::new(),
        pending_image_downloads: Vec::new(),
    };

    for chat in &s.chats {
        if let Some(last_msg) = &chat.last_message {
            s.last_seen_guids
                .insert(chat.guid.clone(), last_msg.guid.clone());
        }
    }

    Ok(s)
}

pub(crate) fn initial_login_fields(saved: &SavedConfig) -> LoginFields {
    LoginFields {
        host: saved.host.clone(),
        password: saved.password.clone(),
        use_private_api: saved.send_method == "private-api",
        active_field: 0,
        error: None,
    }
}
