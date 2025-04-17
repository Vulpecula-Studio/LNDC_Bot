use anyhow::{Context, Result};
use pulldown_cmark::{html, Options, Parser};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::config::Config;

#[derive(Debug)]
pub struct ImageGenerator {
    config: Config,
}

impl ImageGenerator {
    pub fn new(config: &Config) -> Result<Self> {
        // 确保至少一个字体文件存在
        let font_exists = config.font_paths.iter().any(|path| path.exists());

        if !font_exists {
            debug!("警告: 未找到有效的字体文件，将使用系统默认字体");
        } else {
            debug!("找到有效的字体文件: {:?}", config.font_paths);
        }

        Ok(Self {
            config: config.clone(),
        })
    }

    /// 从Markdown文本创建图片
    pub fn create_image_from_markdown(
        &self,
        markdown: &str,
        output_path: &Path,
    ) -> Result<PathBuf> {
        // 创建临时HTML文件
        let temp_html_path = self.create_temp_html_from_markdown(markdown)?;

        // 确保输出目录存在
        if let Some(parent) = output_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }

        debug!("临时HTML文件创建在: {}", temp_html_path.display());

        // 使用wkhtmltoimage渲染HTML为图片
        let image_path = self.render_markdown_to_image(&temp_html_path, output_path)?;

        debug!("图片已渲染至: {}", image_path.display());

        // 删除临时HTML文件
        let _ = fs::remove_file(temp_html_path);

