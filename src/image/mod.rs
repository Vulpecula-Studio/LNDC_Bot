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
        let _font_exists = config.font_paths.iter().any(|path| path.exists());

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
                    src: local('LXGW WenKai'), url('file://{font_path_for_css}') format('truetype');
                    font-weight: normal;
                    font-style: normal;
                }}
                @font-face {{
                    font-family: 'Code Font';
                    src: local('Consolas'), local('Source Code Pro'), local('DejaVu Sans Mono'), local('Courier New'), local('Menlo');
                    font-weight: normal;
                    font-style: normal;
                }}
                body {{
                    font-family: {font_family};
                    line-height: 1.8;
                    padding: {padding}px;
                    background-color: #2b2b2b;  /* 稍微暗一点的灰色背景 */
                    color: #f0f0f0;  /* 更柔和的白色文字 */
                    font-size: {font_size}px;
                    width: 1024px;
                    margin: 0 auto;
                    word-wrap: break-word;
                    overflow-wrap: break-word;
                    word-break: break-all;
                    text-shadow: 0 1px 1px rgba(0, 0, 0, 0.1);  /* 微妙的文字阴影 */
                }}
                pre {{
                    font-family: 'Code Font', {font_family}, monospace;
                    background-color: #383838;  /* 更深的灰色作为代码块背景 */
                    padding: 16px;
                    border-radius: 8px;
                    overflow-x: auto;
                    white-space: pre-wrap;
                    word-wrap: break-word;
                    word-break: break-all;
                    font-size: {code_font_size}px;
                    color: #e0e0e0;  /* 浅灰色代码文字 */
                    border-left: 3px solid #666666;  /* 左侧边框 */
                    margin: 20px 0;  /* 增加边距 */
                    box-shadow: 0 2px 5px rgba(0, 0, 0, 0.15);  /* 微妙的阴影 */
                }}
                code {{
                    font-family: 'Code Font', {font_family}, monospace;
                    background-color: #454545;  /* 内联代码背景 */
                    padding: 3px 6px;
                    border-radius: 4px;
                    white-space: pre-wrap;
                    word-wrap: break-word;
                    color: #e0e0e0;  /* 浅灰色代码文字 */
                }}
                blockquote {{
                    border-left: 4px solid #777777;  /* 更亮的灰色边框 */
                    padding: 10px 20px;
                    margin: 20px 0;
                    background-color: #323232;  /* 微妙的背景色 */
                    border-radius: 0 8px 8px 0;  /* 右侧圆角 */
                    color: #d0d0d0;  /* 浅色引用文字 */
                }}
                img {{
                    max-width: 100%;
                    height: auto;
                    border-radius: 8px;  /* 图片圆角 */
                    margin: 20px 0;
                    box-shadow: 0 3px 10px rgba(0, 0, 0, 0.2);  /* 图片阴影 */
                }}
                table {{
                    border-collapse: collapse;
                    width: 100%;
                    margin: 25px 0;
                    table-layout: fixed;
                    border-radius: 8px;
                    overflow: hidden;  /* 确保圆角有效 */
                    box-shadow: 0 2px 5px rgba(0, 0, 0, 0.1);  /* 表格阴影 */
                }}
                table, th, td {{
                    border: 1px solid #555555;  /* 表格边框 */
                    padding: 12px;
                    word-wrap: break-word;
                    overflow-wrap: break-word;
                }}
                th {{
                    background-color: #444444;  /* 深灰色表头背景 */
                    text-align: left;
                    color: #ffffff;  /* 白色表头文字 */
                    font-weight: bold;
                }}
                tr:nth-child(even) {{
                    background-color: #333333;  /* 交替行颜色 */
                }}
                h1, h2, h3, h4, h5, h6 {{
                    margin-top: 30px;
                    margin-bottom: 15px;
                    color: #ffffff;  /* 白色标题 */
                    line-height: 1.4;
                    font-weight: 600;
                }}
                h1 {{
                    font-size: 32px;
                    border-bottom: 2px solid #555555;  /* 灰色边框 */
                    padding-bottom: 10px;
                    margin-bottom: 25px;
                    text-align: center;  /* 居中标题 */
                }}
                h2 {{
                    font-size: 28px;
                    border-bottom: 1px solid #555555;  /* 灰色边框 */
                    padding-bottom: 8px;
                    margin-top: 40px;  /* 增加间距 */
                }}
                h3 {{
                    font-size: 24px;
                    color: #e0e0e0;  /* 稍微变淡 */
                }}
                p {{
                    margin: 18px 0;
                    text-align: justify;
                    word-wrap: break-word;
                    overflow-wrap: break-word;
                    word-break: break-all;
                    color: #f0f0f0;  /* 确保段落文字是柔和的白色 */
                    line-height: 1.8;
                }}
                ul, ol {{
                    margin: 18px 0;
                    padding-left: 30px;
                    color: #f0f0f0;  /* 确保列表文字颜色 */
                }}
                li {{
                    margin-bottom: 8px;
                    word-wrap: break-word;
                    color: #f0f0f0;  /* 确保列表项文字颜色 */
                    line-height: 1.6;
                }}
                li > ul, li > ol {{
                    margin: 10px 0 10px 20px;  /* 嵌套列表的间距 */
                }}
                a {{
                    color: #78a9ff;  /* 亮蓝色链接，更柔和 */
                    text-decoration: none;
                    word-break: break-all;
                    border-bottom: 1px dotted #78a9ff;  /* 下划线效果 */
                    padding-bottom: 1px;
                }}
                a:hover {{
                    color: #a1c4ff;  /* 悬停色 */
                    border-bottom: 1px solid #a1c4ff;
                }}
                hr {{
                    border: 0;
                    height: 1px;
                    background-image: linear-gradient(to right, rgba(85, 85, 85, 0), rgba(85, 85, 85, 0.75), rgba(85, 85, 85, 0));  /* 渐变分隔线 */
                    margin: 30px 0;
                }}
                /* 代码高亮样式 - 更丰富的配色方案 */
                .hljs-keyword {{
                    color: #ff9580;  /* 关键字颜色 */
                    font-weight: bold;
                }}
                .hljs-string {{
                    color: #b5e88f;  /* 字符串颜色，更鲜明 */
                }}
                .hljs-number {{
                    color: #79d4f3;  /* 数字颜色，更柔和 */
                }}
                .hljs-comment {{
                    color: #b0b0b0;  /* 注释颜色 */
                    font-style: italic;
                }}
                .hljs-function {{
                    color: #d9a9ff;  /* 函数名颜色 */
                }}
                .hljs-parameter {{
                    color: #ffcc66;  /* 参数颜色 */
                }}
                .hljs-tag {{
                    color: #ff8080;  /* 标签颜色 */
                }}
                .hljs-attr {{
                    color: #8cdaff;  /* 属性颜色 */
                }}
                /* 任务列表样式 */
                ul.task-list {{
                    list-style-type: none;
                    padding-left: 20px;
                }}
                .task-list-item {{
                    position: relative;
                    padding-left: 25px;
                }}
                .task-list-item input {{
                    position: absolute;
                    left: 0;
                    top: 3px;
                }}
                /* 脚注样式 */
                .footnote {{
                    font-size: 0.9em;
                    color: #cccccc;
                    margin-top: 40px;
                    padding-top: 10px;
                    border-top: 1px dotted #555555;
                }}
                .footnote-ref {{
                    vertical-align: super;
                    font-size: 0.8em;
                }}
            </style>
        </head>
        <body>
        "#,
            font_family = font_family,
            padding = self.config.padding,
            font_size = self.config.font_size,
            code_font_size = self.config.font_size - 2,
            font_path_for_css = font_path_for_css
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

        result
    }

    /// 将HTML渲染为图片
    fn render_markdown_to_image(&self, html_path: &Path, output_path: &Path) -> Result<PathBuf> {
        // 构建wkhtmltoimage命令
        let wkhtmltoimage_path = match std::env::var("WKHTMLTOIMAGE_PATH") {
            Ok(path) if !path.is_empty() => {
                path
            }
            _ => {
                "wkhtmltoimage".to_string()
            }
        };

        // 获取当前工作目录作为基础路径
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let _current_dir_str = current_dir.to_string_lossy();

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
