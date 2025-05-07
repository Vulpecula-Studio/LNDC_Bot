use anyhow::anyhow;
use anyhow::Result;
use chrono::{DateTime, Utc};
use poise::serenity_prelude as serenity;
use std::fmt::Write;
use std::sync::{Arc, Mutex};
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
    // 收集所有有效的图片URL
    let api_image_urls: Vec<String> = [图片url1, 图片url2, 图片url3]
        .iter()
        .filter_map(|opt| opt.clone())
        .inspect(|url| info!("检测到图片URL: {}", url))
        .collect();
    if !api_image_urls.is_empty() {
        info!("共收集到{}张图片", api_image_urls.len());
    }
    let messages = vec![FastGPTMessage {
        role: "user".into(),
        content: json!([
            {"type": "text", "text": 问题}
        ]),
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

    // 获取用户ID
    let user_id = ctx.author().id.to_string();
    // 获取 API 客户端
    let api_client = &ctx.data().api_client;
    // 创建新的会话并获取 session_id
    let session_id = api_client.session_manager.create_session(&user_id)?;

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

    // 调用FastGPT获取对话响应，仅使用消息内容
    let status_lines: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let chat_resp = api_client
        .get_chat_response(
            None, // 不传 chat_id
            None, // 不传 response_chat_item_id
            messages,
            true, // stream 模式
            true, // detail 模式
            None, // 不传变量
            {
                // 为回调克隆共享状态、上下文和初始消息
                let status_lines = Arc::clone(&status_lines);
                let ctx = ctx.clone();
                let initial_msg = initial_msg.clone();
                move |evt, data| {
                    let status_lines = Arc::clone(&status_lines);
                    let ctx = ctx.clone();
                    let evt = evt.to_string();
                    let data = data.to_string();
                    // 克隆一次 message handle 供异步块使用，避免捕获 initial_msg 并移动
                    let msg = initial_msg.clone();
                    async move {
                        if evt == "flowNodeStatus" {
                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&data) {
                                if val.get("status").and_then(|s| s.as_str()) == Some("running") {
                                    if let Some(name) = val.get("name").and_then(|n| n.as_str()) {
                                        // 更新历史：将前一个节点标记为完成，并为当前节点添加运行状态
                                        let description = {
                                            let mut lines = status_lines.lock().unwrap();
                                            // 如果已有上一行且是运行状态，则标记为完成
                                            if !lines.is_empty() {
                                                let last_index = lines.len() - 1;
                                                if lines[last_index].ends_with(" 🔄") {
                                                    let trimmed =
                                                        lines[last_index].trim_end_matches(" 🔄");
                                                    lines[last_index] = format!("{} ✅", trimmed);
                                                }
                                            }
                                            // 添加当前运行状态
                                            lines.push(format!("{} 🔄", name));
                                            lines.join("\n")
                                        };
                                        // 实时编辑嵌入消息，使用 msg 句柄
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
            },
        )
        .await?;
    // 最后添加完整响应状态
    {
        let history = status_lines.lock().unwrap().join("\n");
        initial_msg
            .edit(ctx, |m| {
                m.embed(|e| {
                    e.title("运行状态")
                        .description([history, "✅ 接收到fastgpt完整响应！".to_string()].join("\n"))
                        .color(0x2ecc71)
                })
            })
            .await?;
    }
    // 保存用户输入和响应markdown到会话目录
    api_client
        .session_manager
        .save_user_input(&session_id, &问题)
        .await?;
    api_client
        .session_manager
        .save_response_markdown(&session_id, &chat_resp.content)
        .await?;
    // 保存用户提供的图片链接到会话目录
    let image_urls = api_image_urls.clone();
    api_client
        .session_manager
        .save_user_images(&session_id, &image_urls)
        .await?;
    // 更新状态：图片生成中
    {
        let history = status_lines.lock().unwrap().join("\n");
        initial_msg
            .edit(ctx, |m| {
                m.embed(|e| {
                    e.title("运行状态")
                        .description([history, "图片生成中...".to_string()].join("\n"))
                        .color(0xf1c40f)
                })
            })
            .await?;
    }
    // 在会话目录生成图片并发送
    let session_dir = api_client.session_manager.get_session_dir(&session_id);
    let image_path = session_dir.join(format!("response_{}.png", Uuid::new_v4()));
    api_client
        .image_generator
        .create_image_from_markdown(&chat_resp.content, &image_path)?;
    // 更新状态：图片生成完成
    {
        let history = status_lines.lock().unwrap().join("\n");
        initial_msg
            .edit(ctx, |m| {
                m.embed(|e| {
                    e.title("运行状态")
                        .description([history, "图片生成完成！".to_string()].join("\n"))
                        .color(0x9b59b6)
                })
            })
            .await?;
    }
    // 删除初始确认消息，发送最终图片回复
    initial_msg.delete(ctx).await?;
    ctx.send(|reply| reply.attachment(serenity::AttachmentType::Path(&image_path)))
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

// 回复模式命令
#[poise::command(prefix_command, rename = "答疑回复")]
pub async fn qa_reply(
    ctx: Context<'_>,
    #[description = "可选 主人指令"] 主人指令: Option<String>,
) -> Result<()> {
    ctx.defer().await?;
    // 仅 prefix 模式下可用，获取 PrefixContext 并取出消息
    let prefix_ctx = match &ctx {
        Context::Prefix(prefix_ctx) => prefix_ctx,
        _ => return Err(anyhow!("请在回复消息时使用此命令")),
    };
    let msg = &prefix_ctx.msg;
    let replied = msg
        .referenced_message
        .as_ref()
        .ok_or_else(|| anyhow!("请回复一条消息来使用此命令"))?;
    // 构造提问文本
    let mut question_text = format!(
        "需要答疑的用户{} 发送了以下消息：\n{}\n",
        replied.author.name, replied.content
    );
    if let Some(owner_cmd) = 主人指令 {
        write!(
            question_text,
            "{{{{{}}}用户在主人的命令这个元素下的参数}}\n",
            owner_cmd
        )?;
    }
    // 构造消息内容
    let mut content_array = Vec::new();
    content_array.push(json!({"type": "text", "text": question_text.clone()}));
    for att in &replied.attachments {
        content_array.push(json!({"type": "image_url", "image_url": {"url": att.url.clone()}}));
    }
    let messages = if replied.attachments.is_empty() {
        vec![FastGPTMessage {
            role: "user".into(),
            content: json!(question_text.clone()),
        }]
    } else {
        vec![FastGPTMessage {
            role: "user".into(),
            content: json!(content_array),
        }]
    };
    // 发送初始确认
    let initial_msg = ctx
        .send(|m| {
            m.embed(|e| {
                e.title("✅ 请求已接收")
                    .description("正在等待fastgpt响应...")
                    .color(0x3498db)
            })
        })
        .await?;
    let api_client = &ctx.data().api_client;
    let chat_resp = api_client
        .get_chat_response(None, None, messages, false, false, None, |_, _| async {
            Ok(())
        })
        .await?;
    // 保存和生成
    let user_id = ctx.author().id.to_string();
    let session_id = api_client.session_manager.create_session(&user_id)?;
    api_client
        .session_manager
        .save_user_input(&session_id, &question_text)
        .await?;
    api_client
        .session_manager
        .save_response_markdown(&session_id, &chat_resp.content)
        .await?;
    let session_dir = api_client.session_manager.get_session_dir(&session_id);
    let image_path = session_dir.join(format!("response_{}.png", Uuid::new_v4()));
    api_client
        .image_generator
        .create_image_from_markdown(&chat_resp.content, &image_path)?;
    initial_msg.delete(ctx).await?;
    ctx.send(|reply| {
        reply.content(format!("<@{}>", replied.author.id));
        reply.attachment(serenity::AttachmentType::Path(&image_path))
    })
    .await?;
    Ok(())
}

// 斜线指令：答疑回复（选择用户，获取其最近消息）
#[poise::command(slash_command, rename = "答疑回复")]
pub async fn qa_reply_slash(
    ctx: Context<'_>,
    #[description = "答疑对象"] target: serenity::User,
    #[description = "可选 主人指令"] owner_cmd: Option<String>,
) -> Result<()> {
    ctx.defer().await?;
    // 拉取本频道最近消息，寻找目标用户最后一条消息
    let http = ctx.serenity_context().http.clone();
    let channel_id = ctx.channel_id();
    let messages_history = channel_id
        .messages(&http, |retriever| retriever.limit(50))
        .await?;
    let last = messages_history
        .iter()
        .find(|m| m.author.id == target.id)
        .ok_or_else(|| anyhow!("未找到该用户的最近消息"))?;
    // 构造提问文本
    let mut question_text = format!(
        "需要答疑的用户{} 发送了以下消息：\n{}\n",
        target.name, last.content
    );
    if let Some(cmd) = owner_cmd {
        write!(
            question_text,
            "{{{{{}}}用户在主人的命令这个元素下的参数}}\n",
            cmd
        )?;
    }
    // 构造 FastGPT 消息体
    let mut content_array = Vec::new();
    content_array.push(json!({"type": "text", "text": question_text.clone()}));
    for att in &last.attachments {
        content_array.push(json!({
            "type": "image_url",
            "image_url": {"url": att.url.clone()}
        }));
    }
    let messages_req = if last.attachments.is_empty() {
        vec![FastGPTMessage {
            role: "user".into(),
            content: json!(question_text.clone()),
        }]
    } else {
        vec![FastGPTMessage {
            role: "user".into(),
            content: json!(content_array),
        }]
    };
    // 调用 FastGPT
    let chat_resp = ctx
        .data()
        .api_client
        .get_chat_response(None, None, messages_req, false, false, None, |_, _| async {
            Ok(())
        })
        .await?;
    // 保存会话并生成图片
    let user_id = ctx.author().id.to_string();
    let session_id = ctx
        .data()
        .api_client
        .session_manager
        .create_session(&user_id)?;
    ctx.data()
        .api_client
        .session_manager
        .save_user_input(&session_id, &question_text)
        .await?;
    ctx.data()
        .api_client
        .session_manager
        .save_response_markdown(&session_id, &chat_resp.content)
        .await?;
    let session_dir = ctx
        .data()
        .api_client
        .session_manager
        .get_session_dir(&session_id);
    let image_path = session_dir.join(format!("response_{}.png", Uuid::new_v4()));
    ctx.data()
        .api_client
        .image_generator
        .create_image_from_markdown(&chat_resp.content, &image_path)?;
    // 发送最终回复，@目标用户
    ctx.send(|reply| {
        reply.content(format!("<@{}>", target.id));
        reply.attachment(serenity::AttachmentType::Path(&image_path))
    })
    .await?;
    Ok(())
}

// 消息上下文菜单命令：右键→Apps→答疑回复
#[poise::command(context_menu_command = "message", rename = "答疑回复")]
pub async fn qa_context_reply(ctx: Context<'_>, message: serenity::Message) -> Result<()> {
    // 延迟响应，避免Discord交互超时
    ctx.defer().await?;
    // 构造 FastGPT 消息体
    let mut content_array = Vec::new();
    let question_text = format!(
        "需要答疑的用户{} 发送了以下消息：\n{}\n",
        message.author.name, message.content
    );
    content_array.push(json!({"type": "text", "text": question_text.clone()}));
    for att in &message.attachments {
        content_array.push(json!({"type": "image_url", "image_url": {"url": att.url.clone()}}));
    }
    // 构建 FastGPT 消息体，始终使用 JSON 数组格式，与 qa_bot 保持一致
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
    // 创建新的会话
    let session_id = api_client.session_manager.create_session(&user_id)?;
    info!(
        "用户 {}({}) 使用了消息上下文菜单命令，内容预览: {}",
        ctx.author().name,
        user_id,
        truncate(&question_text, 30)
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
                                            if lines[last_index].ends_with(" 🔄") {
                                                let trimmed =
                                                    lines[last_index].trim_end_matches(" 🔄");
                                                lines[last_index] = format!("{} ✅", trimmed);
                                            }
                                        }
                                        lines.push(format!("{} 🔄", name));
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
    // 最后添加完整响应状态
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
    // 保存用户输入和响应
    api_client
        .session_manager
        .save_user_input(&session_id, &question_text)
        .await?;
    api_client
        .session_manager
        .save_response_markdown(&session_id, &chat_resp.content)
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
            .edit(ctx, |m| {
                m.embed(|e| {
                    e.title("运行状态")
                        .description([history, "图片生成完成！".to_string()].join("\n"))
                        .color(0x9b59b6)
                })
            })
            .await?;
    }
    // 删除初始消息，发送最终图片回复并 @ 用户
    initial_msg.delete(ctx.clone()).await?;
    ctx.send(|reply| {
        reply.content(format!("<@{}>", message.author.id));
        reply.attachment(serenity::AttachmentType::Path(&image_path))
    })
    .await?;
    Ok(())
}
