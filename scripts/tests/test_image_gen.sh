#!/bin/bash

# 加载环境变量
source .env

echo "测试Markdown转图片功能..."

# 创建测试目录
mkdir -p data/pic/test

# 创建测试Markdown文件
TEST_MD_FILE="data/pic/test/test_markdown.md"
TEST_OUT_IMAGE="data/pic/test/test_output.png"

# 生成测试Markdown内容
cat > $TEST_MD_FILE << EOF
# 图片生成测试

这是一个**粗体**文本，这是*斜体*文本。

## 代码示例

\`\`\`rust
fn main() {
    println!("Hello, World!");
}
\`\`\`

## 列表示例

1. 第一项
2. 第二项
3. 第三项

- 无序列表项1
- 无序列表项2
  - 嵌套列表项

## 表格示例

| 姓名 | 年龄 | 职业 |
|------|------|------|
| 张三 | 25 | 程序员 |
| 李四 | 30 | 设计师 |

> 这是一段引用文本。这是一段引用文本。这是一段引用文本。

[这是一个链接](https://example.com)

---

### 最终测试行

这是最后一行测试文本，用于验证图片生成功能是否正常工作。
EOF

echo "测试Markdown文件已创建: $TEST_MD_FILE"

# 编译并运行测试代码
echo "编译测试代码..."
cat > src/bin/test_image.rs << EOF
use anyhow::Result;
use std::path::PathBuf;
use rust_discord_bot::config::Config;
use rust_discord_bot::image::ImageGenerator;

fn main() -> Result<()> {
    // 初始化配置
    let config = Config::init()?;
    
    println!("初始化图片生成器...");
    let image_generator = ImageGenerator::new(&config)?;
    
    // 读取测试Markdown文件
    let markdown_content = std::fs::read_to_string("data/pic/test/test_markdown.md")?;
    
    println!("开始生成图片...");
    let output_path = PathBuf::from("data/pic/test/test_output.png");
    
    // 生成图片
    let result_path = image_generator.create_image_from_markdown(&markdown_content, &output_path)?;
    
    println!("图片生成成功: {}", result_path.display());
    Ok(())
}
EOF

echo "构建测试程序..."
cargo build --bin test_image

echo "运行测试程序..."
cargo run --bin test_image

# 检查图片是否生成成功
if [ -f "$TEST_OUT_IMAGE" ]; then
    echo "测试成功 ✅"
    echo "生成的图片: $TEST_OUT_IMAGE"
    
    # 获取文件大小
    FILE_SIZE=$(du -h "$TEST_OUT_IMAGE" | cut -f1)
    echo "图片大小: $FILE_SIZE"
    
    echo "请检查生成的图片是否正确显示了Markdown内容。"
else
    echo "测试失败 ❌"
    echo "图片未生成"
fi 