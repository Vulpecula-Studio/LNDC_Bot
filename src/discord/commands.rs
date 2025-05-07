use anyhow::Result;
use chrono::{DateTime, Utc};
use poise::serenity_prelude as serenity;
use std::fmt::Write;
use std::fs;
use tracing::info;
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

/// 向AI提问并获取图片形式的回答
#[poise::command(slash_command, prefix_command, rename = "答疑bot")]
pub async fn qa_bot(
    ctx: Context<'_>,
    #[description = "你想问AI的问题"] 问题: String,
    #[description = "图片链接，可选"] 图片url1: Option<String>,
    #[description = "第二张图片链接，可选"] 图片url2: Option<String>,
    #[description = "第三张图片链接，可选"] 图片url3: Option<String>,
) -> Result<()> {
    // 延迟响应，避免Discord交互超时
    ctx.defer().await?;
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

    // 获取用户ID
    let user_id = ctx.author().id.to_string();

    // 记录命令使用
    info!(
        "用户 {}({}) 使用了/答疑bot命令，问题: {}{}",
        ctx.author().name,
        user_id,
        truncate(&问题, 30),
        if 问题.chars().count() > 30 {
            "..."
        } else {
            ""
        }
    );

    // 收集所有有效的图片URL
    let api_image_urls: Vec<String> = [图片url1, 图片url2, 图片url3]
        .iter()
        .filter_map(|opt| opt.clone())
        .inspect(|url| info!("检测到图片URL: {}", url))
        .collect();

    if !api_image_urls.is_empty() {
        info!("共收集到{}张图片", api_image_urls.len());
    }

    // 调用FastGPT获取对话响应，仅使用 messages，开启 stream 和 detail
    let api_client = &ctx.data().api_client;
    let messages = vec![FastGPTMessage {
        role: "user".into(),
        content: json!([
            {"type": "text", "text": 问题}
        ]),
    }];
    let chat_resp = api_client
        .get_chat_response(
            None, // 不传 chat_id
            None, // 不传 response_chat_item_id
            messages, true, // stream 模式
            true, // detail 模式
            None, // 不传变量
        )
        .await?;
    // 动态更新运行状态，根据流式事件中的 flowNodeStatus
    let mut status_lines: Vec<String> = Vec::new();
    for (evt, data) in &chat_resp.events {
        if evt == "flowNodeStatus" {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(data) {
                if val.get("status").and_then(|s| s.as_str()) == Some("running") {
                    if let Some(name) = val.get("name").and_then(|n| n.as_str()) {
                        status_lines.push(name.to_string());
                        initial_msg
                            .edit(ctx, |m| {
                                m.embed(|e| {
                                    e.title("运行状态")
                                        .description(status_lines.join("\n"))
                                        .color(0x3498db)
                                })
                            })
                            .await?;
                    }
                }
            }
        }
    }
    // 最后添加完整响应状态
    status_lines.push("接收到fastgpt完整响应！".to_string());
    initial_msg
        .edit(ctx, |m| {
            m.embed(|e| {
                e.title("运行状态")
                    .description(status_lines.join("\n"))
                    .color(0x2ecc71)
            })
        })
        .await?;
    // 保存响应markdown并生成图片
    api_client
        .session_manager
        .save_user_input(&user_id, &问题)
        .await?;
    api_client
        .session_manager
        .save_response_markdown(&user_id, &chat_resp.content)
        .await?;
    // 更新状态：图片生成中
    initial_msg
        .edit(ctx, |m| {
            m.embed(|e| {
                e.title("运行状态")
                    .description([status_lines.join("\n"), "图片生成中...".to_string()].join("\n"))
                    .color(0xf1c40f)
            })
        })
        .await?;
    // 生成图片并发送
    let image_resp = api_client.image_generator.create_image_from_markdown(
        &chat_resp.content,
        &api_client
            .config
            .image_output_dir
            .join("temp")
            .join(format!("response_{}.png", Uuid::new_v4())),
    )?;
    // 更新状态：图片生成完成
    initial_msg
        .edit(ctx, |m| {
            m.embed(|e| {
                e.title("运行状态")
                    .description([status_lines.join("\n"), "图片生成完成！".to_string()].join("\n"))
                    .color(0x9b59b6)
            })
        })
        .await?;
    // 删除临时文件并发送最终图片
    let _ = fs::remove_file(&image_resp);
    initial_msg.delete(ctx).await?;
    ctx.send(|reply| reply.attachment(serenity::AttachmentType::Path(&image_resp)))
        .await?;

    Ok(())
}

/// 查看历史会话列表
#[poise::command(slash_command, prefix_command, rename = "历史会话")]
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
#[poise::command(slash_command, prefix_command, rename = "帮助")]
pub async fn help_command(ctx: Context<'_>) -> Result<()> {
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
#[poise::command(slash_command, prefix_command, rename = "存储统计")]
pub async fn storage_stats(
    ctx: Context<'_>,
    #[description = "是否显示详细的统计信息"] 详细信息: Option<bool>,
) -> Result<()> {
    // 延迟响应，避免Discord交互超时
    ctx.defer().await?;

    let detailed = 详细信息.unwrap_or(false);

    info!(
        "用户 {}({}) 请求存储统计，详细信息: {}",
        ctx.author().name,
        ctx.author().id,
        detailed
    );

    // 用户ID
    let user_id = ctx.author().id.to_string();

    // 获取会话列表
    let sessions = ctx
        .data()
        .api_client
        .session_manager
        .get_user_sessions(&user_id);

    // 计算存储统计
    let total_sessions = sessions.len();
    let total_images: u32 = sessions.iter().map(|s| s.images).sum();

    // 生成统计信息
    let mut message = String::with_capacity(1024);

    writeln!(message, "📊 **存储统计**\n").unwrap();
    writeln!(message, "总会话数: **{}**", total_sessions).unwrap();
    writeln!(message, "总图片数: **{}**", total_images).unwrap();

    if detailed && !sessions.is_empty() {
        writeln!(message, "\n**详细会话信息:**\n").unwrap();

        for (i, session) in sessions.iter().enumerate() {
            if i >= 15 {
                writeln!(message, "... 还有 {} 个会话未显示", sessions.len() - 15).unwrap();
                break;
            }

            writeln!(
                message,
                "{}. 会话 `{}` - {} 个图片 - 最后更新: {}",
                i + 1,
                short_session_id(&session.id),
                session.images,
                format_time(session.last_modified)
            )
            .unwrap();
        }
    }

    writeln!(message, "\n⚠️ 注意: 图片会在2天后自动清理，文本内容会保留").unwrap();

    ctx.say(message).await?;

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
