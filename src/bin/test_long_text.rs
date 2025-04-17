use anyhow::Result;
use rust_discord_bot::config::Config;
use rust_discord_bot::image::ImageGenerator;
use std::path::PathBuf;

fn main() -> Result<()> {
    // 初始化配置
    let config = Config::init()?;

    println!("初始化图片生成器...");
    let image_generator = ImageGenerator::new(&config)?;

    // 读取测试Markdown文件
    let markdown_content = std::fs::read_to_string("data/pic/test/test_long_text.md")?;
    println!("开始生成长文本测试图片...");
    let output_path = PathBuf::from("data/pic/test/test_long_text.png");

    // 生成图片
    let result_path =
        image_generator.create_image_from_markdown(&markdown_content, &output_path)?;
    println!("长文本测试图片生成成功: {}", result_path.display());
    Ok(())
}
