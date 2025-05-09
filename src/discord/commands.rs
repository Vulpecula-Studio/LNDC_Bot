use anyhow::Result;
use chrono::{DateTime, Utc};
use poise::serenity_prelude as serenity;
use std::fmt::Write;
use std::sync::{Arc, Mutex};
use tracing::{debug, info};
use uuid::Uuid;

use super::Context;
use crate::api::FastGPTMessage;
use serde_json::json;

// 安全截断字符串助手函数
fn truncate(s: &str, max_len: usize) -> &str {
    if s.chars().count() <= max_len {
        s
    } else {
        // 确保在字符边界处截断
        s.char_indices()
            .nth(max_len)
            .map_or(s, |(idx, _)| &s[..idx])
    }
}

/// 新增通用问答流程，支持最多10张图片
async fn run_qa_flow(ctx: Context<'_>, question: String, image_urls: Vec<String>) -> Result<()> {
    // 调试级别：记录调用参数
    debug!(
        "run_qa_flow called: question='{0}', image_count={1}",
        question,
        image_urls.len()
    );
    // 构造 FastGPT 消息体
    let mut content_array = Vec::new();
    content_array.push(json!({"type":"text","text": question.clone()}));
    for url in &image_urls {
        content_array.push(json!({"type":"image_url","image_url":{"url": url}}));
    }
    // 调试级别：展示消息结构
    debug!("FastGPT messages: {:#?}", {
        let mut msgs = Vec::new();
        msgs.push(FastGPTMessage {
            role: "user".into(),
            content: json!(content_array.clone()),
        });
        msgs
    });
    let messages = vec![FastGPTMessage {
        role: "user".into(),
        content: json!(content_array),
    }];
    // 发送嵌入式初始确认消息
    let initial_msg = ctx
        .send(|reply| {
            reply.embed(|e| {
                e.title("✅ 请求已接收")
                    .description("正在等待fastgpt响应...")
                    .color(0x3498db)
            })
        })
        .await?;
    // 获取用户ID和 API 客户端
    let user_id = ctx.author().id.to_string();
    let api_client = &ctx.data().api_client;
    // 创建新的会话并记录
    let session_id = api_client.session_manager.create_session(&user_id)?;
    // 信息级别：记录简要提问
    info!(
        "用户{} 提问: {}",
        ctx.author().name,
        truncate(&question, 30)
    );
    // 调试级别：记录会话ID
    debug!("session_id: {}", session_id);
    // 调用 FastGPT 获取对话响应，启用流式与详细模式
    let status_lines: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let chat_resp = api_client
        .get_chat_response(None, None, messages, true, true, None, {
            let status_lines = Arc::clone(&status_lines);
            let ctx = ctx.clone();
            let initial_msg = initial_msg.clone();
            move |evt, data| {
                let status_lines = Arc::clone(&status_lines);
                let ctx = ctx.clone();
                let evt = evt.to_string();
                let data = data.to_string();
                let msg = initial_msg.clone();
                async move {
                    if evt == "flowNodeStatus" {
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&data) {
                            if val.get("status").and_then(|s| s.as_str()) == Some("running") {
                                if let Some(name) = val.get("name").and_then(|n| n.as_str()) {
                                    let description = {
                                        let mut lines = status_lines.lock().unwrap();
                                        if !lines.is_empty() {
                                            let last_index = lines.len() - 1;
                                            if lines[last_index].starts_with("🔄 丨") {
                                                let node =
                                                    lines[last_index].trim_start_matches("🔄 丨");
                                                lines[last_index] = format!("✅ 丨{}", node);
                                            }
                                        }
                                        lines.push(format!("🔄 丨{}", name));
                                        lines.join("\n")
                                    };
                                    msg.edit(ctx.clone(), |m| {
                                        m.embed(|e| {
                                            e.title("运行状态")
                                                .description(description.clone())
                                                .color(0x3498db)
                                        })
                                    })
                                    .await?;
                                }
                            }
                        }
                    }
                    Ok(())
                }
            }
        })
        .await?;
    // 调试级别：记录响应长度
    debug!("chat response length: {} ", chat_resp.content.len());
    // 添加完整响应状态
    {
        let history = status_lines.lock().unwrap().join("\n");
        initial_msg
            .edit(ctx.clone(), |m| {
                m.embed(|e| {
                    e.title("运行状态")
                        .description([history, "✅ 接收到fastgpt完整响应！".to_string()].join("\n"))
                        .color(0x2ecc71)
                })
            })
            .await?;
    }
    // 保存用户输入、响应和图片链接
    api_client
        .session_manager
        .save_user_input(&session_id, &question)
        .await?;
    api_client
        .session_manager
        .save_response_markdown(&session_id, &chat_resp.content)
        .await?;
    api_client
        .session_manager
        .save_user_images(&session_id, &image_urls)
        .await?;
    // 更新状态：图片生成中
    {
        let history = status_lines.lock().unwrap().join("\n");
        initial_msg
            .edit(ctx.clone(), |m| {
                m.embed(|e| {
                    e.title("运行状态")
                        .description([history, "图片生成中...".to_string()].join("\n"))
                        .color(0xf1c40f)
                })
            })
            .await?;
    }
    // 生成图片
    let session_dir = api_client.session_manager.get_session_dir(&session_id);
    let image_path = session_dir.join(format!("response_{}.png", Uuid::new_v4()));
    api_client
        .image_generator
        .create_image_from_markdown(&chat_resp.content, &image_path)?;
    // 更新状态：图片生成完成
    {
        let history = status_lines.lock().unwrap().join("\n");
        initial_msg
            .edit(ctx.clone(), |m| {
                m.embed(|e| {
                    e.title("运行状态")
                        .description([history, "图片生成完成！".to_string()].join("\n"))
                        .color(0x9b59b6)
                })
            })
            .await?;
    }
    // 删除初始消息并发送最终图片回复
    initial_msg.delete(ctx.clone()).await?;
    ctx.send(|reply| reply.attachment(serenity::AttachmentType::Path(&image_path)))
        .await?;
    Ok(())
}

