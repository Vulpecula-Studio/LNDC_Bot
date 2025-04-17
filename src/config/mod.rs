use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Config {
    // 应用根目录
    #[allow(dead_code)]
    pub root_dir: PathBuf,

    // 数据目录
    pub data_dir: PathBuf,

    // FastGPT配置
    pub fastgpt_api_url: String,
    pub fastgpt_auth_token: String,

    // 图片生成配置
    pub image_output_dir: PathBuf,
    pub font_paths: Vec<PathBuf>,
    #[allow(dead_code)]
    pub font_size: u32,
    #[allow(dead_code)]
    pub padding: u32,

    // Discord配置
    pub discord_token: String,
    #[allow(dead_code)]
    pub discord_channel_whitelist: Vec<String>,

    // 会话配置
    #[allow(dead_code)]
    pub session_expiry: u64,
    // API 并发请求限制
    #[allow(dead_code)]
    pub api_concurrency_limit: usize,
}

impl Config {
    pub fn init() -> Result<Self> {
        // 环境变量加载由 main.rs 统一进行

        // 获取应用根目录
        let root_dir = env::current_dir().context("无法获取当前目录")?;

        // 数据目录
        let data_dir = root_dir.join("data");

        // 图片输出目录
        let image_output_dir = data_dir.join("pic");

        // FastGPT配置
        let fastgpt_api_url = env::var("FASTGPT_API_URL").context("缺少FASTGPT_API_URL环境变量")?;

        let fastgpt_auth_token =
            env::var("FASTGPT_AUTH_TOKEN").context("缺少FASTGPT_AUTH_TOKEN环境变量")?;

        // 字体配置
        let font_paths_str = env::var("FONT_PATHS")
            .unwrap_or_else(|_| "./assets/fonts/LXGWWenKaiGBScreen.ttf".to_string());

        let font_paths: Vec<PathBuf> = font_paths_str
            .split(',')
            .map(|p| Path::new(p.trim()).to_path_buf())
            .collect();
        // 校验字体路径
        for path in &font_paths {
            if !path.exists() {
                tracing::warn!("字体文件不存在: {}，请检查 FONT_PATHS 设置", path.display());
            }
        }

        let font_size = env::var("FONT_SIZE")
            .unwrap_or_else(|_| "24".to_string())
            .parse()
            .context("FONT_SIZE必须是数字")?;

        let padding = env::var("PADDING")
            .unwrap_or_else(|_| "30".to_string())
            .parse()
            .context("PADDING必须是数字")?;

        // Discord配置
        let discord_token = env::var("DISCORD_TOKEN").context("缺少DISCORD_TOKEN环境变量")?;

        let discord_channel_whitelist = env::var("DISCORD_CHANNEL_WHITELIST")
            .unwrap_or_default()
            .split(',')
            .filter(|s| !s.trim().is_empty())
            .map(String::from)
            .collect();

        // 会话配置
        let session_expiry = env::var("SESSION_EXPIRY")
            .unwrap_or_else(|_| "3600".to_string())
            .parse()
            .context("SESSION_EXPIRY必须是数字")?;

        // API 并发请求限制，默认 5
        let api_concurrency_limit = env::var("FASTGPT_CONCURRENCY_LIMIT")
            .unwrap_or_else(|_| "5".to_string())
            .parse()
            .context("FASTGPT_CONCURRENCY_LIMIT 必须是数字")?;

        Ok(Config {
            root_dir,
            data_dir,
            fastgpt_api_url,
            fastgpt_auth_token,
            image_output_dir,
            font_paths,
            font_size,
            padding,
            discord_token,
            discord_channel_whitelist,
            session_expiry,
            api_concurrency_limit,
        })
    }
}

pub fn init_directories(config: &Config) -> Result<()> {
    // 创建所需的目录
    let directories = [
        &config.data_dir,
        &config.image_output_dir,
        &config.data_dir.join("sessions"),
        &config.data_dir.join("logs"),
    ];

    for dir in directories.iter() {
        if !dir.exists() {
            fs::create_dir_all(dir).context(format!("无法创建目录: {}", dir.display()))?;
            tracing::info!("已创建目录: {}", dir.display());
        }
    }

    Ok(())
}
