use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct ChatCompletionResponse {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub created: u64,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub choices: Vec<ChatCompletionChoice>,
    #[serde(default)]
    pub usage: Usage,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct ChatCompletionChoice {
    #[serde(default)]
    pub index: u32,
    #[serde(default)]
    pub message: ChatCompletionMessage,
    #[serde(default)]
    pub finish_reason: String,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct ChatCompletionMessage {
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub content: String,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct Usage {
    #[serde(default = "default_token_count")]
    pub prompt_tokens: u32,
    #[serde(default = "default_token_count")]
    pub completion_tokens: u32,
    #[serde(default = "default_token_count")]
    pub total_tokens: u32,
}

fn default_token_count() -> u32 {
    1
}

// FastGPT API请求所需的新结构体
#[derive(Debug, Serialize, Default)]
pub struct FastGPTChatRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_chat_item_id: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub stream: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub detail: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables: Option<serde_json::Value>,
    pub messages: Vec<FastGPTMessage>,
}

#[derive(Debug, Serialize)]
pub struct FastGPTMessage {
    pub role: String,
    pub content: serde_json::Value,
}
