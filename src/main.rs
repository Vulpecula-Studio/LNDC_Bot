mod api;
mod config;
mod discord;
mod image;
mod session;

use anyhow::Result;
use chrono::Local;
use dotenv::dotenv;
use tracing::{error, info};
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::time::FormatTime;
use tracing_subscriber::{fmt, EnvFilter};

struct LocalOnlyTime;

impl FormatTime for LocalOnlyTime {
    fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
        let now = Local::now().format("%Y-%m-%d %H:%M:%S");
        write!(w, "{}", now)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // 先加载 .env 中的环境变量，确保日志级别设置生效
    dotenv().ok();
    // 设置日志级别：INFO 为默认，项目模块启用 DEBUG，可通过 RUST_LOG 环境变量覆盖
    let default_filter = "info,rust_discord_bot=debug,rust_discord_bot::api=debug,rust_discord_bot::discord=debug,rust_discord_bot::image=debug";
    // 仅使用默认过滤，避免外部库的 DEBUG 日志
    let env_filter = EnvFilter::new(default_filter.to_string());
    fmt::fmt()
        .with_env_filter(env_filter)
        .with_timer(LocalOnlyTime) // 只输出日期和时分秒
        .compact() // 使用精简格式，去除多余字段
        .init();

    info!("日志系统已初始化");

    // 初始化配置
    let config = config::Config::init()?;
    // 创建会话管理器并启动定期清理任务
    let session_manager = session::SessionManager::new(&config);
    tokio::spawn(async move {
        let expiry_days = config.session_expiry;
        let interval = std::time::Duration::from_secs(expiry_days * 24 * 60 * 60);
        loop {
            session_manager.periodic_cleanup(expiry_days).await;
            tokio::time::sleep(interval).await;
        }
    });

    info!("配置已加载");

    // 初始化目录
    config::init_directories(&config)?;

    // 启动Discord机器人
    info!("正在启动Discord机器人...");
    match discord::start_bot(&config).await {
        Ok(_) => info!("Discord机器人已关闭"),
        Err(e) => error!("Discord机器人运行错误: {}", e),
    }

    Ok(())
}
