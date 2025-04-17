use anyhow::Result;
use dotenv::dotenv;
use std::path::PathBuf;

// 这里我们导入主项目的模块
use rust_discord_bot::config;
use rust_discord_bot::image;

#[tokio::main]
async fn main() -> Result<()> {
    // 加载环境变量
    dotenv().ok();

    // 初始化日志
    tracing_subscriber::fmt::init();

    // 初始化配置
    let config = config::Config::init()?;
    println!("配置已加载");

    // 初始化目录
    config::init_directories(&config)?;

    // 创建图像生成器
    let image_generator = image::ImageGenerator::new(&config)?;
    println!("图像生成器已创建");

    // 测试Markdown
    let markdown = r#"# Markdown渲染测试

这是一个测试Markdown渲染功能的示例。

## 基本格式

**粗体文本** 和 *斜体文本*。

## 列表

无序列表:
- 项目1
- 项目2
- 项目3

有序列表:
1. 第一项
2. 第二项
3. 第三项

## 代码块

```rust
fn main() {
    println!("Hello, world!");
    let x = 42;
    let y = x * 2;
}
```

## 表格

| 名称 | 年龄 | 职业 |
|------|------|------|
| 张三 | 28   | 工程师 |
| 李四 | 32   | 设计师 |

## 引用

> 这是一段引用的文本。
> 
> 它可以包含多个段落。

## 任务列表

- [x] 已完成任务
- [ ] 未完成任务
- [ ] 另一个未完成任务

## 链接和图片

[链接文本](https://example.com)

这个测试不包含图片，因为我们只是测试渲染功能。

## 水平线

---

## 其他格式

上标：x^2^
下标：H~2~O

~~删除线文本~~
"#;

    // 保存到临时目录
    let output_dir = PathBuf::from("data/pic/temp");
    std::fs::create_dir_all(&output_dir)?;
    let output_path = output_dir.join("markdown_test.png");

    println!("开始渲染Markdown到图片: {}", output_path.display());

    // 渲染为图片
    let result = image_generator.create_image_from_markdown(markdown, &output_path)?;

    println!("渲染完成! 图片保存在: {}", result.display());
    println!("请检查图片以验证Markdown渲染是否正确");

    Ok(())
}
