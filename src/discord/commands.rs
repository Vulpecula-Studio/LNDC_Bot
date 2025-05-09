use anyhow::Result;
use chrono::{DateTime, Utc};
use poise::serenity_prelude as serenity;
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
    // 获取用户ID和 API 客户端
    let user_id = ctx.author().id.to_string();
    debug!(
        "run_qa_flow 调用, 用户ID: {}, 问题: {}, 图片数量: {}",
        user_id,
        question,
        image_urls.len()
    );
    let api_client = &ctx.data().api_client;
    // 构造 FastGPT 消息体
    let mut content_array = Vec::new();
    content_array.push(json!({"type":"text","text": question.clone()}));
    for url in &image_urls {
        content_array.push(json!({"type":"image_url","image_url":{"url": url}}));
    }
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
    // 创建新的会话并记录
    let session_id = api_client.session_manager.create_session(&user_id)?;
    // 信息级别：记录简要提问
    info!(
        "用户{} 提问: {}",
        ctx.author().name,
        truncate(&question, 30)
    );
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
    // 如果重试后仍为空，则取消生成图片并提示用户
    if chat_resp.content.trim().is_empty() {
        debug!("重复获取后回复仍为空，取消后续操作");
        initial_msg
            .edit(ctx.clone(), |m| {
                m.embed(|e| {
                    e.title("错误")
                        .description("未收到有效回复，已取消图片生成。")
                        .color(0xe74c3c)
                })
            })
            .await?;
        return Ok(());
    }
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

    // 获取用户ID和会话列表
    let user_id = ctx.author().id.to_string();
    let sessions = ctx
        .data()
        .api_client
        .session_manager
        .get_user_sessions(&user_id);

    // 如果没有会话，直接提示
    if sessions.is_empty() {
        ctx.say("📭 你还没有历史会话记录。").await?;
        return Ok(());
    }

    // 分页参数
    let per_page = 10;
    let total = sessions.len();
    let total_pages = (total + per_page - 1) / per_page;
    let page = 0;
    let start = page * per_page;
    let end = ((page + 1) * per_page).min(total);
    let sessions_page = &sessions[start..end];

    // 发送嵌入式消息并添加翻页按钮
    ctx.send(|r| {
        r.embed(|e| {
            e.title("📚 你的历史会话列表")
                .color(0x3498db)
                .description(
                    sessions_page
                        .iter()
                        .enumerate()
                        .map(|(i, session)| format_session_info(start + i, session))
                        .collect::<Vec<_>>()
                        .join("\n"),
                )
                .footer(|f| f.text(format!("第 {}/{} 页", page + 1, total_pages)))
        })
        .components(|c| {
            c.create_action_row(|row| {
                row.create_button(|b| {
                    b.custom_id(format!("history_{}_{}_prev", user_id, page))
                        .label("上一页")
                        .style(serenity::ButtonStyle::Secondary)
                        .disabled(true)
                })
                .create_button(|b| {
                    b.custom_id(format!("history_{}_{}_next", user_id, page))
                        .label("下一页")
                        .style(serenity::ButtonStyle::Secondary)
                        .disabled(total_pages <= 1)
                })
            })
        })
    })
    .await?;

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
    #[description = "是否显示详细的统计信息"] 详细信息: Option<bool>,
) -> Result<()> {
    // 延迟响应，避免Discord交互超时
    ctx.defer().await?;
    // 判断是否展示详细信息
    let detailed = 详细信息.unwrap_or(false);
    let session_manager = &ctx.data().api_client.session_manager;
    let user_id = ctx.author().id.to_string();
    let sessions = session_manager.get_user_sessions(&user_id);
    let total_sessions = sessions.len();
    // 准备各会话目录
    let session_dirs: Vec<std::path::PathBuf> = sessions
        .iter()
        .map(|s| session_manager.get_session_dir(&s.id))
        .collect();
    if !detailed {
        // 简略统计
        let (cleaned_count, total_size) = tokio::task::spawn_blocking(move || {
            let mut cleaned = 0;
            let mut size = 0u64;
            for dir in &session_dirs {
                if dir.join(".cleaned").exists() {
                    cleaned += 1;
                }
                if let Ok(entries) = std::fs::read_dir(dir) {
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
    } else {
        // 详细统计：包括每个会话大小与清理状态，支持分页
        let sessions_clone = sessions.clone();
        let dirs_clone = session_dirs.clone();
        // 构建每会话详情文本
        let mut per_details = Vec::new();
        let mut cleaned_total = 0usize;
        let mut size_total = 0u64;
        for (session, dir) in sessions_clone.iter().zip(dirs_clone.iter()) {
            let cleaned_flag = dir.join(".cleaned").exists();
            if cleaned_flag {
                cleaned_total += 1;
            }
            let mut ss = 0u64;
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.filter_map(Result::ok) {
                    let path = entry.path();
                    if let Some(ext) = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|s| s.to_lowercase())
                    {
                        if ext == "png" || ext == "jpg" || ext == "jpeg" {
                            if let Ok(meta) = std::fs::metadata(&path) {
                                ss += meta.len();
                            }
                        }
                    }
                }
            }
            size_total += ss;
            let short = if session.id.len() > 8 {
                &session.id[..8]
            } else {
                &session.id
            };
            let time = format_time(session.last_modified);
            per_details.push(format!(
                "`{}` | 时间: {} | 图片: {} | 大小: {:.2}KB | 已清理: {}",
                short,
                time,
                session.images,
                ss as f64 / 1024.0,
                if cleaned_flag { "✅" } else { "❌" }
            ));
        }
        let total_images: u32 = sessions_clone.iter().map(|s| s.images).sum();
        // 分页显示，每页10条
        let per_page = 10;
        let detail_count = per_details.len();
        let total_pages = (detail_count + per_page - 1) / per_page;
        let page = 0;
        let start = page * per_page;
        let end = ((page + 1) * per_page).min(detail_count);
        let page_details = &per_details[start..end];
        let mut detail_text = page_details.join("\n");
        // 裁剪确保长度不超限
        if detail_text.chars().count() > 1024 {
            detail_text = detail_text.chars().take(1021).collect::<String>() + "...";
        }
        ctx.send(|r| {
            r.embed(|e| {
                e.title("📊 存储统计（详细）")
                    .color(0x3498db)
                    .field("总会话数", total_sessions.to_string(), true)
                    .field("已清理会话", cleaned_total.to_string(), true)
                    .field("剩余图片数", total_images.to_string(), true)
                    .field(
                        "总图片大小",
                        format!("{:.2} KB", size_total as f64 / 1024.0),
                        true,
                    )
                    .footer(|f| f.text(format!("第 {}/{} 页", page + 1, total_pages)))
                    .field("会话详情", detail_text, false)
            })
            .components(|c| {
                c.create_action_row(|row| {
                    row.create_button(|b| {
                        b.custom_id(format!("stats_{}_{}_prev", user_id, page))
                            .label("上一页")
                            .style(serenity::ButtonStyle::Secondary)
                            .disabled(true)
                    })
                    .create_button(|b| {
                        b.custom_id(format!("stats_{}_{}_next", user_id, page))
                            .label("下一页")
                            .style(serenity::ButtonStyle::Secondary)
                            .disabled(total_pages <= 1)
                    })
                })
            })
        })
        .await?;
    }
    Ok(())
}

// 格式化时间辅助函数
pub(super) fn format_time(dt: DateTime<Utc>) -> String {
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
pub(super) fn format_session_info(index: usize, session: &crate::session::SessionInfo) -> String {
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
