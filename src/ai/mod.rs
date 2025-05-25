use anyhow::{bail, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize}; 
use std::env;

const DEFAULT_GEMINI_MODEL_ID: &str = "gemini-2.0-flash";
const GEMINI_API_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta/models";

#[derive(Serialize)]
struct GeminiApiRequest {
    contents: Vec<Content>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generation_config: Option<GenerationConfig>,
}

#[derive(Serialize)]
struct Content {
    parts: Vec<Part>,
}

#[derive(Serialize)]
struct Part {
    text: String,
}

#[derive(Serialize, Debug, Clone)]
struct GenerationConfig {
    #[serde(rename = "candidateCount")]
    candidate_count: Option<u32>,
}

#[derive(Deserialize, Debug)]
struct GeminiApiResponse {
    candidates: Option<Vec<Candidate>>,
    error: Option<ApiErrorDetail>,
}

#[derive(Deserialize, Debug)]
struct Candidate {
    content: Option<ModelContent>,
}

#[derive(Deserialize, Debug)]
struct ModelContent {
    parts: Option<Vec<ModelPart>>,
}

#[derive(Deserialize, Debug)]
struct ModelPart {
    text: Option<String>,
}

#[derive(Deserialize, Debug)]
struct ApiErrorDetail {
    code: i32,
    message: String,
    status: String,
}

pub async fn generate_text(prompt_text: &str, num_suggestions: u32) -> Result<Vec<String>> {
    let api_key =
        env::var("GEMINI_API_KEY").context("GEMINI_API_KEY environment variable not set.")?;

    let model_id = DEFAULT_GEMINI_MODEL_ID;

    let client = Client::new();
    let url = format!(
        "{}/{}:generateContent?key={}",
        GEMINI_API_BASE_URL, model_id, api_key
    );

    let request_payload = GeminiApiRequest {
        contents: vec![Content {
            parts: vec![Part {
                text: prompt_text.to_string(),
            }],
        }],
        generation_config: Some(GenerationConfig {
            candidate_count: Some(num_suggestions),
        }),
    };

    let response = client
        .post(&url)
        .json(&request_payload)
        .send()
        .await
        .context("Failed to send request to Gemini API")?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error body".to_string());
        bail!(
            "Gemini API request failed with status {}: {}",
            status,
            error_text
        );
    }

    let response_body: GeminiApiResponse = response
        .json()
        .await
        .context("Failed to parse Gemini API response")?;

    if let Some(error) = response_body.error {
        bail!(
            "Gemini API returned an error: code {}, message: {}, status: {}",
            error.code,
            error.message,
            error.status
        );
    }

    let mut suggestions = Vec::new();
    if let Some(candidates) = response_body.candidates {
        for candidate in candidates {
            if let Some(content) = candidate.content {
                if let Some(parts) = content.parts {
                    for part in parts {
                        if let Some(text) = part.text {
                            suggestions.push(text);
                        }
                    }
                }
            }
        }
    }

    if suggestions.is_empty() {
        bail!("No suggestions received from Gemini API or response format was unexpected.");
    }

    Ok(suggestions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[tokio::test]
    async fn test_generate_text_api_key_missing() {
        let original_key_value = env::var("GEMINI_API_KEY").ok();
        unsafe { 
            env::remove_var("GEMINI_API_KEY");
        }

        let result = generate_text("Test prompt for missing key", 1).await;
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e
                .to_string()
                .contains("GEMINI_API_KEY environment variable not set."));
        }

        if let Some(key_val) = original_key_value {
            unsafe { 
                env::set_var("GEMINI_API_KEY", key_val);
            }
        }
    }

    #[tokio::test]
    #[ignore] 
    async fn test_generate_single_suggestion_live() -> Result<()> {
        if env::var("GEMINI_API_KEY").is_err() {
            println!("Skipping test_generate_single_suggestion_live: GEMINI_API_KEY not set.");
            return Ok(());
        }
        let prompt = "Write a short poem about Rust programming.";
        let suggestions = generate_text(prompt, 1).await?;
        assert_eq!(suggestions.len(), 1);
        assert!(!suggestions[0].is_empty());
        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_generate_multiple_suggestions_live() -> Result<()> {
        if env::var("GEMINI_API_KEY").is_err() {
            println!("Skipping test_generate_multiple_suggestions_live: GEMINI_API_KEY not set.");
            return Ok(());
        }
        let prompt = "Suggest three names for a new tech startup focused on AI.";
        let suggestions = generate_text(prompt, 3).await?;
        assert_eq!(suggestions.len(), 3);
        for suggestion in suggestions {
            assert!(!suggestion.is_empty());
        }
        Ok(())
    }
}
