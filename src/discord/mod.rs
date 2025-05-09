mod commands;

use anyhow::Result;
use poise::serenity_prelude as serenity;
use poise::FrameworkBuilder;
use std::sync::Arc;
use tokio::select;
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn};

use crate::api::APIClient;
use crate::config::Config;

use commands::*;

pub type Context<'a> = poise::Context<'a, Data, anyhow::Error>;

// 机器人数据结构
#[derive(Debug, Clone)]
pub struct Data {
    #[allow(dead_code)]
    pub config: Config,
    pub api_client: Arc<APIClient>,
}

// 启动Discord机器人
pub async fn start_bot(config: &Config) -> Result<()> {
    // 初始化API客户端
    let api_client = Arc::new(APIClient::new(config.clone())?);

    info!(
        "Discord Token 前10个字符: {}...",
        &config.discord_token[..10]
    );
    info!("正在初始化Discord机器人...");

    // 创建共享数据
    let data = Data {
        config: config.clone(),
        api_client: api_client.clone(),
    };

    // 创建框架
    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![
                qa_bot(),
                qa_context_reply(),
                history_sessions(),
                help_command(),
                storage_stats(),
            ],
            prefix_options: poise::PrefixFrameworkOptions {
                prefix: Some("!".into()),
                ..Default::default()
            },
            // 设置事件处理
            event_handler: |ctx, event, _framework, data| Box::pin(event_handler(ctx, event, data)),
            // 注册全局错误处理
            on_error: |error| Box::pin(on_error(error)),

            // 启用命令编辑跟踪
            command_check: Some(|ctx| {
                Box::pin(async move {
                    info!("接收到命令: {:?}", ctx.command().qualified_name);
                    Ok(true)
                })
            }),

            // 设置其他选项
            skip_checks_for_owners: true,

            ..Default::default()
        })
        .token(&config.discord_token)
        .intents(
            serenity::GatewayIntents::non_privileged() | serenity::GatewayIntents::MESSAGE_CONTENT,
        )
        .setup(|ctx, ready, framework| {
            Box::pin(async move {
                info!(
                    "机器人已登录: {}#{}",
                    ready.user.name, ready.user.discriminator
                );
                info!("机器人ID: {}", ready.user.id);
                info!("所在服务器数量: {}", ready.guilds.len());

                // 列出所有服务器
                for guild in &ready.guilds {
                    info!("  - 服务器 ID: {}", guild.id);
                }

                // 注册全局斜线命令 - 这很重要，确保全局命令被正确注册
                info!("正在注册全局斜线命令...");
                match poise::builtins::register_globally(ctx, &framework.options().commands).await {
                    Ok(_) => info!("已成功注册全局斜线命令"),
                    Err(e) => {
                        error!("注册全局斜线命令失败: {}", e);
                        error!("详细错误: {:?}", e);
                    }
                }

                Ok(data)
            })
        });

    info!("正在启动Discord机器人...");

    // 启动周期性清理任务和机器人
    start_with_periodic_cleanup(framework, api_client).await
}

// 并发运行机器人和清理任务
async fn start_with_periodic_cleanup(
    framework: FrameworkBuilder<Data, anyhow::Error>,
    api_client: Arc<APIClient>,
) -> Result<()> {
    // 创建一个关闭信号通道
    let (shutdown_send, mut shutdown_recv) = tokio::sync::oneshot::channel::<()>();
    let mut shutdown_send = Some(shutdown_send);

    // 机器人任务
    let bot_task = tokio::spawn(async move {
        info!("机器人框架开始运行");
        match framework.run().await {
            Ok(_) => info!("机器人正常关闭"),
            Err(e) => error!("机器人运行时错误: {}", e),
        }

        // 如果机器人关闭，发送关闭信号
        if let Some(sender) = shutdown_send.take() {
            let _ = sender.send(());
        }
    });

    // 清理任务 - 每6小时运行一次
    let cleanup_task = tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(6 * 60 * 60));
        loop {
            select! {
                _ = interval.tick() => {
                    info!("开始执行定期清理任务");
                    // 执行清理
                    api_client.session_manager.periodic_cleanup(2).await;
                },
                _ = &mut shutdown_recv => {
                    info!("接收到关闭信号，停止清理任务");
                    break;
                }
            }
        }
    });

    // 等待任务完成
    tokio::select! {
        _ = bot_task => {
            info!("机器人任务已结束");
            // 在这里不需要abort cleanup_task，因为它会收到关闭信号
        }
        _ = cleanup_task => {
            info!("清理任务已结束");
            // 这种情况不应该发生，因为清理任务应该一直运行
            error!("清理任务意外结束");
        }
    }

    Ok(())
}

