//! Minimal blocking client for interacting with an Ollama server.
//!
//! Provides a simple wrapper around the `/api/generate` endpoint.

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

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
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    response: String,
}

impl Default for Ollama {
    fn default() -> Self {
        Self::new("http://127.0.0.1:11434/api/generate")
    }
}

impl Ollama {
    /// Create a new Ollama client with a given base URL.
    pub fn new<S: Into<String>>(base_url: S) -> Self {
        Self {
            base_url: base_url.into(),
            client: Client::new(),
        }
    }

    /// Send a prompt to the Ollama server and return the generated response.
    pub fn generate(&self, model: &str, prompt: &str) -> anyhow::Result<String> {
        let request = OllamaRequest {
            model,
            prompt,
            stream: false,
        };

        let response = self
            .client
            .post(&self.base_url)
            .json(&request)
            .send()?
            .error_for_status()?;

        let parsed: OllamaResponse = response.json::<OllamaResponse>()?;
        Ok(parsed.response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_url_is_correct() {
        let client = Ollama::default();
        assert_eq!(client.base_url, "http://127.0.0.1:11434/api/generate");
    }

    #[test]
    fn new_sets_base_url() {
        let client = Ollama::new("http://example.com");
        assert_eq!(client.base_url, "http://example.com");
    }
}
