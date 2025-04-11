use anyhow::{Result, Context};
use std::path::{Path, PathBuf};
use std::fs;
use std::process::Command;
use uuid::Uuid;
use pulldown_cmark::{Parser, Options, html};
use tracing::{info, error, debug};

use crate::config::Config;

#[derive(Debug)]
pub struct ImageGenerator {
    config: Config,
}

impl ImageGenerator {
    pub fn new(config: &Config) -> Result<Self> {
        // 确保至少一个字体文件存在
        let font_exists = config.font_paths.iter()
            .any(|path| path.exists());
            
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
    pub fn create_image_from_markdown(&self, markdown: &str, output_path: &Path) -> Result<PathBuf> {
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
    fn markdown_to_html(&self, markdown: &str) -> String {
        // 获取字体设置
        let font_paths = self.config.font_paths.iter()
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
            format!("'LXGW WenKai', 'Microsoft YaHei', 'SimHei', sans-serif")
        } else {
            "sans-serif".to_string()
        };
        
        // 处理字体路径，确保能正确在wkhtmltoimage中使用
        // 注意：使用相对路径，避免使用绝对路径
        let font_path_for_css = if !font_path.is_empty() {
            let path = Path::new(&font_path);
            if path.is_absolute() {
                // 生成一个相对路径形式
                path.file_name()
                    .map(|f| format!("./assets/fonts/{}", f.to_string_lossy()))
                    .unwrap_or_default()
            } else {
                font_path
            }
        } else {
            "./assets/fonts/LXGWWenKaiGBScreen.ttf".to_string()
        };
        
        debug!("使用字体路径: {}", font_path_for_css);
        
        // 创建HTML头部和样式
        let html_header = format!(r#"
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
                    background-color: white;
                    color: #333;
                    font-size: {font_size}px;
                    width: 1024px;
                    margin: 0 auto;
                    word-wrap: break-word;
                    overflow-wrap: break-word;
                    word-break: break-all;
                }}
                pre {{
                    background-color: #f5f5f5;
                    padding: 10px;
                    border-radius: 5px;
                    overflow-x: auto;
                    white-space: pre-wrap;
                    word-wrap: break-word;
                    word-break: break-all;
                    font-size: {code_font_size}px;
                }}
                code {{
                    font-family: 'Courier New', monospace;
                    background-color: #f5f5f5;
                    padding: 2px 4px;
                    border-radius: 3px;
                    white-space: pre-wrap;
                    word-wrap: break-word;
                }}
                blockquote {{
                    border-left: 4px solid #ddd;
                    padding-left: 15px;
                    color: #666;
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
                    border: 1px solid #ddd;
                    padding: 8px;
                    word-wrap: break-word;
                    overflow-wrap: break-word;
                }}
                th {{
                    background-color: #f2f2f2;
                    text-align: left;
                }}
                h1, h2, h3, h4, h5, h6 {{
                    margin-top: 20px;
                    margin-bottom: 10px;
                    color: #222;
                    line-height: 1.4;
                }}
                h1 {{
                    font-size: 28px;
                    border-bottom: 1px solid #eee;
                    padding-bottom: 10px;
                }}
                h2 {{
                    font-size: 24px;
                    border-bottom: 1px solid #eee;
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
                }}
                ul, ol {{
                    margin: 15px 0;
                    padding-left: 30px;
                }}
                li {{
                    margin-bottom: 5px;
                    word-wrap: break-word;
                }}
                a {{
                    color: #0366d6;
                    text-decoration: none;
                    word-break: break-all;
                }}
                a:hover {{
                    text-decoration: underline;
                }}
                hr {{
                    border: 0;
                    height: 1px;
                    background-color: #ddd;
                    margin: 20px 0;
                }}
                /* 代码高亮样式 */
                .hljs-keyword {{
                    color: #a71d5d;
                }}
                .hljs-string {{
                    color: #183691;
                }}
                .hljs-number {{
                    color: #0086b3;
                }}
                .hljs-comment {{
                    color: #969896;
                }}
            </style>
        </head>
        <body>
        "#, 
        font_family = font_family,
        padding = self.config.padding,
        font_size = self.config.font_size,
        code_font_size = self.config.font_size - 2,
        font_path = font_path_for_css);
        
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
            },
            _ => {
                debug!("使用默认wkhtmltoimage路径");
                "wkhtmltoimage".to_string()
            }
        };
        
        // 获取当前工作目录作为基础路径
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let current_dir_str = current_dir.to_string_lossy();
        
        debug!("当前工作目录: {}", current_dir_str);
        debug!("执行命令: {} --quality 95 --width 1024 --enable-local-file-access {} {}", 
               wkhtmltoimage_path, 
               html_path.display(), 
               output_path.display());
        
        // 使用wkhtmltoimage渲染HTML为图片
        let output = Command::new(&wkhtmltoimage_path)
            .arg("--quality")
            .arg("95")     // 提高图片质量
            .arg("--width")
            .arg("1024")   // 固定宽度
            .arg("--encoding")
            .arg("UTF-8")  // 确保使用UTF-8编码
            .arg("--enable-local-file-access")  // 允许访问本地文件
            .arg("--disable-javascript")  // 禁用JavaScript以提高稳定性
            .arg(html_path.to_str().unwrap())
            .arg(output_path.to_str().unwrap())
            .output()
            .context("运行wkhtmltoimage失败，请确保已安装")?;
            
        if !output.status.success() {
            error!("wkhtmltoimage命令执行失败");
            error!("错误输出: {}", String::from_utf8_lossy(&output.stderr));
            return Err(anyhow::anyhow!("wkhtmltoimage命令执行失败: {}", 
                                  String::from_utf8_lossy(&output.stderr)));
        }
        
        info!("图片渲染成功: {}", output_path.display());
        Ok(output_path.to_path_buf())
    }
} 