// Discord事件处理
async fn event_handler(
    ctx: &serenity::Context,
    event: &poise::Event<'_>,
    _data: &Data,
) -> Result<(), anyhow::Error> {
    match event {
        poise::Event::Ready { data_about_bot } => {
            info!("机器人已登录: {}", data_about_bot.user.name);

            // 获取应用命令
            match ctx.http.get_global_application_commands().await {
                Ok(commands) => {
                    info!("已注册的全局应用命令数量: {}", commands.len());
                    for cmd in commands {
                        info!("全局命令: {} (ID: {})", cmd.name, cmd.id);
                    }
                }
                Err(e) => error!("获取全局应用命令失败: {}", e),
            }
        }
        poise::Event::InteractionCreate { interaction } => {
            info!(
                "收到交互: {:?}, 类型: {:?}",
                interaction.id(),
                interaction.kind()
            );

            if let Some(cmd) = interaction.as_application_command() {
                info!("收到应用命令: {} (ID: {})", cmd.data.name, interaction.id());
                debug!("命令数据: {:?}", cmd.data);
            } else if let Some(autocomplete) = interaction.as_autocomplete() {
                debug!("收到自动完成交互: {}", autocomplete.data.name);
            } else if let Some(msg_component) = interaction.as_message_component() {
                let cid = &msg_component.data.custom_id;
                // 处理历史会话分页交互，custom_id 格式: history_{user_id}_{page}_{action}
                if cid.starts_with("history_") {
                    let parts: Vec<&str> = cid.split('_').collect();
                    if parts.len() == 4 {
                        let target_user_id = parts[1];
                        let page: usize = parts[2].parse().unwrap_or(0);
                        let action = parts[3];
                        // 仅允许原用户操作
                        if msg_component.user.id.to_string() != target_user_id {
                            let _ = msg_component.create_interaction_response(&ctx.http, |response| {
                                response.kind(serenity::InteractionResponseType::ChannelMessageWithSource)
                                    .interaction_response_data(|m| {
                                        m.content("❌ 无权操作此分页").ephemeral(true)
                                    })
                            }).await;
                        } else {
                            // 计算新页面
                            let mut new_page = match action {
                                "prev" if page > 0 => page - 1,
                                "next" => page + 1,
                                _ => page,
                            };
                            let sessions = _data.api_client.session_manager.get_user_sessions(&target_user_id.to_string());
                            let per_page = 10;
                            let total = sessions.len();
                            let total_pages = (total + per_page - 1) / per_page;
                            if new_page >= total_pages {
                                new_page = total_pages.saturating_sub(1);
                            }
                            let start = new_page * per_page;
                            let end = ((new_page + 1) * per_page).min(total);
                            let sessions_page = &sessions[start..end];

                            let buttons_disabled_prev = new_page == 0;
                            let buttons_disabled_next = new_page + 1 >= total_pages;

                            let _ = msg_component.create_interaction_response(&ctx.http, |response| {
                                response.kind(serenity::InteractionResponseType::UpdateMessage)
                                    .interaction_response_data(|m| {
                                        m.embed(|e| {
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
                                                .footer(|f| f.text(format!("第 {}/{} 页", new_page + 1, total_pages)))
                                        })
                                        .components(|c| {
                                            c.create_action_row(|row| {
                                                row.create_button(|b| {
                                                    b.custom_id(format!("history_{}_{}_prev", target_user_id, new_page))
                                                        .label("上一页")
                                                        .style(serenity::ButtonStyle::Secondary)
                                                        .disabled(buttons_disabled_prev)
                                                })
                                                .create_button(|b| {
                                                    b.custom_id(format!("history_{}_{}_next", target_user_id, new_page))
                                                        .label("下一页")
                                                        .style(serenity::ButtonStyle::Secondary)
                                                        .disabled(buttons_disabled_next)
                                                })
                                            })
                                        })
                                    })
                            }).await;
                        }
                    }
                } else if cid.starts_with("stats_") {
                    // 处理存储统计分页交互，custom_id 格式: stats_{user_id}_{page}_{action}
                    let parts: Vec<&str> = cid.split('_').collect();
                    if parts.len() == 4 {
                        let target_user_id = parts[1];
                        let page: usize = parts[2].parse().unwrap_or(0);
                        let action = parts[3];
                        if msg_component.user.id.to_string() != *target_user_id {
                            let _ = msg_component.create_interaction_response(&ctx.http, |response| {
                                response.kind(serenity::InteractionResponseType::ChannelMessageWithSource)
                                    .interaction_response_data(|m| {
                                        m.content("❌ 无权操作此分页").ephemeral(true)
                                    })
                            }).await;
                        } else {
                            // 重新生成统计详情
                            let sessions = _data.api_client.session_manager.get_user_sessions(&target_user_id.to_string());
                            let session_dirs: Vec<std::path::PathBuf> = sessions
                                .iter()
                                .map(|s| _data.api_client.session_manager.get_session_dir(&s.id))
                                .collect();
                            let mut per_details = Vec::new();
                            let mut cleaned_total = 0;
                            let mut size_total = 0u64;
                            for (session, dir) in sessions.iter().zip(session_dirs.iter()) {
                                let cleaned_flag = dir.join(".cleaned").exists();
                                if cleaned_flag { cleaned_total += 1; }
                                let mut ss = 0u64;
                                if let Ok(entries) = std::fs::read_dir(dir) {
                                    for entry in entries.filter_map(Result::ok) {
                                        let path = entry.path();
                                        if let Some(ext) = path.extension().and_then(|e| e.to_str()).map(|s| s.to_lowercase()) {
                                            if ext == "png" || ext == "jpg" || ext == "jpeg" {
                                                if let Ok(meta) = std::fs::metadata(&path) {
                                                    ss += meta.len();
                                                }
                                            }
                                        }
                                    }
                                }
                                size_total += ss;
                                let short = if session.id.len() > 8 { &session.id[..8] } else { &session.id };
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
                            let total_images: u32 = sessions.iter().map(|s| s.images).sum();
                            let per_page = 10;
                            let detail_count = per_details.len();
                            let total_pages = (detail_count + per_page - 1) / per_page;
                            let mut new_page = match action {
                                "prev" if page > 0 => page - 1,
                                "next" => page + 1,
                                _ => page,
                            };
                            if new_page >= total_pages { new_page = total_pages.saturating_sub(1); }
                            let start = new_page * per_page;
                            let end = ((new_page + 1) * per_page).min(detail_count);
                            let page_details = &per_details[start..end];
                            let mut detail_text = page_details.join("\n");
                            if detail_text.chars().count() > 1024 {
                                detail_text = detail_text.chars().take(1021).collect::<String>() + "...";
                            }
                            let buttons_disabled_prev = new_page == 0;
                            let buttons_disabled_next = new_page + 1 >= total_pages;
                            let _ = msg_component.create_interaction_response(&ctx.http, |response| {
                                response.kind(serenity::InteractionResponseType::UpdateMessage)
                                    .interaction_response_data(|m| {
                                        m.embed(|e| {
                                            e.title("📊 存储统计（详细）")
                                                .color(0x3498db)
                                                .field("总会话数", sessions.len().to_string(), true)
                                                .field("已清理会话", cleaned_total.to_string(), true)
                                                .field("剩余图片数", total_images.to_string(), true)
                                                .field("总图片大小", format!("{:.2} KB", size_total as f64 / 1024.0), true)
                                                .footer(|f| f.text(format!("第 {}/{} 页", new_page + 1, total_pages)))
                                                .field("会话详情", detail_text, false)
                                        })
                                        .components(|c| {
                                            c.create_action_row(|row| {
                                                row.create_button(|b| {
                                                    b.custom_id(format!("stats_{}_{}_prev", target_user_id, new_page))
                                                        .label("上一页")
                                                        .style(serenity::ButtonStyle::Secondary)
                                                        .disabled(buttons_disabled_prev)
                                                })
                                                .create_button(|b| {
                                                    b.custom_id(format!("stats_{}_{}_next", target_user_id, new_page))
                                                        .label("下一页")
                                                        .style(serenity::ButtonStyle::Secondary)
                                                        .disabled(buttons_disabled_next)
                                                })
                                            })
                                        })
                                    })
                            }).await;
                        }
                    }
                } else {
                    debug!("收到消息组件交互: {:?}", cid);
                }
            } else {
                debug!("收到其他类型的交互");
            }
        }
        poise::Event::GuildCreate { guild, is_new: _ } => {
            info!("加入了服务器: {} (ID: {})", guild.name, guild.id);
        }
        poise::Event::Resume { .. } => {
            info!("会话已恢复");
        }
        poise::Event::CacheReady { .. } => {
            info!("缓存准备就绪");
        }
        _ => {}
    }
    Ok(())
}

// 错误处理
async fn on_error(error: poise::FrameworkError<'_, Data, anyhow::Error>) {
    match error {
        poise::FrameworkError::Setup { error, .. } => {
            error!("设置错误: {:?}", error);
        }
        poise::FrameworkError::Command { error, ctx, .. } => {
            error!("命令 '{}' 执行出错: {:?}", ctx.command().name, error);

            if let Err(e) = ctx.say(format!("❌ 命令执行出错: {}", error)).await {
                error!("发送错误消息失败: {:?}", e);
            }
        }
        poise::FrameworkError::CommandCheckFailed { error, ctx, .. } => {
            if let Some(error) = error {
                error!("命令检查失败: {:?}", error);
                if let Err(e) = ctx.say(format!("❌ 权限检查失败: {}", error)).await {
                    error!("发送错误消息失败: {:?}", e);
                }
            }
        }
        poise::FrameworkError::CommandPanic { payload, ctx, .. } => {
            error!("命令崩溃: {:?}", payload);
            if let Err(e) = ctx.say("❌ 内部错误: 命令执行崩溃").await {
                error!("发送错误消息失败: {:?}", e);
            }
        }
        poise::FrameworkError::ArgumentParse {
            error, input, ctx, ..
        } => {
            warn!("参数解析错误: {:?}, 输入: {:?}", error, input);
            if let Err(e) = ctx.say(format!("❌ 参数解析错误: {}", error)).await {
                error!("发送错误消息失败: {:?}", e);
            }
        }
        error => {
            error!("其他错误: {error:?}");
        }
    }
}
