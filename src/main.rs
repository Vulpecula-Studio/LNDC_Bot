mod api;
mod config;
mod discord;
mod image;
mod session;

use anyhow::Result;
use tracing::{error, info};
use tracing_subscriber::{fmt, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志系统，支持环境变量RUST_LOG设置日志级别
    fmt::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(true) // 显示目标模块
        .with_thread_ids(true) // 显示线程ID
        .with_thread_names(true) // 显示线程名称
        .with_file(true) // 显示文件名
        .with_line_number(true) // 显示行号
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