        Ok(image_path)
    }

    /// 创建临时HTML文件
    fn create_temp_html_from_markdown(&self, markdown: &str) -> Result<PathBuf> {
        // 创建临时目录
        let temp_dir = self.config.image_output_dir.join("temp");
        if !temp_dir.exists() {
            fs::create_dir_all(&temp_dir)?;
        }

        // 生成临时文件名
        let temp_html_filename = format!("temp_{}.html", Uuid::new_v4());
        let temp_html_path = temp_dir.join(&temp_html_filename);

        // 将Markdown转换为HTML
        let html_content = self.markdown_to_html(markdown);

        // 写入临时HTML文件
        fs::write(&temp_html_path, html_content)?;

        Ok(temp_html_path)
    }

    /// 将Markdown转换为HTML
    pub(crate) fn markdown_to_html(&self, markdown: &str) -> String {
        // 获取字体设置
        let font_paths = self
            .config
            .font_paths
            .iter()
            .filter(|path| path.exists())
            .map(|path| path.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        let font_path = if !font_paths.is_empty() {
            // 使用第一个有效的字体路径
            font_paths[0].clone()
        } else {
            // 如果没有有效的字体，使用空字符串
            "".to_string()
        };

        let font_family = if !font_paths.is_empty() {
            "'LXGW WenKai', 'Microsoft YaHei', 'SimHei', sans-serif".to_string()
        } else {
            "sans-serif".to_string()
        };

        // 处理字体路径，确保能正确在wkhtmltoimage中使用
        let font_path_for_css = if !font_path.is_empty() {
            let path = Path::new(&font_path);
            if path.is_absolute() {
                // 已经是绝对路径，直接使用
                path.to_string_lossy().to_string()
            } else {
                // 对于相对路径，转换为绝对路径
                let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                current_dir.join(path).to_string_lossy().to_string()
            }
        } else {
            // 默认字体路径，使用绝对路径
            let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            current_dir
                .join("assets/fonts/LXGWWenKaiGBScreen.ttf")
                .to_string_lossy()
                .to_string()
        };

        debug!("使用字体路径: {}", font_path_for_css);

        // 创建HTML头部和样式
        let html_header = format!(
            r#"
        <!DOCTYPE html>
        <html>
        <head>
            <meta charset="UTF-8">
            <style>
                @font-face {{
                    font-family: 'LXGW WenKai';
                    src: local('LXGW WenKai'), url('{font_path}') format('truetype');
                    font-weight: normal;
                    font-style: normal;
                }}
                body {{
                    font-family: {font_family};
                    line-height: 1.8;
                    padding: {padding}px;
                    background-color: #333333;  /* 深灰色背景 */
                    color: #ffffff;  /* 白色文字 */
                    font-size: {font_size}px;
                    width: 1024px;
                    margin: 0 auto;
                    word-wrap: break-word;
                    overflow-wrap: break-word;
                    word-break: break-all;
                }}
                pre {{
                    background-color: #444444;  /* 更深的灰色作为代码块背景 */
                    padding: 10px;
                    border-radius: 5px;
                    overflow-x: auto;
                    white-space: pre-wrap;
                    word-wrap: break-word;
                    word-break: break-all;
                    font-size: {code_font_size}px;
                    color: #e0e0e0;  /* 浅灰色代码文字 */
                }}
                code {{
                    font-family: 'Courier New', monospace;
                    background-color: #444444;  /* 更深的灰色作为内联代码背景 */
                    padding: 2px 4px;
                    border-radius: 3px;
                    white-space: pre-wrap;
                    word-wrap: break-word;
                    color: #e0e0e0;  /* 浅灰色代码文字 */
                }}
                blockquote {{
                    border-left: 4px solid #666666;  /* 更亮的灰色边框 */
                    padding-left: 15px;
                    color: #cccccc;  /* 浅色引用文字 */
                    margin-left: 0;
                }}
                img {{
                    max-width: 100%;
                    height: auto;
                }}
                table {{
                    border-collapse: collapse;
                    width: 100%;
                    margin: 15px 0;
                    table-layout: fixed;
                }}
                table, th, td {{
                    border: 1px solid #555555;  /* 更亮的灰色边框 */
                    padding: 8px;
                    word-wrap: break-word;
                    overflow-wrap: break-word;
                }}
                th {{
                    background-color: #444444;  /* 深灰色表头背景 */
                    text-align: left;
                    color: #ffffff;  /* 白色表头文字 */
                }}
                h1, h2, h3, h4, h5, h6 {{
                    margin-top: 20px;
                    margin-bottom: 10px;
                    color: #ffffff;  /* 白色标题 */
                    line-height: 1.4;
                }}
                h1 {{
                    font-size: 28px;
                    border-bottom: 1px solid #555555;  /* 灰色边框 */
                    padding-bottom: 10px;
                }}
                h2 {{
                    font-size: 24px;
                    border-bottom: 1px solid #555555;  /* 灰色边框 */
                    padding-bottom: 8px;
                }}
                h3 {{
                    font-size: 20px;
                }}
                p {{
                    margin: 15px 0;
                    text-align: justify;
                    word-wrap: break-word;
                    overflow-wrap: break-word;
                    word-break: break-all;
                    color: #ffffff;  /* 确保段落文字是白色 */
                }}
                ul, ol {{
                    margin: 15px 0;
                    padding-left: 30px;
                    color: #ffffff;  /* 确保列表文字是白色 */
                }}
                li {{
                    margin-bottom: 5px;
                    word-wrap: break-word;
                    color: #ffffff;  /* 确保列表项文字是白色 */
                }}
                a {{
                    color: #66b3ff;  /* 亮蓝色链接 */
                    text-decoration: none;
                    word-break: break-all;
                }}
                a:hover {{
                    text-decoration: underline;
                }}
                hr {{
                    border: 0;
                    height: 1px;
                    background-color: #555555;  /* 灰色分隔线 */
                    margin: 20px 0;
                }}
                /* 代码高亮样式 */
                .hljs-keyword {{
                    color: #ff9580;  /* 调整为亮色以适应深色背景 */
                }}
                .hljs-string {{
                    color: #a8e08f;  /* 调整为亮色以适应深色背景 */
                }}
                .hljs-number {{
                    color: #66d9ef;  /* 调整为亮色以适应深色背景 */
                }}
                .hljs-comment {{
                    color: #b0b0b0;  /* 调整为亮色以适应深色背景 */
                }}
            </style>
        </head>
        <body>
        "#,
            font_family = font_family,
            padding = self.config.padding,
            font_size = self.config.font_size,
            code_font_size = self.config.font_size - 2,
            font_path = font_path_for_css
        );

        // 使用pulldown-cmark解析Markdown
        // 启用所有扩展功能
        let mut options = Options::empty();
        options.insert(Options::ENABLE_STRIKETHROUGH);
        options.insert(Options::ENABLE_TABLES);
        options.insert(Options::ENABLE_FOOTNOTES);
        options.insert(Options::ENABLE_TASKLISTS);

        let parser = Parser::new_ext(markdown, options);

        // 转换为HTML
        let mut html_content = String::new();
        html::push_html(&mut html_content, parser);

        // 构建完整的HTML
        let result = format!("{}{}</body></html>", html_header, html_content);

        debug!("HTML内容生成完成，长度: {} 字节", result.len());

        result
    }

    /// 将HTML渲染为图片
    fn render_markdown_to_image(&self, html_path: &Path, output_path: &Path) -> Result<PathBuf> {
        // 构建wkhtmltoimage命令
        let wkhtmltoimage_path = match std::env::var("WKHTMLTOIMAGE_PATH") {
            Ok(path) if !path.is_empty() => {
                debug!("使用自定义wkhtmltoimage路径: {}", path);
                path
            }
            _ => {
                debug!("使用默认wkhtmltoimage路径");
                "wkhtmltoimage".to_string()
            }
        };

        // 获取当前工作目录作为基础路径
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let current_dir_str = current_dir.to_string_lossy();

        debug!("当前工作目录: {}", current_dir_str);
        debug!(
            "执行命令: {} --quality 95 --width 1024 --enable-local-file-access {} {}",
            wkhtmltoimage_path,
            html_path.display(),
            output_path.display()
        );

        // 使用wkhtmltoimage渲染HTML为图片
        let output = Command::new(&wkhtmltoimage_path)
            .arg("--quality")
            .arg("95") // 提高图片质量
            .arg("--width")
            .arg("1024") // 固定宽度
            .arg("--encoding")
            .arg("UTF-8") // 确保使用UTF-8编码
            .arg("--enable-local-file-access") // 允许访问本地文件
            .arg("--disable-javascript") // 禁用JavaScript以提高稳定性
            .arg(html_path.to_str().unwrap())
            .arg(output_path.to_str().unwrap())
            .output()
            .context("运行wkhtmltoimage失败，请确保已安装")?;

        if !output.status.success() {
            error!("wkhtmltoimage命令执行失败");
            error!("错误输出: {}", String::from_utf8_lossy(&output.stderr));
            return Err(anyhow::anyhow!(
                "wkhtmltoimage命令执行失败: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        info!("图片渲染成功: {}", output_path.display());
        Ok(output_path.to_path_buf())
    }
}

#[cfg(test)]
mod tests {
    use super::ImageGenerator;
    use crate::config::Config;
    use std::path::PathBuf;

    #[test]
    fn markdown_to_html_basic() {
        // 构造简易配置
        let config = Config {
            root_dir: PathBuf::from("."),
            data_dir: PathBuf::from("data"),
            fastgpt_api_url: String::new(),
            fastgpt_auth_token: String::new(),
            image_output_dir: PathBuf::from("data/pic"),
            font_paths: vec![],
            font_size: 24,
            padding: 30,
            discord_token: String::new(),
            discord_channel_whitelist: vec![],
            session_expiry: 0,
            api_concurrency_limit: 1,
        };
        let gen = ImageGenerator::new(&config).expect("创建 ImageGenerator 失败");
        let html = gen.markdown_to_html("# Hello\n\nWorld");
        assert!(html.contains("<h1>Hello</h1>"), "应包含 H1 标记");
        assert!(html.contains("<p>World</p>"), "应包含段落标记");
        // 检查样式片段
        assert!(html.contains("<style>"), "应包含样式标签");
    }
}
