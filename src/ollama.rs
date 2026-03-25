//! Minimal blocking client for interacting with an Ollama server.
//!
//! Provides a simple wrapper around the `/api/generate` endpoint.

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct Ollama {
    base_url: String,
    client: Client,
}

#[derive(Debug, Serialize)]
struct OllamaRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    response: String,
}

#[derive(Debug, Deserialize)]
struct TagsResponse {
    models: Vec<ModelInfo>,
}

#[derive(Debug, Deserialize)]
struct ModelInfo {
    name: String,
}

impl Default for Ollama {
    fn default() -> Self {
        Self::new("http://127.0.0.1:11434/api")
    }
}

impl Ollama {
    /// Create a new Ollama client with a given base URL.
    pub fn new<S: Into<String>>(base_url: S) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .unwrap();

        Self {
            base_url: base_url.into(),
            client,
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Send a prompt to the Ollama server and return the generated response.
    pub fn generate(&self, model: &str, prompt: &str) -> anyhow::Result<String> {
        //println!("{}",prompt );
        let request = OllamaRequest {
            model,
            prompt,
            stream: false,
            format: None,
        };

        let response = self
            .client
            .post(format!("{}/generate", &self.base_url))
            .json(&request)
            .send()?
            .error_for_status()?;

        let parsed: OllamaResponse = response.json::<OllamaResponse>()?;
        Ok(parsed.response)
    }

    /// Send a prompt with structured output requested via JSON schema.
    pub fn generate_structured(
        &self,
        model: &str,
        prompt: &str,
        schema: Value,
    ) -> anyhow::Result<String> {
        let request = OllamaRequest {
            model,
            prompt,
            stream: false,
            format: Some(schema),
        };

        let response = self
            .client
            .post(format!("{}/generate", self.base_url))
            .json(&request)
            .send()?
            .error_for_status()?;

        let parsed: OllamaResponse = response.json()?;
        Ok(parsed.response)
    }

    /// Return the installed Ollama model names.
    pub fn list_models(&self) -> anyhow::Result<Vec<String>> {
        let response = self
            .client
            .get(format!("{}/tags", self.base_url))
            .send()?
            .error_for_status()?;

        let parsed: TagsResponse = response.json()?;
        Ok(parsed.models.into_iter().map(|m| m.name).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_url_is_correct() {
        let client = Ollama::default();
        assert_eq!(client.base_url, "http://127.0.0.1:11434/api");
    }

    #[test]
    fn new_sets_base_url() {
        let client = Ollama::new("http://example.com");
        assert_eq!(client.base_url, "http://example.com");
    }
}
