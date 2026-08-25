use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub status: u16,
    pub message: String,
    pub data: Option<T>,
}

// --- Core Entities ---

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Handle {
    pub address: String,
    pub country: Option<String>,
    #[serde(rename = "uncanonicalizedId")]
    pub uncanonicalized_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Chat {
    pub guid: String,
    pub participants: Option<Vec<Handle>>,
    #[serde(rename = "chatIdentifier")]
    pub chat_identifier: String,
    #[serde(rename = "isArchived")]
    pub is_archived: bool,
    #[serde(rename = "isFiltered")]
    pub is_filtered: bool,
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
    #[serde(rename = "lastMessage")]
    pub last_message: Option<Box<Message>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Message {
    pub guid: String,
    pub text: Option<String>,
    pub handle: Option<Handle>,
    #[serde(rename = "handleId")]
    pub handle_id: Option<u64>,
    pub attachments: Option<Vec<Attachment>>,
    pub subject: Option<String>,
    pub error: u64,
    #[serde(rename = "dateCreated")]
    pub date_created: u64,
    #[serde(rename = "dateRead")]
    pub date_read: Option<u64>,
    #[serde(rename = "dateDelivered")]
    pub date_delivered: Option<u64>,
    #[serde(rename = "isFromMe")]
    pub is_from_me: bool,
    #[serde(rename = "isDelayed")]
    pub is_delayed: bool,
    #[serde(rename = "isAutoReply")]
    pub is_auto_reply: bool,
    #[serde(rename = "isSystemMessage")]
    pub is_system_message: bool,
    #[serde(rename = "isServiceMessage")]
    pub is_service_message: bool,
    #[serde(rename = "isForward")]
    pub is_forward: bool,
    #[serde(rename = "isArchived")]
    pub is_archived: bool,
    #[serde(rename = "isAudioMessage")]
    pub is_audio_message: bool,
    #[serde(rename = "itemType")]
    pub item_type: u64,
    #[serde(rename = "groupTitle")]
    pub group_title: Option<String>,
    #[serde(rename = "groupActionType")]
    pub group_action_type: u64,
    #[serde(rename = "associatedMessageGuid")]
    pub associated_message_guid: Option<String>,
    #[serde(rename = "associatedMessageType")]
    pub associated_message_type: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Attachment {
    pub guid: String,
    #[serde(rename = "transferName")]
    pub transfer_name: Option<String>,
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
    #[serde(rename = "totalBytes")]
    pub total_bytes: u64,
    #[serde(rename = "isOutgoing")]
    pub is_outgoing: bool,
    #[serde(rename = "isSticker")]
    pub is_sticker: bool,
    #[serde(rename = "uti")]
    pub uti: Option<String>,
    #[serde(rename = "transferState")]
    pub transfer_state: u64,
}

// --- Request/Response Data Structs ---

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatCountData {
    pub total: u64,
    pub breakdown: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MessageCountData {
    pub total: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ContactAddress {
    pub address: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ContactData {
    #[serde(rename = "firstName")]
    pub first_name: Option<String>,
    #[serde(rename = "lastName")]
    pub last_name: Option<String>,
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
    pub nickname: Option<String>,
    #[serde(rename = "phoneNumbers", default)]
    pub phone_numbers: Vec<ContactAddress>,
    #[serde(default)]
    pub emails: Vec<ContactAddress>,
}

impl ContactData {
    pub fn best_name(&self) -> Option<String> {
        if let Some(n) = &self.display_name
            && !n.is_empty()
        {
            return Some(n.clone());
        }
        let first = self.first_name.as_deref().unwrap_or("");
        let last = self.last_name.as_deref().unwrap_or("");
        let full = format!("{} {}", first, last).trim().to_string();
        if !full.is_empty() {
            return Some(full);
        }
        if let Some(n) = &self.nickname
            && !n.is_empty()
        {
            return Some(n.clone());
        }
        None
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ScheduledMessageData {
    pub id: u64,
    #[serde(rename = "type")]
    pub action_type: String,
    pub payload: Value,
    #[serde(rename = "scheduledFor")]
    pub scheduled_for: u64,
    pub schedule: Value,
    pub status: String,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerInfo {
    pub os_version: String,
    pub server_version: String,
    pub private_api: bool,
    pub proxy_service: String,
    pub helper_connected: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EntityTotals {
    pub handles: u64,
    pub messages: u64,
    pub chats: u64,
    pub attachments: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MediaTotals {
    pub images: u64,
    pub videos: u64,
    pub locations: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerAlert {
    #[serde(rename = "type")]
    pub alert_type: String,
    pub value: String,
    #[serde(rename = "isRead")]
    pub is_read: bool,
    pub created: String,
    pub updated: String,
}

// --- Chat Request Structs ---

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ChatQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u64>,
    #[serde(rename = "with", skip_serializing_if = "Option::is_none")]
    pub with_related: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NewChat {
    pub addresses: Vec<String>,
    pub message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateChat {
    #[serde(rename = "displayName")]
    pub display_name: String,
}

// --- Message Request Structs ---

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct MessageQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u64>,
    #[serde(rename = "chatGuid", skip_serializing_if = "Option::is_none")]
    pub chat_guid: Option<String>,
    #[serde(rename = "with", skip_serializing_if = "Option::is_none")]
    pub with_related: Option<Vec<String>>,
    #[serde(rename = "where", skip_serializing_if = "Option::is_none")]
    pub where_clauses: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct SendText {
    #[serde(rename = "chatGuid")]
    pub chat_guid: String,
    pub message: String,
    pub method: String,
    #[serde(rename = "tempGuid")]
    pub temp_guid: Option<String>,
    pub subject: Option<String>,
    #[serde(rename = "effectId")]
    pub effect_id: Option<String>,
    #[serde(rename = "selectedMessageGuid")]
    pub selected_message_guid: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct SendReaction {
    #[serde(rename = "chatGuid")]
    pub chat_guid: String,
    #[serde(rename = "selectedMessageGuid")]
    pub selected_message_guid: String,
    pub reaction: String,
    #[serde(rename = "partIndex")]
    pub part_index: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ScheduledMessageRequest {
    #[serde(rename = "type")]
    pub action_type: String,
    pub payload: Value,
    #[serde(rename = "scheduledFor")]
    pub scheduled_for: u64,
    pub schedule: Value,
}
