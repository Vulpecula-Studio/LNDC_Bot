use anyhow::Result;
use poise::serenity_prelude as serenity;
use serenity::builder::CreateEmbed;
use tracing::{info, error};
use chrono::{DateTime, Utc};
use std::fmt::Write;

use super::Context;

/// 向AI提问并获取图片形式的回答
#[poise::command(slash_command, prefix_command)]
pub async fn qa_bot(
    ctx: Context<'_>,
    #[description = "你想问AI的问题"] 问题: String,
    #[description = "Discord图片链接，可选"] 图片url: Option<String>,
) -> Result<()> {
    // 延迟响应，避免Discord交互超时
    ctx.defer().await?;
    
    // 获取用户ID
    let user_id = ctx.author().id.to_string();
    
    // 记录命令使用
    info!(
        "用户 {} (ID: {}) 使用了/答疑bot命令，问题: {}...",
        ctx.author().name,
        user_id,
        if 问题.len() > 30 { &问题[..30] } else { &问题 }
    );
    
    // 创建图片URL
    let api_image_url = if let Some(url) = 图片url {
        // 验证图片URL是否来自Discord
        if url.contains("discordapp.net") || url.contains("cdn.discord.com") {
            info!("检测到Discord图片URL: {}", url);
            Some(url)
        } else {
            ctx.say("❌ 只支持Discord图片链接。").await?;
            return Ok(());
        }
    } else {
        None
    };
    
    // 调用API获取图片回答
    let api_client = &ctx.data().api_client;
    
    match api_client.get_response_as_image(
        &问题,
        &user_id,
        api_image_url.as_deref(),
    ).await {
        Ok(response) => {
            // 构建回复
            let image_path = response.image_path;
            let session_id = response.session_id;
            
            // 检查文件是否存在
            if !image_path.exists() {
                ctx.say("❌ 生成图片失败：文件不存在。").await?;
                return Ok(());
            }
            
            // 创建嵌入消息
            let embed = CreateEmbed::default()
                .title("🤖 AI回答").to_owned()
                .description(format!("会话ID: `{}`", &session_id[..8])).to_owned()
                .color(0x3498db).to_owned()
                .footer(|f| {
                    f.text(format!("提问者: {}", ctx.author().name))
                }).to_owned()
                .timestamp(Utc::now()).to_owned();
            
            // 发送嵌入消息和图片
            ctx.send(|reply| {
                reply
                    .attachment(serenity::AttachmentType::Path(&image_path))
                    .embed(|e| {
                        *e = embed.clone();
                        e
                    })
            }).await?;
            
            info!("成功回答问题，会话ID: {}", session_id);
        },
        Err(e) => {
            error!("处理问题时出错: {}", e);
            ctx.say(format!("❌ 请求处理失败: {}", e)).await?;
        }
    }
    
    Ok(())
}

/// 查看历史会话列表
#[poise::command(slash_command, prefix_command)]
pub async fn history_sessions(
    ctx: Context<'_>,
) -> Result<()> {
    // 延迟响应，避免Discord交互超时
    ctx.defer().await?;
    
    // 获取用户ID
    let user_id = ctx.author().id.to_string();
    
    info!("用户 {} (ID: {}) 请求查看历史会话", ctx.author().name, user_id);
    
    // 获取会话列表
    let sessions = ctx.data().api_client.session_manager.get_user_sessions(&user_id);
    
    if sessions.is_empty() {
        ctx.say("📭 你还没有历史会话记录。").await?;
        return Ok(());
    }
    
    // 构建会话列表消息
    let mut message = String::new();
    writeln!(message, "📚 **你的历史会话列表**\n").unwrap();
    
    for (i, session) in sessions.iter().take(10).enumerate() {
        let last_modified = format_time(session.last_modified);
        writeln!(
            message,
            "**{}. 会话 `{}`**\n   问题: {}\n   时间: {}\n   图片数: {}\n",
            i + 1,
            &session.id[..8],
            session.input_preview,
            last_modified,
            session.images
        ).unwrap();
    }
    
    if sessions.len() > 10 {
        writeln!(message, "... 还有 {} 个会话未显示", sessions.len() - 10).unwrap();
    }
    
    ctx.say(message).await?;
    
    Ok(())
}

/// 获取机器人使用指南
#[poise::command(slash_command, prefix_command)]
pub async fn help_command(
    ctx: Context<'_>,
) -> Result<()> {
    info!("用户 {} (ID: {}) 请求帮助", ctx.author().name, ctx.author().id);
    
    let help_text = r#"# 🤖 Discord AI助手使用指南

## 基本命令

**/答疑bot [问题] [图片url]** - 向AI提问并获取图片形式的回答
- `问题`: 你想问AI的问题
- `图片url`: (可选) Discord图片链接，用于视觉分析

**/历史会话** - 查看你的历史会话列表

**/帮助** - 获取机器人使用指南

**/存储统计** - 查看会话存储状态和统计信息

## 使用提示

1. 提问时尽量描述清晰，以获得更准确的回答
2. 图片链接必须是Discord上传的图片链接
3. 历史会话默认保存，但图片会在2天后自动清理
4. 每个用户的会话互相隔离，其他人无法看到你的会话内容

如有问题，请联系管理员。"#;

    ctx.say(help_text).await?;
    
    Ok(())
}

/// 查看会话存储状态和统计信息
#[poise::command(slash_command, prefix_command)]
pub async fn storage_stats(
    ctx: Context<'_>,
    #[description = "是否显示详细的统计信息"] 详细信息: Option<bool>,
) -> Result<()> {
    // 延迟响应，避免Discord交互超时
    ctx.defer().await?;
    
    let detailed = 详细信息.unwrap_or(false);
    
    info!("用户 {} (ID: {}) 请求存储统计，详细信息: {}", 
        ctx.author().name, ctx.author().id, detailed);
    
    // 用户ID
    let user_id = ctx.author().id.to_string();
    
    // 获取会话列表
    let sessions = ctx.data().api_client.session_manager.get_user_sessions(&user_id);
    
    // 计算存储统计
    let total_sessions = sessions.len();
    let total_images: u32 = sessions.iter().map(|s| s.images).sum();
    
    // 生成统计信息
    let mut message = String::new();
    
    writeln!(message, "📊 **存储统计**\n").unwrap();
    writeln!(message, "总会话数: **{}**", total_sessions).unwrap();
    writeln!(message, "总图片数: **{}**", total_images).unwrap();
    
    if detailed && !sessions.is_empty() {
        writeln!(message, "\n**详细会话信息:**\n").unwrap();
        
        for (i, session) in sessions.iter().enumerate() {
            let last_modified = format_time(session.last_modified);
            writeln!(
                message,
                "{}. 会话 `{}` - {} 个图片 - 最后更新: {}",
                i + 1,
                &session.id[..8],
                session.images,
                last_modified
            ).unwrap();
            
            if i >= 14 && sessions.len() > 15 {
                writeln!(message, "... 还有 {} 个会话未显示", sessions.len() - 15).unwrap();
                break;
            }
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