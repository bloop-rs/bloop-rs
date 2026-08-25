use serde_json::Value;
use std::error::Error;
use std::time::Duration;
use ureq::Agent;

pub enum Method {
    Get,
    Post,
    Put,
    Delete,
}

pub trait HttpShim: Send + Sync {
    fn request(
        &self,
        method: Method,
        url: &str,
        body: Option<Value>,
    ) -> Result<Value, Box<dyn Error>>;

    fn request_multipart(
        &self,
        url: &str,
        fields: Vec<(String, String)>,
        files: Vec<(String, String, Vec<u8>)>,
    ) -> Result<Value, Box<dyn Error>>;

    fn request_raw(&self, url: &str) -> Result<Vec<u8>, Box<dyn Error>>;
}

pub struct UreqShim {
    agent: Agent,
}

impl UreqShim {
    pub fn new() -> Self {
        let config = Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(10)))
            .build();
        let agent = Agent::new_with_config(config);
        UreqShim { agent }
    }
}

impl Default for UreqShim {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpShim for UreqShim {
    fn request(
        &self,
        method: Method,
        url: &str,
        body: Option<Value>,
    ) -> Result<Value, Box<dyn Error>> {
        let mut response = match method {
            Method::Get => self.agent.get(url).call()?,
            Method::Post => {
                if let Some(body) = body {
                    self.agent.post(url).send_json(body)?
                } else {
                    self.agent.post(url).send("")?
                }
            }
            Method::Put => {
                if let Some(body) = body {
                    self.agent.put(url).send_json(body)?
                } else {
                    self.agent.put(url).send("")?
                }
            }
            Method::Delete => {
                if let Some(body) = body {
                    // ureq 3.x RequestBuilder<WithoutBody> (from ureq::delete) doesn't support send_json.
                    // As a workaround, we use ureq::post which supports body.
                    // Note: This might need to be adjusted if the server strictly requires DELETE method.
                    self.agent.post(url).send_json(body)?
                } else {
                    self.agent.delete(url).call()?
                }
            }
        };

        let val: Value = response.body_mut().read_json()?;
        Ok(val)
    }

    fn request_multipart(
        &self,
        url: &str,
        fields: Vec<(String, String)>,
        files: Vec<(String, String, Vec<u8>)>,
    ) -> Result<Value, Box<dyn Error>> {
        use ureq::unversioned::multipart::{Form, Part};
        let mut form = Form::new();
        for (name, value) in &fields {
            form = form.text(name, value);
        }
        for (field_name, file_name, content) in &files {
            form = form.part(field_name, Part::bytes(content).file_name(file_name));
        }

        let mut response = self.agent.post(url).send(form)?;
        let val: Value = response.body_mut().read_json()?;
        Ok(val)
    }

    fn request_raw(&self, url: &str) -> Result<Vec<u8>, Box<dyn Error>> {
        use std::io::Read;
        let mut response = self.agent.get(url).call()?;
        let mut bytes = Vec::new();
        // as_reader() bypasses the default 10 MB limit imposed by read_to_vec/read_json
        response.body_mut().as_reader().read_to_end(&mut bytes)?;
        Ok(bytes)
    }
}