/// 向AI提问并获取图片形式的回答
#[poise::command(slash_command, rename = "答疑bot")]
pub async fn qa_bot(
    ctx: Context<'_>,
    #[description = "你想问AI的问题"] 问题: String,
    #[description = "图片链接，可选"] 图片url1: Option<String>,
    #[description = "第二张图片链接，可选"] 图片url2: Option<String>,
    #[description = "第三张图片链接，可选"] 图片url3: Option<String>,
) -> Result<()> {
    ctx.defer().await?;
    let api_image_urls: Vec<String> = [图片url1, 图片url2, 图片url3]
        .iter()
        .filter_map(|opt| opt.clone())
        .collect();
    run_qa_flow(ctx, 问题, api_image_urls).await?;
    Ok(())
}

/// 查看历史会话列表
#[poise::command(slash_command, rename = "历史会话")]
pub async fn history_sessions(ctx: Context<'_>) -> Result<()> {
    // 延迟响应，避免Discord交互超时
    ctx.defer().await?;

    // 获取用户ID
    let user_id = ctx.author().id.to_string();

    info!("用户 {}({}) 请求查看历史会话", ctx.author().name, user_id);

    // 获取会话列表
    let sessions = ctx
        .data()
        .api_client
        .session_manager
        .get_user_sessions(&user_id);

    if sessions.is_empty() {
        ctx.say("📭 你还没有历史会话记录。").await?;
        return Ok(());
    }

    // 构建会话列表消息
    let mut message = String::with_capacity(1024);
    writeln!(message, "📚 **你的历史会话列表**\n").unwrap();

    for (i, session) in sessions.iter().take(10).enumerate() {
        writeln!(message, "{}", format_session_info(i, session)).unwrap();
    }

    if sessions.len() > 10 {
        writeln!(message, "... 还有 {} 个会话未显示", sessions.len() - 10).unwrap();
    }

    ctx.say(message).await?;

    Ok(())
}

