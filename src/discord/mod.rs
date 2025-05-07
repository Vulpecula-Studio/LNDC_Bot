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
                qa_reply_slash(),
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
                debug!("收到消息组件交互: {:?}", msg_component.data.custom_id);
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
