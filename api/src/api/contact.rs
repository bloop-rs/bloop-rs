use crate::BlueBubblesApi;
use crate::http::Method;
use crate::types::ContactData;
use std::error::Error;

impl BlueBubblesApi {
    pub fn get_contacts(&self) -> Result<Vec<ContactData>, Box<dyn Error>> {
        let url = self.build_url("/api/v1/contact");
        let response = self.http.request(Method::Get, &url, None)?;
        self.handle_response(response)
    }

    pub fn query_contacts(
        &self,
        addresses: Vec<String>,
    ) -> Result<Vec<ContactData>, Box<dyn Error>> {
        let url = self.build_url("/api/v1/contact/query");
        let body = serde_json::json!({ "addresses": addresses });
        let response = self.http.request(Method::Post, &url, Some(body))?;
        self.handle_response(response)
    }
}
