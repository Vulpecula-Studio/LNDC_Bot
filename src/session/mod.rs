use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{error, info};
use uuid::Uuid;

use crate::config::Config;

#[derive(Debug, Clone)]
pub struct SessionManager {
    sessions_dir: PathBuf,
}

impl SessionManager {
    pub fn new(config: &Config) -> Self {
        let sessions_dir = config.data_dir.join("sessions");

        // 确保会话目录存在
        if !sessions_dir.exists() {
            if let Err(e) = fs::create_dir_all(&sessions_dir) {
                error!("创建会话目录失败: {}", e);
            }
        }

        SessionManager { sessions_dir }
    }

    /// 创建新的会话
    pub fn create_session(&self, user_id: &str) -> String {
        // 生成会话ID
        let session_id = Uuid::new_v4().to_string();

        // 创建会话目录
        let session_dir = self.get_session_dir(&session_id);
        if let Err(e) = fs::create_dir_all(&session_dir) {
            error!("创建会话目录失败: {}", e);
        }

        // 保存用户ID
        let user_info_path = session_dir.join("user_id.txt");
        if let Err(e) = fs::write(&user_info_path, user_id) {
            error!("保存用户ID失败: {}", e);
        }

        session_id
    }

    /// 获取会话目录
    pub fn get_session_dir(&self, session_id: &str) -> PathBuf {
        self.sessions_dir.join(session_id)
    }

    /// 保存用户输入到会话
    pub async fn save_user_input(&self, session_id: &str, input: &str) -> Result<()> {
        let session_dir = self.get_session_dir(session_id);
        let input = input.to_string();
        tokio::task::spawn_blocking(move || {
            let input_file = session_dir.join("input.txt");
            fs::write(&input_file, input).context("保存用户输入失败")
        })
        .await
        .context("保存用户输入任务失败")?;
        Ok(())
    }

    /// 保存API响应到会话
    pub async fn save_response_markdown(&self, session_id: &str, markdown: &str) -> Result<()> {
        let session_dir = self.get_session_dir(session_id);
        let markdown = markdown.to_string();
        tokio::task::spawn_blocking(move || {
            let response_file = session_dir.join("response.md");
            fs::write(&response_file, markdown).context("保存API响应失败")
        })
        .await
        .context("保存API响应任务失败")?;
        Ok(())
    }

    /// 保存响应图片到会话
    pub async fn save_response_image(
        &self,
        session_id: &str,
        original_image_path: &Path,
    ) -> Result<PathBuf> {
        let session_dir = self.get_session_dir(session_id);
        let original = original_image_path.to_path_buf();
        tokio::task::spawn_blocking(move || -> Result<PathBuf> {
            // 生成时间戳
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            let filename = format!("response_{}.png", now);
            let target_path = session_dir.join(&filename);
            fs::copy(&original, &target_path).context("复制响应图片到会话目录失败")?;
            Ok(target_path)
        })
        .await
        .context("保存响应图片任务失败")?
    }

