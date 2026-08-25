use crate::BlueBubblesApi;
use crate::http::Method;
use crate::types::{Chat, ChatCountData, ChatQuery, Message, NewChat, UpdateChat};
use serde_json::Value;
use std::error::Error;

impl BlueBubblesApi {
    pub fn get_chat_count(&self) -> Result<ChatCountData, Box<dyn Error>> {
        let url = self.build_url("/api/v1/chat/count");
        let response = self.http.request(Method::Get, &url, None)?;
        self.handle_response(response)
    }

    pub fn get_chat_by_guid(&self, guid: &str) -> Result<Chat, Box<dyn Error>> {
        let url = self.build_url(&format!("/api/v1/chat/{}", guid));
        let response = self.http.request(Method::Get, &url, None)?;
        self.handle_response(response)
    }

    pub fn get_chat_messages(&self, guid: &str) -> Result<Vec<Message>, Box<dyn Error>> {
        let url = self.build_url(&format!("/api/v1/chat/{}/message", guid));
        let response = self.http.request(Method::Get, &url, None)?;
        self.handle_response(response)
    }

    pub fn query_chats(&self, query: ChatQuery) -> Result<Vec<Chat>, Box<dyn Error>> {
        let url = self.build_url("/api/v1/chat/query");
        let body = serde_json::to_value(query)?;
        let response = self.http.request(Method::Post, &url, Some(body))?;
        self.handle_response(response)
    }

    pub fn create_new_chat(&self, new_chat: NewChat) -> Result<Chat, Box<dyn Error>> {
        let url = self.build_url("/api/v1/chat/new");
        let body = serde_json::to_value(new_chat)?;
        let response = self.http.request(Method::Post, &url, Some(body))?;
        self.handle_response(response)
    }

    pub fn mark_chat_read(&self, guid: &str) -> Result<(), Box<dyn Error>> {
        let url = self.build_url(&format!("/api/v1/chat/{}/read", guid));
        let response = self.http.request(Method::Post, &url, None)?;
        let _: Value = self.handle_response(response)?;
        Ok(())
    }

    pub fn mark_chat_unread(&self, guid: &str) -> Result<(), Box<dyn Error>> {
        let url = self.build_url(&format!("/api/v1/chat/{}/unread", guid));
        let response = self.http.request(Method::Post, &url, None)?;
        let _: Value = self.handle_response(response)?;
        Ok(())
    }

    pub fn share_contact_info(&self, guid: &str) -> Result<(), Box<dyn Error>> {
        let url = self.build_url(&format!("/api/v1/chat/{}/share/contact", guid));
        let response = self.http.request(Method::Post, &url, None)?;
        let _: Value = self.handle_response(response)?;
        Ok(())
    }

    pub fn get_contact_share_status(&self, guid: &str) -> Result<Vec<Message>, Box<dyn Error>> {
        let url = self.build_url(&format!("/api/v1/chat/{}/share/contact/status", guid));
        let response = self.http.request(Method::Get, &url, None)?;
        self.handle_response(response)
    }

    pub fn get_chat_icon_by_guid(&self, guid: &str) -> Result<Chat, Box<dyn Error>> {
        let url = self.build_url(&format!("/api/v1/chat/{}/icon", guid));
        let response = self.http.request(Method::Get, &url, None)?;
        self.handle_response(response)
    }

    pub fn delete_chat(&self, guid: &str) -> Result<(), Box<dyn Error>> {
        let url = self.build_url(&format!("/api/v1/chat/{}", guid));
        let response = self.http.request(Method::Delete, &url, None)?;
        let _: Value = self.handle_response(response)?;
        Ok(())
    }

    pub fn update_chat(&self, guid: &str, update: UpdateChat) -> Result<Chat, Box<dyn Error>> {
        let url = self.build_url(&format!("/api/v1/chat/{}", guid));
        let body = serde_json::to_value(update)?;
        let response = self.http.request(Method::Put, &url, Some(body))?;
        self.handle_response(response)
    }

    pub fn add_participant(&self, guid: &str, address: &str) -> Result<Chat, Box<dyn Error>> {
        let url = self.build_url(&format!("/api/v1/chat/{}/participant", guid));
        let body = serde_json::json!({ "address": address });
        let response = self.http.request(Method::Post, &url, Some(body))?;
        self.handle_response(response)
    }

    pub fn remove_participant(&self, guid: &str, address: &str) -> Result<Chat, Box<dyn Error>> {
        let url = self.build_url(&format!("/api/v1/chat/{}/participant", guid));
        let body = serde_json::json!({ "address": address });
        let response = self.http.request(Method::Delete, &url, Some(body))?;
        self.handle_response(response)
    }

    pub fn leave_chat(&self, guid: &str) -> Result<(), Box<dyn Error>> {
        let url = self.build_url(&format!("/api/v1/chat/{}/leave", guid));
        let response = self.http.request(Method::Post, &url, None)?;
        let _: Value = self.handle_response(response)?;
        Ok(())
    }

    pub fn remove_group_icon(&self, guid: &str) -> Result<(), Box<dyn Error>> {
        let url = self.build_url(&format!("/api/v1/chat/{}/icon", guid));
        let response = self.http.request(Method::Delete, &url, None)?;
        let _: Value = self.handle_response(response)?;
        Ok(())
    }

    pub fn set_group_icon(
        &self,
        guid: &str,
        file_name: &str,
        content: Vec<u8>,
    ) -> Result<Chat, Box<dyn Error>> {
        let url = self.build_url(&format!("/api/v1/chat/{}/icon", guid));
        let response = self.http.request_multipart(
            &url,
            vec![],
            vec![("icon".to_string(), file_name.to_string(), content)],
        )?;
        self.handle_response(response)
    }
}
