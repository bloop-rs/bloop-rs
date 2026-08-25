use crate::BlueBubblesApi;
use crate::http::Method;
use crate::types::{
    Message, MessageCountData, MessageQuery, ScheduledMessageData, ScheduledMessageRequest,
    SendReaction, SendText,
};
use serde_json::Value;
use std::error::Error;

impl BlueBubblesApi {
    pub fn get_message_count(&self) -> Result<MessageCountData, Box<dyn Error>> {
        let url = self.build_url("/api/v1/message/count");
        let response = self.http.request(Method::Get, &url, None)?;
        self.handle_response(response)
    }

    pub fn get_my_sent_message_count(&self) -> Result<MessageCountData, Box<dyn Error>> {
        let url = self.build_url("/api/v1/message/count/me");
        let response = self.http.request(Method::Get, &url, None)?;
        self.handle_response(response)
    }

    pub fn get_message_by_guid(&self, guid: &str) -> Result<Message, Box<dyn Error>> {
        let url = self.build_url(&format!("/api/v1/message/{}", guid));
        let response = self.http.request(Method::Get, &url, None)?;
        self.handle_response(response)
    }

    pub fn get_embedded_media(&self, guid: &str) -> Result<Value, Box<dyn Error>> {
        let url = self.build_url(&format!("/api/v1/message/{}/embedded-media", guid));
        let response = self.http.request(Method::Get, &url, None)?;
        self.handle_response(response)
    }

    pub fn query_messages(&self, query: MessageQuery) -> Result<Vec<Message>, Box<dyn Error>> {
        let url = self.build_url("/api/v1/message/query");
        let body = serde_json::to_value(query)?;
        let response = self.http.request(Method::Post, &url, Some(body))?;
        let status = response["status"].as_u64().ok_or("Missing status")?;
        if status != 200 {
            let message = response["message"].as_str().unwrap_or("Unknown error");
            return Err(format!("API error ({}): {}", status, message).into());
        }
        let arr = response["data"]
            .as_array()
            .ok_or("Expected array in data")?;
        let messages = arr
            .iter()
            .enumerate()
            .map(|(idx, v)| {
                serde_json::from_value::<Message>(v.clone())
                    .map_err(|e| format!("Failed to parse message at index {}: {}", idx, e))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(messages)
    }

    pub fn send_text(&self, send_text: SendText) -> Result<Message, Box<dyn Error>> {
        let url = self.build_url("/api/v1/message/text");
        let body = serde_json::to_value(send_text)?;
        let response = self.http.request(Method::Post, &url, Some(body))?;
        self.handle_response(response)
    }

    pub fn send_reaction(&self, send_reaction: SendReaction) -> Result<Message, Box<dyn Error>> {
        let url = self.build_url("/api/v1/message/react");
        let body = serde_json::to_value(send_reaction)?;
        let response = self.http.request(Method::Post, &url, Some(body))?;
        self.handle_response(response)
    }

    pub fn edit_message(
        &self,
        guid: &str,
        edited_message: &str,
        backwards_compatibility: &str,
        part_index: u64,
    ) -> Result<Message, Box<dyn Error>> {
        let url = self.build_url(&format!("/api/v1/message/{}/edit", guid));
        let body = serde_json::json!({
            "editedMessage": edited_message,
            "backwardsCompatibilityMessage": backwards_compatibility,
            "partIndex": part_index
        });
        let response = self.http.request(Method::Post, &url, Some(body))?;
        self.handle_response(response)
    }

    pub fn unsend_message(&self, guid: &str, part_index: u64) -> Result<Message, Box<dyn Error>> {
        let url = self.build_url(&format!("/api/v1/message/{}/unsend", guid));
        let body = serde_json::json!({ "partIndex": part_index });
        let response = self.http.request(Method::Post, &url, Some(body))?;
        self.handle_response(response)
    }

    pub fn notify_silenced(&self, guid: &str) -> Result<(), Box<dyn Error>> {
        let url = self.build_url(&format!("/api/v1/message/{}/notify", guid));
        let response = self.http.request(Method::Post, &url, None)?;
        let _: Value = self.handle_response(response)?;
        Ok(())
    }

    pub fn get_scheduled_messages(&self) -> Result<Vec<ScheduledMessageData>, Box<dyn Error>> {
        let url = self.build_url("/api/v1/message/schedule");
        let response = self.http.request(Method::Get, &url, None)?;
        self.handle_response(response)
    }

    pub fn schedule_message(
        &self,
        request: ScheduledMessageRequest,
    ) -> Result<ScheduledMessageData, Box<dyn Error>> {
        let url = self.build_url("/api/v1/message/schedule");
        let body = serde_json::to_value(request)?;
        let response = self.http.request(Method::Post, &url, Some(body))?;
        self.handle_response(response)
    }

    pub fn get_scheduled_message_by_id(
        &self,
        id: u64,
    ) -> Result<ScheduledMessageData, Box<dyn Error>> {
        let url = self.build_url(&format!("/api/v1/message/schedule/{}", id));
        let response = self.http.request(Method::Get, &url, None)?;
        self.handle_response(response)
    }

    pub fn delete_scheduled_message(&self, id: u64) -> Result<(), Box<dyn Error>> {
        let url = self.build_url(&format!("/api/v1/message/schedule/{}", id));
        let response = self.http.request(Method::Delete, &url, None)?;
        let _: Value = self.handle_response(response)?;
        Ok(())
    }

    pub fn update_scheduled_message(
        &self,
        id: u64,
        request: ScheduledMessageRequest,
    ) -> Result<ScheduledMessageData, Box<dyn Error>> {
        let url = self.build_url(&format!("/api/v1/message/schedule/{}", id));
        let body = serde_json::to_value(request)?;
        let response = self.http.request(Method::Put, &url, Some(body))?;
        self.handle_response(response)
    }

    pub fn send_attachment(
        &self,
        chat_guid: &str,
        file_name: &str,
        content: Vec<u8>,
        temp_guid: Option<&str>,
        method: Option<&str>,
        is_audio_message: bool,
    ) -> Result<(), Box<dyn Error>> {
        let url = self.build_url("/api/v1/message/attachment");
        let mut fields = vec![
            ("chatGuid".to_string(), chat_guid.to_string()),
            ("name".to_string(), file_name.to_string()),
            ("isAudioMessage".to_string(), is_audio_message.to_string()),
        ];
        if let Some(tg) = temp_guid {
            fields.push(("tempGuid".to_string(), tg.to_string()));
        }
        if let Some(m) = method {
            fields.push(("method".to_string(), m.to_string()));
        }

        let response = self.http.request_multipart(
            &url,
            fields,
            vec![("attachment".to_string(), file_name.to_string(), content)],
        )?;
        // Parse status only — the response body schema varies and we don't use the returned message.
        let status = response["status"].as_u64().ok_or("Missing status")?;
        if status != 200 {
            let message = response["message"].as_str().unwrap_or("Unknown error");
            return Err(format!("API error ({}): {}", status, message).into());
        }
        Ok(())
    }

    pub fn send_multipart_message(
        &self,
        chat_guid: &str,
        parts: Vec<Value>,
    ) -> Result<Message, Box<dyn Error>> {
        let url = self.build_url("/api/v1/message/multipart");
        let body = serde_json::json!({
            "chatGuid": chat_guid,
            "parts": parts
        });
        let response = self.http.request(Method::Post, &url, Some(body))?;
        self.handle_response(response)
    }
}
