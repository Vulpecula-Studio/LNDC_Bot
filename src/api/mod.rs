mod models;

use anyhow::{anyhow, Context, Result};
use reqwest::{header, Client};
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info};
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
    pub async fn get_chat_response<Fut>(
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
        // 可选的事件回调
        mut on_event: impl FnMut(&str, &str) -> Fut + Send,
    ) -> Result<ChatResponse>
    where
        Fut: std::future::Future<Output = Result<()>> + Send,
    {
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

        // DEBUG级：记录请求体JSON
        debug!("请求体 JSON: {}", serde_json::to_string(&request).unwrap_or_default());

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
        let mut fast_answer = String::new();
        let mut answer_delta = String::new();
        let mut current_event = String::new();
        let mut byte_stream = response.bytes_stream();
        let mut done = false;
        while let Some(item) = byte_stream.next().await {
            let chunk = item.context("读取流式数据失败")?;
            let text = String::from_utf8_lossy(&chunk);
            debug!("SSE 原始数据: {}", text);
            for line in text.lines() {
                if let Some(evt) = line.strip_prefix("event: ") {
                    current_event = evt.to_string();
                    // 仅记录事件名称，不单独输出
                } else if let Some(data) = line.strip_prefix("data: ") {
                    debug!("SSE 事件 [{}] 数据: {}", current_event, data);
                    // 记录事件与完整数据
                    events.push((current_event.clone(), data.to_string()));
                    // 实时回调事件
                    on_event(&current_event, data).await?;
                    // 处理 fastAnswer 和 answer 事件，仅追加非空内容并根据 finish_reason 结束
                    if current_event == "fastAnswer" || current_event == "answer" {
                        if let Ok(resp_val) = serde_json::from_str::<serde_json::Value>(data) {
                            // 提取非空增量内容
                            if let Some(delta) = resp_val["choices"][0]["delta"]["content"]
                                .as_str()
                                .filter(|s| !s.trim().is_empty())
                            {
                                if current_event == "fastAnswer" {
                                    fast_answer.push_str(delta);
                                } else {
                                    answer_delta.push_str(delta);
                                }
                                debug!("收到 non-empty {} 增量: {}", current_event, delta);
                            }
                            // 如果对应 buffer 为空，则尝试完整回答
                            let buffer = if current_event == "fastAnswer" {
                                &mut fast_answer
                            } else {
                                &mut answer_delta
                            };
                            if buffer.is_empty() {
                                if let Some(full) = resp_val["choices"][0]["message"]["content"]
                                    .as_str()
                                    .filter(|s| !s.trim().is_empty())
                                {
                                    buffer.push_str(full);
                                    debug!("收到 {} 完整回答: {}", current_event, full);
                                }
                            }
                            // finish_reason stop 时结束循环
                            if let Some(reason) = resp_val["choices"][0]["finish_reason"]
                                .as_str()
                            {
                                if reason == "stop" {
                                    done = true;
                                }
                            }
                        }
                    }
                }
            }
            if done {
                break;
            }
        }
        // 合并 fastAnswer 与 answer 两种事件的内容
        let content = format!("{}{}", fast_answer, answer_delta);
        debug!("成功解析API响应，内容长度: {} 字符", content.len());

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
        _image_urls: Option<&[String]>,
    ) -> Result<ImageResponse> {
        // 创建会话
        let session_id = self.session_manager.create_session(user_id)?;

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
            .get_chat_response(
                Some(session_id.clone()),
                None,
                messages,
                false,
                false,
                None,
                |_, _| async { Ok(()) },
            )
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
    #[allow(dead_code)]
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
