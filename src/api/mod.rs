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
    config: Config,
    pub session_manager: SessionManager,
    image_generator: ImageGenerator,
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
        prompt: &str,
        image_urls: Option<&[String]>,
    ) -> Result<ChatResponse> {
        // 并发请求限流
        let _permit = self
            .semaphore
            .clone()
            .acquire_owned()
            .await
            .expect("Semaphore closed");

        // 构建消息内容：文本和可选图片
        let mut content_items = vec![json!({
            "type": "text",
            "text": prompt
        })];
        if let Some(urls) = image_urls {
            for url in urls {
                content_items.push(json!({
                    "type": "image_url",
                    "image_url": {"url": url}
                }));
            }
        }
        let content = json!(content_items);

        // 创建FastGPT消息
        let message = FastGPTMessage {
            role: "user".to_string(),
            content,
        };

        // 创建请求体 - stream和detail将使用struct中的默认值(false)
        let request = FastGPTChatRequest {
            chat_id: Some(format!("discord_{}", Uuid::new_v4())),
            response_chat_item_id: Some(format!("resp_{}", Uuid::new_v4())),
            variables: Some(json!({
                "uid": format!("user_{}", Uuid::new_v4()),
                "name": "DiscordUser"
            })),
            messages: vec![message],
            stream: false, // 明确设置为false，与默认值一致
            detail: false, // 明确设置为false，与默认值一致
        };

        info!("发送FastGPT请求，提示词长度: {}", prompt.len());
        if let Some(urls) = image_urls {
            if !urls.is_empty() {
                info!("包含{}张图片", urls.len());
                for (i, url) in urls.iter().enumerate() {
                    debug!("图片URL {}: {}", i + 1, url);
                }
            }
        }

        // 发送请求，重试逻辑
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
            info!("重试请求，第 {} 次，等待 {} 秒", attempts, backoff.as_secs());
            sleep(backoff).await;
        };

        // 检查响应状态已经在重试中保证是成功的

        // 获取原始响应文本以便日志记录
        let response_text = response.text().await.context("读取API响应文本失败")?;
        info!("收到FastGPT响应，长度: {} 字节", response_text.len());
        debug!(
            "FastGPT响应内容: {}",
            if response_text.len() > 200 {
                format!("{}...(已截断)", safe_truncate(&response_text, 200))
            } else {
                response_text.clone()
            }
        );

        // 尝试解析响应
        let api_response: ChatCompletionResponse = match serde_json::from_str(&response_text) {
            Ok(res) => res,
            Err(e) => {
                error!("解析API响应JSON失败: {}, 原始响应: {}", e, &response_text);
                return Err(anyhow!("解析API响应失败: {}", e));
            }
        };

        // 提取响应内容
        let content = match api_response.choices.get(0) {
            Some(choice) => choice.message.content.clone(),
            None => {
                error!("API响应中没有选项: {:?}", api_response);
                return Err(anyhow!("API响应中没有选项"));
            }
        };

        info!("成功解析API响应，内容长度: {} 字符", content.len());

        Ok(ChatResponse {
            content,
            raw_response: api_response,
        })
    }

    /// 获取响应并生成图片
    pub async fn get_response_as_image(
        &self,
        prompt: &str,
        user_id: &str,
        image_urls: Option<&[String]>,
    ) -> Result<ImageResponse> {
        // 创建会话
        let session_id = self.session_manager.create_session(user_id);

        // 保存用户输入
        self.session_manager.save_user_input(&session_id, prompt).await?;

        // 从API获取响应
        let chat_response = self.get_chat_response(prompt, image_urls).await?;

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
}

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
