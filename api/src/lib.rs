mod api;
pub mod http;
pub mod types;

use http::{HttpShim, UreqShim};
use serde::Deserialize;
use serde_json::Value;
use std::error::Error;

use std::sync::Arc;

#[derive(Clone)]
pub struct BlueBubblesApi {
    host: String,
    password: String,
    http: Arc<dyn HttpShim>,
}

impl BlueBubblesApi {
    pub fn new(host: String, password: String) -> Self {
        let mut host = host.trim_end_matches('/').to_string();
        if !host.starts_with("http://") && !host.starts_with("https://") {
            host = format!("http://{}", host);
        }
        Self {
            host,
            password,
            http: Arc::new(UreqShim::new()),
        }
    }

    pub fn with_shim(host: String, password: String, http: Arc<dyn HttpShim>) -> Self {
        Self {
            host: host.trim_end_matches('/').to_string(),
            password,
            http,
        }
    }

    fn build_url(&self, path: &str) -> String {
        let encoded = urlencoding::encode(&self.password);
        let sep = if path.contains('?') { "&" } else { "?" };
        format!("{}{}{}password={}", self.host, path, sep, encoded)
    }

    fn handle_response<T: for<'de> Deserialize<'de>>(
        &self,
        response: Value,
    ) -> Result<T, Box<dyn Error>> {
        let status = response["status"].as_u64().ok_or("Missing status")?;
        if status != 200 {
            let message = response["message"].as_str().unwrap_or("Unknown error");
            return Err(format!("API error ({}): {}", status, message).into());
        }
        serde_json::from_value(response["data"].clone()).map_err(|e| e.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::Method;
    use std::sync::Arc;

    struct MockHttp {
        response: Value,
    }

    impl HttpShim for MockHttp {
        fn request(
            &self,
            _method: Method,
            _url: &str,
            _body: Option<Value>,
        ) -> Result<Value, Box<dyn Error>> {
            Ok(self.response.clone())
        }

        fn request_multipart(
            &self,
            _url: &str,
            _fields: Vec<(String, String)>,
            _files: Vec<(String, String, Vec<u8>)>,
        ) -> Result<Value, Box<dyn Error>> {
            Ok(self.response.clone())
        }

        fn request_raw(&self, _url: &str) -> Result<Vec<u8>, Box<dyn Error>> {
            Ok(Vec::new())
        }
    }

    fn api_with_response(response: Value) -> BlueBubblesApi {
        BlueBubblesApi::with_shim(
            "http://localhost:1234".to_string(),
            "password".to_string(),
            Arc::new(MockHttp { response }),
        )
    }

    fn valid_message(guid: &str) -> Value {
        serde_json::json!({
            "guid": guid,
            "text": "hello",
            "error": 0,
            "dateCreated": 1700000000000_u64,
            "isFromMe": false,
            "isDelayed": false,
            "isAutoReply": false,
            "isSystemMessage": false,
            "isServiceMessage": false,
            "isForward": false,
            "isArchived": false,
            "isAudioMessage": false,
            "itemType": 0,
            "groupActionType": 0
        })
    }

    #[test]
    fn ping_accepts_success_response() {
        let api = api_with_response(
            serde_json::json!({ "status": 200, "message": "Success", "data": {} }),
        );
        assert!(api.ping().is_ok());
    }

    #[test]
    fn handle_response_preserves_api_error_status_and_message() {
        let api = BlueBubblesApi::new("http://localhost:1234".to_string(), "password".to_string());
        let response = serde_json::json!({ "status": 401, "message": "Unauthorized" });
        let result: Result<Value, _> = api.handle_response(response);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "API error (401): Unauthorized"
        );
    }

    #[test]
    fn query_messages_parses_valid_message_arrays() {
        let api = api_with_response(serde_json::json!({
            "status": 200,
            "message": "Success",
            "data": [valid_message("message-1"), valid_message("message-2")]
        }));

        let messages = api.query_messages(Default::default()).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].guid, "message-1");
        assert_eq!(messages[1].guid, "message-2");
    }

    #[test]
    fn query_messages_reports_malformed_entries() {
        let api = api_with_response(serde_json::json!({
            "status": 200,
            "message": "Success",
            "data": [valid_message("message-1"), { "guid": "bad-message" }]
        }));

        let err = api.query_messages(Default::default()).unwrap_err();
        assert!(
            err.to_string()
                .contains("Failed to parse message at index 1")
        );
    }
}
