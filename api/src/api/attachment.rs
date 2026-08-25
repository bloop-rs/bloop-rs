use crate::BlueBubblesApi;
use std::error::Error;

impl BlueBubblesApi {
    pub fn download_attachment(&self, guid: &str) -> Result<Vec<u8>, Box<dyn Error>> {
        let url = self.build_url(&format!("/api/v1/attachment/{}/download", guid));
        self.http.request_raw(&url)
    }
}
