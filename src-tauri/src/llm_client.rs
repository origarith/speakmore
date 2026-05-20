use crate::settings::{PostProcessProvider, PostProcessStructuredOutputMode};
use log::debug;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE, USER_AGENT};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct JsonSchema {
    name: String,
    strict: bool,
    schema: Value,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum ResponseFormat {
    JsonSchema {
        #[serde(rename = "type")]
        format_type: String,
        json_schema: JsonSchema,
    },
    JsonObject {
        #[serde(rename = "type")]
        format_type: String,
    },
}

#[derive(Debug, Serialize, Clone, Default)]
pub struct ReasoningConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude: Option<bool>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ThinkingConfig {
    #[serde(rename = "type")]
    pub thinking_type: String,
}

#[derive(Debug, Clone)]
pub struct ChatCompletionOptions {
    pub system_prompt: Option<String>,
    pub structured_output_mode: PostProcessStructuredOutputMode,
    pub json_schema: Option<Value>,
    pub reasoning_effort: Option<String>,
    pub reasoning: Option<ReasoningConfig>,
    pub thinking: Option<ThinkingConfig>,
}

impl Default for ChatCompletionOptions {
    fn default() -> Self {
        Self {
            system_prompt: None,
            structured_output_mode: PostProcessStructuredOutputMode::None,
            json_schema: None,
            reasoning_effort: None,
            reasoning: None,
            thinking: None,
        }
    }
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<ResponseFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning: Option<ReasoningConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<ThinkingConfig>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessageResponse,
}

#[derive(Debug, Deserialize)]
struct ChatMessageResponse {
    content: Option<String>,
}

/// Build headers for API requests based on provider type
fn build_headers(provider: &PostProcessProvider, api_key: &str) -> Result<HeaderMap, String> {
    let mut headers = HeaderMap::new();

    // Common headers
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static(concat!("SpeakMore/", env!("CARGO_PKG_VERSION"))),
    );
    headers.insert("X-Title", HeaderValue::from_static("SpeakMore"));

    // Provider-specific auth headers
    if !api_key.is_empty() {
        if provider.id == "anthropic" {
            headers.insert(
                "x-api-key",
                HeaderValue::from_str(api_key)
                    .map_err(|e| format!("Invalid API key header value: {}", e))?,
            );
            headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
        } else {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {}", api_key))
                    .map_err(|e| format!("Invalid authorization header value: {}", e))?,
            );
        }
    }

    Ok(headers)
}

/// Create an HTTP client with provider-specific headers
fn create_client(provider: &PostProcessProvider, api_key: &str) -> Result<reqwest::Client, String> {
    let headers = build_headers(provider, api_key)?;
    reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))
}

fn build_messages(system_prompt: Option<String>, user_content: String) -> Vec<ChatMessage> {
    let mut messages = Vec::new();

    if let Some(system) = system_prompt {
        messages.push(ChatMessage {
            role: "system".to_string(),
            content: system,
        });
    }

    messages.push(ChatMessage {
        role: "user".to_string(),
        content: user_content,
    });

    messages
}

fn build_response_format(options: &ChatCompletionOptions) -> Option<ResponseFormat> {
    match options.structured_output_mode {
        PostProcessStructuredOutputMode::None => None,
        PostProcessStructuredOutputMode::OpenAiJsonSchema => {
            options
                .json_schema
                .clone()
                .map(|schema| ResponseFormat::JsonSchema {
                    format_type: "json_schema".to_string(),
                    json_schema: JsonSchema {
                        name: "transcription_output".to_string(),
                        strict: true,
                        schema,
                    },
                })
        }
        PostProcessStructuredOutputMode::JsonObject => Some(ResponseFormat::JsonObject {
            format_type: "json_object".to_string(),
        }),
    }
}

fn build_chat_completion_request(
    model: &str,
    user_content: String,
    options: ChatCompletionOptions,
) -> ChatCompletionRequest {
    let messages = build_messages(options.system_prompt.clone(), user_content);
    let response_format = build_response_format(&options);

    ChatCompletionRequest {
        model: model.to_string(),
        messages,
        response_format,
        reasoning_effort: options.reasoning_effort,
        reasoning: options.reasoning,
        thinking: options.thinking,
    }
}

#[cfg(test)]
pub(crate) fn build_chat_completion_request_json(
    model: &str,
    user_content: String,
    options: ChatCompletionOptions,
) -> Value {
    serde_json::to_value(build_chat_completion_request(model, user_content, options))
        .expect("chat completion request serializes")
}

