use bluebubbles_api::types::{
    ApiResponse, Chat, ChatCountData, EntityTotals, MediaTotals, Message, ServerAlert, ServerInfo,
};
use std::fs;

#[test]
fn test_parse_chat_count() {
    let data = fs::read_to_string(
        "test_fixtures/a98282be-6696-4c7d-95c2-0ed6dbbb86de_Get_Chat_Count.json",
    )
    .unwrap();
    let res: ApiResponse<ChatCountData> = serde_json::from_str(&data).unwrap();
    assert_eq!(res.status, 200);
    assert_eq!(res.data.unwrap().total, 230);
}

#[test]
fn test_parse_chat_by_guid_ok() {
    let data = fs::read_to_string(
        "test_fixtures/53b2db6c-d63b-4708-bb0f-04943abe049d_Get_Chat_by_GUID_-_OK.json",
    )
    .unwrap();
    let res: ApiResponse<Chat> = serde_json::from_str(&data).unwrap();
    assert_eq!(res.status, 200);
    let chat = res.data.unwrap();
    assert_eq!(chat.guid, "iMessage;+;chat1234567890");
    assert!(chat.participants.is_some());
    assert_eq!(chat.participants.unwrap().len(), 4);
}

#[test]
fn test_parse_chat_messages_ok() {
    let data = fs::read_to_string(
        "test_fixtures/7a53f416-a9af-43a6-bda6-33f759b40c0a_Get_Chat_Messages_-_OK.json",
    )
    .unwrap();
    let res: ApiResponse<Vec<Message>> = serde_json::from_str(&data).unwrap();
    assert_eq!(res.status, 200);
    let messages = res.data.unwrap();
    assert!(!messages.is_empty());
    assert_eq!(messages[0].guid, "AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEE");
}

#[test]
fn test_parse_fcm_client_config() {
    let data = fs::read_to_string(
        "test_fixtures/f2b2e843-94c6-49cc-b723-eba3a3d1b175_Get_FCM_Client_Config.json",
    )
    .unwrap();
    let res: ApiResponse<serde_json::Value> = serde_json::from_str(&data).unwrap();
    assert_eq!(res.status, 200);
}

#[test]
fn test_parse_server_metadata() {
    let data = fs::read_to_string(
        "test_fixtures/bb3fbf11-e1fd-452b-94ca-1d1047525673_Get_Server_Metadata.json",
    )
    .unwrap();
    let res: ApiResponse<ServerInfo> = serde_json::from_str(&data).unwrap();
    assert_eq!(res.status, 200);
    let info = res.data.unwrap();
    assert_eq!(info.os_version, "11.6.0");
    assert_eq!(info.server_version, "11.1.1");
}

#[test]
fn test_parse_entity_totals() {
    let data = fs::read_to_string(
        "test_fixtures/cc5dca71-01f1-4cf6-a265-e691d2714749_Get_iMessage_Entity_Totals.json",
    )
    .unwrap();
    let res: ApiResponse<EntityTotals> = serde_json::from_str(&data).unwrap();
    assert_eq!(res.status, 200);
    let totals = res.data.unwrap();
    assert_eq!(totals.messages, 124039);
}

#[test]
fn test_parse_media_totals() {
    let data = fs::read_to_string(
        "test_fixtures/6b49c5f1-384b-4cca-b702-b63dd2d1094e_Get_Media_Totals.json",
    )
    .unwrap();
    let res: ApiResponse<MediaTotals> = serde_json::from_str(&data).unwrap();
    assert_eq!(res.status, 200);
    let totals = res.data.unwrap();
    assert_eq!(totals.images, 6773);
}

#[test]
fn test_parse_server_alerts() {
    let data = fs::read_to_string(
        "test_fixtures/0320b267-d5a5-438a-ba9e-468a9e84f9b0_Get_Server_Alerts.json",
    )
    .unwrap();
    let res: ApiResponse<Vec<ServerAlert>> = serde_json::from_str(&data).unwrap();
    assert_eq!(res.status, 200);
    let alerts = res.data.unwrap();
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].alert_type, "warn");
}