    /// 获取会话列表
    pub fn get_user_sessions(&self, user_id: &str) -> Vec<SessionInfo> {
        let mut sessions = Vec::new();

        // 遍历会话目录
        if let Ok(entries) = fs::read_dir(&self.sessions_dir) {
            for entry in entries.filter_map(Result::ok) {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_dir() {
                        // 检查这个会话是否属于该用户
                        let session_path = entry.path();
                        let user_id_file = session_path.join("user_id.txt");

                        if let Ok(stored_user_id) = fs::read_to_string(&user_id_file) {
                            if stored_user_id.trim() == user_id {
                                // 提取会话信息
                                if let Some(session_id) = session_path.file_name() {
                                    if let Some(session_id) = session_id.to_str() {
                                        if let Some(info) = self.get_session_info(session_id) {
                                            sessions.push(info);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // 按最后修改时间排序
        sessions.sort_by(|a, b| b.last_modified.cmp(&a.last_modified));

        sessions
    }

    /// 获取会话信息
    fn get_session_info(&self, session_id: &str) -> Option<SessionInfo> {
        let session_dir = self.get_session_dir(session_id);

        // 读取用户输入
        let input_path = session_dir.join("input.txt");
        let input_preview = match fs::read_to_string(&input_path) {
            Ok(content) => {
                // 提取前30个字符作为预览
                if content.len() > 30 {
                    format!("{}...", &content[..30])
                } else {
                    content.clone()
                }
            }
            Err(_) => String::from("无法读取输入"),
        };

        // 获取目录的最后修改时间
        let modified = fs::metadata(&session_dir)
            .ok()
            .and_then(|m| m.modified().ok())
            .unwrap_or_else(|| SystemTime::now());

        let datetime = DateTime::<Utc>::from(modified);

        let images = fs::read_dir(&session_dir)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .filter(|e| {
                        if let Ok(file_type) = e.file_type() {
                            if file_type.is_file() {
                                if let Some(name) = e.path().file_name() {
                                    if let Some(name_str) = name.to_str() {
                                        return name_str.ends_with(".png")
                                            || name_str.ends_with(".jpg")
                                            || name_str.ends_with(".jpeg");
                                    }
                                }
                            }
                        }
                        false
                    })
                    .count()
            })
            .unwrap_or(0);

        Some(SessionInfo {
            id: session_id.to_string(),
            input_preview,
            last_modified: datetime,
            images: images as u32,
        })
    }

    /// 清理会话中的图片
    pub fn cleanup_session_images(&self, session_id: &str) -> Result<usize> {
        let session_dir = self.get_session_dir(session_id);

        if !session_dir.exists() {
            return Ok(0);
        }

        let mut removed = 0;

        if let Ok(entries) = fs::read_dir(&session_dir) {
            for entry in entries.filter_map(Result::ok) {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_file() {
                        let path = entry.path();
                        if let Some(ext) = path.extension() {
                            if let Some(ext_str) = ext.to_str() {
                                if ext_str == "png" || ext_str == "jpg" || ext_str == "jpeg" {
                                    if fs::remove_file(&path).is_ok() {
                                        removed += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // 创建清理标记
        let cleaned_marker = session_dir.join(".cleaned");
        let timestamp = Utc::now().to_rfc3339();
        let _ = fs::write(cleaned_marker, format!("图片已于 {} 清理", timestamp));

        Ok(removed)
    }

    /// 定期清理旧会话
    pub async fn periodic_cleanup(&self, expiry_days: u64) {
        let seconds_in_day = 24 * 60 * 60;
        let expiry_seconds = expiry_days * seconds_in_day;

        if let Ok(entries) = fs::read_dir(&self.sessions_dir) {
            let now = SystemTime::now();
            let mut cleaned_sessions = 0;
            let mut cleaned_files = 0;

            for entry in entries.filter_map(Result::ok) {
                let session_path = entry.path();
                // 如果缺少user_id.txt，视为脱离管理的会话，清理其图片
                let user_file = session_path.join("user_id.txt");
                if !user_file.exists() {
                    if let Some(session_id) = session_path.file_name().and_then(|n| n.to_str()) {
                        match self.cleanup_session_images(session_id) {
                            Ok(count) => {
                                if count > 0 {
                                    cleaned_sessions += 1;
                                    cleaned_files += count;
                                    info!("已清理脱离管理会话 {} 中的 {} 个图片文件", session_id, count);
                                }
                            }
                            Err(e) => error!("清理脱离管理会话 {} 图片时出错: {}", session_id, e),
                        }
                    }
                    continue;
                }

                if let Ok(metadata) = entry.metadata() {
                    if metadata.is_dir() {
                        if let Ok(modified) = metadata.modified() {
                            if let Ok(duration) = now.duration_since(modified) {
                                if duration.as_secs() > expiry_seconds {
                                    // 会话已过期，清理图片
                                    if let Some(filename) = session_path.file_name().and_then(|n| n.to_str()) {
                                        match self.cleanup_session_images(filename) {
                                            Ok(count) => {
                                                if count > 0 {
                                                    cleaned_sessions += 1;
                                                    cleaned_files += count;
                                                    info!(
                                                        "已清理会话 {} 中的 {} 个图片文件",
                                                        filename, count
                                                    );
                                                }
                                            }
                                            Err(e) => error!(
                                                "清理会话 {} 图片时出错: {}",
                                                filename, e
                                            ),
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if cleaned_sessions > 0 {
                info!(
                    "定期清理完成: 已处理 {} 个会话，删除 {} 个图片文件",
                    cleaned_sessions, cleaned_files
                );
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub id: String,
    pub input_preview: String,
    pub last_modified: DateTime<Utc>,
    pub images: u32,
}
