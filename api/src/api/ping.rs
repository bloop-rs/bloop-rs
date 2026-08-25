use crate::BlueBubblesApi;
use crate::http::Method;
use serde_json::Value;
use std::error::Error;

impl BlueBubblesApi {
    pub fn ping(&self) -> Result<(), Box<dyn Error>> {
        let url = self.build_url("/api/v1/ping");
        let response = self.http.request(Method::Get, &url, None)?;
        let _: Value = self.handle_response(response)?;
        Ok(())
    }
}
