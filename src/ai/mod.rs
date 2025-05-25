use anyhow::{Context, Result, bail};
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

#[derive(Deserialize, Debug, Clone)]
struct GeminiApiResponse {
    candidates: Option<Vec<Candidate>>,
    error: Option<ApiErrorDetail>,
}

#[derive(Deserialize, Debug, Clone)]
struct Candidate {
    content: Option<ModelContent>,
}

#[derive(Deserialize, Debug, Clone)]
struct ModelContent {
    parts: Option<Vec<ModelPart>>,
}

#[derive(Deserialize, Debug, Clone)]
struct ModelPart {
    text: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
struct ApiErrorDetail {
    code: i32,
    message: String,
    status: String,
}

fn process_api_response_candidates(
    api_response_candidates: Option<Vec<Candidate>>,
    max_suggestions_to_return: u32,
) -> Result<Vec<String>> {
    let mut suggestions = Vec::new();
    if let Some(candidates_vec) = api_response_candidates {
        for candidate in candidates_vec {
            if let Some(content) = candidate.content {
                if let Some(parts) = content.parts {
                    for part in parts {
                        if let Some(text_block) = part.text {
                            let mut processed_text = text_block.trim();

                            if processed_text.starts_with("```\n")
                                && processed_text.ends_with("\n```")
                            {
                                processed_text = processed_text
                                    .strip_prefix("```\n")
                                    .unwrap_or(processed_text)
                                    .strip_suffix("\n```")
                                    .unwrap_or(processed_text)
                                    .trim();
                            } else if processed_text.starts_with("```")
                                && processed_text.ends_with("```")
                            {
                                processed_text = processed_text
                                    .strip_prefix("```")
                                    .unwrap_or(processed_text)
                                    .strip_suffix("```")
                                    .unwrap_or(processed_text)
                                    .trim();
                            }

                            for line in processed_text.lines() {
                                let trimmed_line = line.trim();
                                if !trimmed_line.is_empty() && trimmed_line != "```" {
                                    suggestions.push(trimmed_line.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if suggestions.len() > max_suggestions_to_return as usize {
        suggestions.truncate(max_suggestions_to_return as usize);
    }

    if suggestions.is_empty() {
        bail!("No suggestions derived from API candidates after processing.");
    }

    Ok(suggestions)
}

pub async fn generate_text(prompt_text: &str, num_api_candidates: u32) -> Result<Vec<String>> {
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
            candidate_count: Some(num_api_candidates.max(1)),
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

    process_api_response_candidates(response_body.candidates, num_api_candidates)
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
            assert!(
                e.to_string()
                    .contains("GEMINI_API_KEY environment variable not set.")
            );
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
        let prompt =
            "Suggest three names for a new tech startup focused on AI. Each name on a new line.";
        let suggestions = generate_text(prompt, 3).await?;
        assert_eq!(suggestions.len(), 3);
        for suggestion in suggestions {
            assert!(!suggestion.is_empty());
        }
        Ok(())
    }

    fn create_mock_candidate(text: &str) -> Candidate {
        Candidate {
            content: Some(ModelContent {
                parts: Some(vec![ModelPart {
                    text: Some(text.to_string()),
                }]),
            }),
        }
    }

    #[test]
    fn test_process_empty_candidates() {
        let result = process_api_response_candidates(None, 3);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("No suggestions derived")
        );

        let result_empty_vec = process_api_response_candidates(Some(vec![]), 3);
        assert!(result_empty_vec.is_err());
        assert!(
            result_empty_vec
                .unwrap_err()
                .to_string()
                .contains("No suggestions derived")
        );
    }

    #[test]
    fn test_process_single_clean_suggestion() {
        let candidates = vec![create_mock_candidate("feat: A single clean suggestion")];
        let result = process_api_response_candidates(Some(candidates), 1).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], "feat: A single clean suggestion");
    }

    #[test]
    fn test_process_markdown_stripping_and_splitting() {
        let text_block = "```\nfeat: Suggestion one\nfix: Suggestion two\n```";
        let candidates = vec![create_mock_candidate(text_block)];
        let result = process_api_response_candidates(Some(candidates), 2).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "feat: Suggestion one");
        assert_eq!(result[1], "fix: Suggestion two");

        let text_block_no_nl = "```feat: Suggestion alpha\nchore: Suggestion beta```";
        let candidates_no_nl = vec![create_mock_candidate(text_block_no_nl)];
        let result_no_nl = process_api_response_candidates(Some(candidates_no_nl), 2).unwrap();
        assert_eq!(result_no_nl.len(), 2);
        assert_eq!(result_no_nl[0], "feat: Suggestion alpha");
        assert_eq!(result_no_nl[1], "chore: Suggestion beta");
    }

    #[test]
    fn test_process_stray_markdown_fences_and_empty_lines() {
        let text_block = "```\nfeat: Valid one\n\n```\nfix: Valid two\n  ```  \nchore: Valid three";
        let candidates = vec![create_mock_candidate(text_block)];
        let result = process_api_response_candidates(Some(candidates), 3).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], "feat: Valid one");
        assert_eq!(result[1], "fix: Valid two");
        assert_eq!(result[2], "chore: Valid three");
    }

    #[test]
    fn test_process_truncation() {
        let candidates = vec![
            create_mock_candidate("s1"),
            create_mock_candidate("s2\ns3"),
            create_mock_candidate("s4\ns5\ns6"),
        ];
        let result = process_api_response_candidates(Some(candidates), 3).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], "s1");
        assert_eq!(result[1], "s2");
        assert_eq!(result[2], "s3");

        let result_request_more_than_available =
            process_api_response_candidates(Some(vec![create_mock_candidate("one\ntwo")]), 5)
                .unwrap();
        assert_eq!(result_request_more_than_available.len(), 2);
        assert_eq!(result_request_more_than_available[0], "one");
        assert_eq!(result_request_more_than_available[1], "two");
    }

    #[test]
    fn test_process_no_text_in_part() {
        let candidate_no_text = Candidate {
            content: Some(ModelContent {
                parts: Some(vec![ModelPart { text: None }]),
            }),
        };
        let result = process_api_response_candidates(Some(vec![candidate_no_text]), 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_process_no_parts_in_content() {
        let candidate_no_parts = Candidate {
            content: Some(ModelContent { parts: None }),
        };
        let result = process_api_response_candidates(Some(vec![candidate_no_parts]), 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_process_no_content_in_candidate() {
        let candidate_no_content = Candidate { content: None };
        let result = process_api_response_candidates(Some(vec![candidate_no_content]), 1);
        assert!(result.is_err());
    }
}