/// 获取机器人使用指南
#[poise::command(slash_command, rename = "帮助")]
pub async fn help_command(ctx: Context<'_>) -> Result<()> {
    // 延迟响应，避免Discord交互超时
    ctx.defer().await?;
    info!("用户 {}({}) 请求帮助", ctx.author().name, ctx.author().id);

    let help_text = r#"# 🤖 Discord AI助手使用指南

## 基本命令

**/答疑bot [问题] [图片url1] [图片url2] [图片url3]** - 向AI提问并获取图片形式的回答
- `问题`: 你想问AI的问题
- `图片url1`: (可选) 第一张图片链接，用于视觉分析
- `图片url2`: (可选) 第二张图片链接，用于视觉分析
- `图片url3`: (可选) 第三张图片链接，用于视觉分析

**/历史会话** - 查看你的历史会话列表

**/帮助** - 获取机器人使用指南

**/存储统计** - 查看会话存储状态和统计信息

## 使用提示

1. 提问时尽量描述清晰，以获得更准确的回答
2. 支持任何有效的图片URL地址
3. 可以同时上传多张图片（最多3张）进行分析
4. 历史会话默认保存，但图片会在2天后自动清理
5. 每个用户的会话互相隔离，其他人无法看到你的会话内容

如有问题，请联系管理员。"#;

    ctx.say(help_text).await?;

    Ok(())
}

/// 查看会话存储状态和统计信息
#[poise::command(slash_command, rename = "存储统计")]
pub async fn storage_stats(
    ctx: Context<'_>,
    #[description = "是否显示详细的统计信息"] _详细信息: Option<bool>,
) -> Result<()> {
    // 延迟响应，避免Discord交互超时
    ctx.defer().await?;
    // 参数 _详细信息 暂未使用
    let session_manager = &ctx.data().api_client.session_manager;
    let user_id = ctx.author().id.to_string();
    // 获取用户会话列表
    let sessions = session_manager.get_user_sessions(&user_id);
    let total_sessions = sessions.len();
    // 统计已清理会话数与总图片大小（字节）
    let session_dirs: Vec<std::path::PathBuf> = sessions
        .iter()
        .map(|s| session_manager.get_session_dir(&s.id))
        .collect();
    let (cleaned_count, total_size) = tokio::task::spawn_blocking(move || {
        let mut cleaned = 0;
        let mut size = 0u64;
        for dir in session_dirs {
            if dir.join(".cleaned").exists() {
                cleaned += 1;
            }
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.filter_map(Result::ok) {
                    let path = entry.path();
                    if let Some(ext) = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|s| s.to_lowercase())
                    {
                        if ext == "png" || ext == "jpg" || ext == "jpeg" {
                            if let Ok(meta) = std::fs::metadata(&path) {
                                size += meta.len();
                            }
                        }
                    }
                }
            }
        }
        (cleaned, size)
    })
    .await
    .unwrap_or((0, 0));
    let total_images: u32 = sessions.iter().map(|s| s.images).sum();
    // 构造嵌入式消息
    ctx.send(|r| {
        r.embed(|e| {
            e.title("📊 存储统计")
                .color(0x3498db)
                .field("总会话数", total_sessions.to_string(), true)
                .field("已清理会话", cleaned_count.to_string(), true)
                .field("剩余图片数", total_images.to_string(), true)
                .field(
                    "总图片大小",
                    format!("{:.2} KB", total_size as f64 / 1024.0),
                    true,
                )
        })
    })
    .await?;
    Ok(())
}

// 格式化时间辅助函数
fn format_time(dt: DateTime<Utc>) -> String {
    dt.format("%Y-%m-%d %H:%M:%S").to_string()
}

// 获取简短会话ID
fn short_session_id(session_id: &str) -> &str {
    if session_id.len() > 8 {
        &session_id[..8]
    } else {
        session_id
    }
}

// 格式化会话信息
fn format_session_info(index: usize, session: &crate::session::SessionInfo) -> String {
    format!(
        "**{}. 会话 `{}`**\n   问题: {}\n   时间: {}\n   图片数: {}\n",
        index + 1,
        short_session_id(&session.id),
        session.input_preview,
        format_time(session.last_modified),
        session.images
    )
}

// 消息上下文菜单命令：右键→Apps→答疑回复
#[poise::command(context_menu_command = "回复答疑")]
pub async fn qa_context_reply(ctx: Context<'_>, message: serenity::Message) -> Result<()> {
    ctx.defer().await?;
    let question = format!(
        "需要答疑的用户{} 发送了以下消息：\n{}\n",
        message.author.name, message.content
    );
    let image_urls: Vec<String> = message
        .attachments
        .iter()
        .take(9)
        .map(|att| att.url.clone())
        .collect();
    run_qa_flow(ctx, question, image_urls).await?;
    Ok(())
}
