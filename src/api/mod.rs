mod models;

use anyhow::{anyhow, Context, Result};
use reqwest::{header, Client};
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::time::{sleep, Duration};
use tracing::{error, info};
use uuid::Uuid;

use crate::config::Config;
use crate::image::ImageGenerator;
use crate::session::SessionManager;

pub use self::models::*;

#[derive(Debug)]
pub struct APIClient {
    client: Client,
    pub config: Config,
    pub session_manager: SessionManager,
    pub image_generator: ImageGenerator,
    semaphore: Arc<Semaphore>,
}

impl APIClient {
    pub fn new(config: Config) -> Result<Self> {
        // 创建HTTP客户端
        let mut headers = header::HeaderMap::new();
        headers.insert(
            "Authorization",
            header::HeaderValue::from_str(&format!("Bearer {}", config.fastgpt_auth_token))
                .context("无效的授权令牌")?,
        );

        let client = Client::builder()
            .default_headers(headers)
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .context("创建HTTP客户端失败")?;

        // 创建会话管理器
        let session_manager = SessionManager::new(&config);

        // 创建图像生成器
        let image_generator = ImageGenerator::new(&config)?;

        // 并发请求限流
        let semaphore = Arc::new(Semaphore::new(config.api_concurrency_limit));

        Ok(Self {
            client,
            config,
            session_manager,
            image_generator,
            semaphore,
        })
    }

    /// 从FastGPT获取响应
    pub async fn get_chat_response(
        &self,
        // 可选的对话 ID，不传则不使用上下文
        chat_id: Option<String>,
        // 可选的响应消息 ID，用于存储本次响应
        response_chat_item_id: Option<String>,
        // GPT 聊天消息列表
        messages: Vec<FastGPTMessage>,
        // 是否流式
        stream: bool,
        // 是否返回详细信息
        detail: bool,
        // 可选的模块变量
        variables: Option<serde_json::Value>,
    ) -> Result<ChatResponse> {
        // 并发请求限流
        let _permit = self
            .semaphore
            .clone()
            .acquire_owned()
            .await
            .expect("Semaphore closed");

        // 在 move `messages` 之前捕获其长度
        let msg_count = messages.len();
        // 构建请求体
        let request = FastGPTChatRequest {
            chat_id,
            response_chat_item_id,
            messages,
            stream,
            detail,
            variables,
        };

        // 记录消息数量
        info!(
            "发送FastGPT请求，消息数: {}, stream: {}, detail: {}",
            msg_count, stream, detail
        );

        // 发送请求并流式读取SSE事件，重试逻辑保持不变
        let max_retries = 3;
        let mut attempts = 0;
        let response = loop {
            attempts += 1;
            let send_result = self
                .client
                .post(&self.config.fastgpt_api_url)
                .json(&request)
                .send()
                .await;
            match send_result {
                Ok(resp) if resp.status().is_success() => break resp,
                Ok(resp) => {
                    let status = resp.status();
                    let error_text = resp.text().await.unwrap_or_default();
                    error!("API请求失败: 状态码 {}, 错误信息: {}", status, error_text);
                    if attempts >= max_retries {
                        return Err(anyhow!("API请求失败: {}, {}", status, error_text));
                    }
                }
                Err(e) => {
                    error!("发送API请求失败: {}", e);
                    if attempts >= max_retries {
                        return Err(anyhow!("发送API请求失败: {}", e));
                    }
                }
            }
            let backoff = Duration::from_secs(2_u64.pow(attempts));
            info!(
                "重试请求，第 {} 次，等待 {} 秒",
                attempts,
                backoff.as_secs()
            );
            sleep(backoff).await;
        };

        // 解析流式SSE事件
        use futures::StreamExt;
        let mut events = Vec::new();
        let mut answer = String::new();
        let mut current_event = String::new();
        let mut byte_stream = response.bytes_stream();
        let mut done = false;
        while let Some(item) = byte_stream.next().await {
            let chunk = item.context("读取流式数据失败")?;
            let text = String::from_utf8_lossy(&chunk);
            for line in text.lines() {
                if let Some(evt) = line.strip_prefix("event: ") {
                    current_event = evt.to_string();
                    info!("SSE 事件: {}", &current_event);
                } else if let Some(data) = line.strip_prefix("data: ") {
                    events.push((current_event.clone(), data.to_string()));
                    // 如果收到 fastAnswer，处理其内容并结束流式传输
                    if current_event == "fastAnswer" {
                        // 处理 fastAnswer 事件内容
                        if let Ok(resp_val) = serde_json::from_str::<serde_json::Value>(data) {
                            if let Some(delta) = resp_val["choices"][0]["delta"]["content"].as_str()
                            {
                                answer.push_str(delta);
                                info!("收到 fastAnswer 增量: {}", delta);
                            }
                        }
                        done = true;
                        break;
                    }
                    if current_event == "answer" {
                        if let Ok(resp_val) = serde_json::from_str::<serde_json::Value>(data) {
                            if let Some(delta) = resp_val["choices"][0]["delta"]["content"].as_str()
                            {
                                answer.push_str(delta);
                                info!("收到答案增量: {}", delta);
                            }
                        }
                    }
                }
            }
            if done {
                break;
            }
        }
        info!("流式传输结束，最终回答: {}", safe_truncate(&answer, 200));
        let content = answer;

        info!("成功解析API响应，内容长度: {} 字符", content.len());

        Ok(ChatResponse {
            content,
            raw_response: ChatCompletionResponse {
                // 补全默认字段
                id: "".to_string(),
                object: "".to_string(),
                created: 0,
                model: "".to_string(),
                choices: vec![],
                usage: Default::default(), // 添加默认 usage
            },
            events,
        })
    }

