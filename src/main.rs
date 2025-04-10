mod config;
mod api;
mod discord;
mod image;
mod session;

use anyhow::Result;
use dotenv::dotenv;
use tracing::{info, error};
use tracing_subscriber::{fmt, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    // 加载环境变量
    dotenv().ok();
    
    // 初始化日志系统，支持环境变量RUST_LOG设置日志级别
    fmt::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(true)  // 显示目标模块
        .with_thread_ids(true)  // 显示线程ID
        .with_thread_names(true)  // 显示线程名称
        .with_file(true)  // 显示文件名
        .with_line_number(true)  // 显示行号
        .init();
    
    info!("日志系统已初始化");
    
    // 初始化配置
    let config = config::Config::init()?;
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