/// Send a chat completion request to an OpenAI-compatible API.
/// Optional fields must already be filtered by the provider capability profile.
pub async fn send_chat_completion(
    provider: &PostProcessProvider,
    api_key: String,
    model: &str,
    user_content: String,
    options: ChatCompletionOptions,
) -> Result<Option<String>, String> {
    let base_url = provider.base_url.trim_end_matches('/');
    let url = format!("{}/chat/completions", base_url);

    debug!("Sending chat completion request to: {}", url);

    let client = create_client(provider, &api_key)?;
    let request_body = build_chat_completion_request(model, user_content, options);

    let response = client
        .post(&url)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Failed to read error response".to_string());
        return Err(summarize_http_error(status, &error_text));
    }

    let completion: ChatCompletionResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse API response: {}", e))?;

    Ok(completion
        .choices
        .first()
        .and_then(|choice| choice.message.content.clone()))
}

fn summarize_http_error(status: reqwest::StatusCode, error_text: &str) -> String {
    let message = serde_json::from_str::<Value>(error_text)
        .ok()
        .and_then(|value| {
            value
                .get("error")
                .and_then(|error| error.get("message"))
                .and_then(Value::as_str)
                .or_else(|| value.get("message").and_then(Value::as_str))
                .map(str::to_string)
        })
        .unwrap_or_else(|| error_text.to_string())
        .replace(['\n', '\r'], " ");

    let normalized = message.to_lowercase();
    let summary = if normalized.contains("reasoning_effort")
        && (normalized.contains("unknown variant") || normalized.contains("invalid"))
    {
        "Provider rejected the configured reasoning_effort value. Disable thinking or choose a supported reasoning effort for this provider.".to_string()
    } else {
        message
    };

    let mut truncated: String = summary.chars().take(240).collect();
    if summary.chars().count() > 240 {
        truncated.push_str("...");
    }
    format!("API request failed with status {}: {}", status, truncated)
}

/// Fetch available models from an OpenAI-compatible API
/// Returns a list of model IDs
pub async fn fetch_models(
    provider: &PostProcessProvider,
    api_key: String,
) -> Result<Vec<String>, String> {
    let base_url = provider.base_url.trim_end_matches('/');
    let endpoint = provider
        .models_endpoint
        .as_deref()
        .ok_or_else(|| format!("{} does not expose a model list endpoint", provider.label))?;
    let url = if endpoint.starts_with("http://") || endpoint.starts_with("https://") {
        endpoint.to_string()
    } else {
        format!("{}/{}", base_url, endpoint.trim_start_matches('/'))
    };

    debug!("Fetching models from: {}", url);

    let client = create_client(provider, &api_key)?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch models: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!(
            "Model list request failed ({}): {}",
            status, error_text
        ));
    }

    let parsed: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let mut models = Vec::new();

    // Handle OpenAI format: { data: [ { id: "..." }, ... ] }
    if let Some(data) = parsed.get("data").and_then(|d| d.as_array()) {
        for entry in data {
            if let Some(id) = entry.get("id").and_then(|i| i.as_str()) {
                models.push(id.to_string());
            } else if let Some(name) = entry.get("name").and_then(|n| n.as_str()) {
                models.push(name.to_string());
            }
        }
    }
    // Handle array format: [ "model1", "model2", ... ]
    else if let Some(array) = parsed.as_array() {
        for entry in array {
            if let Some(model) = entry.as_str() {
                models.push(model.to_string());
            }
        }
    }

    Ok(models)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_schema_response_format_serializes_as_openai_schema() {
        let body = build_chat_completion_request_json(
            "test-model",
            "hello".to_string(),
            ChatCompletionOptions {
                structured_output_mode: PostProcessStructuredOutputMode::OpenAiJsonSchema,
                json_schema: Some(serde_json::json!({"type": "object"})),
                ..Default::default()
            },
        );

        assert_eq!(body["response_format"]["type"], "json_schema");
        assert!(body["response_format"].get("json_schema").is_some());
    }

    #[test]
    fn http_error_summary_hides_raw_reasoning_schema_error() {
        let summary = summarize_http_error(
            reqwest::StatusCode::BAD_REQUEST,
            r#"{"error":{"message":"Failed to deserialize the JSON body into the target type: reasoning_effort: unknown variant `none`, expected one of `high`, `low`, `medium`, `max`, `xhigh`"}}"#,
        );

        assert!(summary.contains("Provider rejected"));
        assert!(!summary.contains("unknown variant"));
        assert!(!summary.contains("`none`"));
    }
}