    /// 获取响应并生成图片
    #[allow(dead_code)]
    pub async fn get_response_as_image(
        &self,
        prompt: &str,
        user_id: &str,
        image_urls: Option<&[String]>,
    ) -> Result<ImageResponse> {
        // 创建会话
        let session_id = self.session_manager.create_session(user_id);

        // 保存用户输入
        self.session_manager
            .save_user_input(&session_id, prompt)
            .await?;

        // 构建 messages 并从 API 获取响应
        let messages = vec![FastGPTMessage {
            role: "user".into(),
            content: json!([{"type": "text", "text": prompt}]),
        }];
        let chat_response = self
            .get_chat_response(Some(session_id.clone()), None, messages, false, false, None)
            .await?;

        // 保存响应内容
        self.session_manager
            .save_response_markdown(&session_id, &chat_response.content)
            .await?;

        // 生成图片
        let temp_dir = self.config.image_output_dir.join("temp");
        if !temp_dir.exists() {
            fs::create_dir_all(&temp_dir)?;
        }

        let output_filename = format!("response_{}.png", Uuid::new_v4());
        let output_path = temp_dir.join(&output_filename);

        // 使用图像生成器创建图片
        let image_path = self
            .image_generator
            .create_image_from_markdown(&chat_response.content, &output_path)?;

        // 保存图片到会话
        let final_image_path = self
            .session_manager
            .save_response_image(&session_id, &image_path)
            .await?;

        // 尝试删除临时图片
        let _ = fs::remove_file(image_path);

        Ok(ImageResponse {
            image_path: final_image_path,
            session_id,
            #[allow(dead_code)]
            markdown_text: chat_response.content,
        })
    }

    // 安全截断UTF-8字符串的辅助函数
    #[allow(dead_code)]
    fn safe_truncate(s: &str, max_len: usize) -> String {
        // 调用模块内的 free 函数
        crate::api::safe_truncate(s, max_len)
    }
}

pub struct ChatResponse {
    pub content: String,
    #[allow(dead_code)]
    pub raw_response: ChatCompletionResponse,
    /// 流式事件 (event, data)
    pub events: Vec<(String, String)>,
}

#[allow(dead_code)]
pub struct ImageResponse {
    pub image_path: PathBuf,
    pub session_id: String,
    #[allow(dead_code)]
    pub markdown_text: String,
}

// 安全截断UTF-8字符串的辅助函数
fn safe_truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        return s.to_string();
    }

    // 截断到指定字符数
    s.char_indices()
        .nth(max_len)
        .map_or(s.to_string(), |(idx, _)| s[..idx].to_string())
}

#[cfg(test)]
mod tests {
    use super::safe_truncate;

    #[test]
    fn truncate_short() {
        assert_eq!(safe_truncate("hello", 10), "hello");
    }

    #[test]
    fn truncate_exact() {
        let s = "1234567890";
        assert_eq!(safe_truncate(s, 10), s);
    }

    #[test]
    fn truncate_long() {
        let s = "a".repeat(100);
        let t = safe_truncate(&s, 10);
        assert_eq!(t.chars().count(), 10);
    }
}
