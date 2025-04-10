#!/bin/bash

# 加载环境变量
source .env

echo "测试长文本换行功能..."

# 创建测试目录
mkdir -p data/pic/test

# 创建测试Markdown文件
TEST_MD_FILE="data/pic/test/test_long_text.md"
TEST_OUT_IMAGE="data/pic/test/test_long_text.png"

# 生成测试Markdown内容
cat > $TEST_MD_FILE << EOF
# 长文本换行测试

## 长句测试

这是一个非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常长的句子，测试是否能够正确自动换行。

## 无空格长文本测试

这是一段没有空格的超长文本测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试测试。

## 长URL测试

[这是一个非常长的URL链接，测试是否能够正确换行显示https://example.com/this/is/a/very/very/very/very/very/very/very/very/very/very/very/very/very/very/very/very/very/very/very/very/very/very/very/very/very/very/very/very/very/long/url/for/testing/word/wrapping](https://example.com)

## 长代码行测试

\`\`\`rust
fn very_long_function_name_that_should_wrap_properly_in_the_generated_image(very_long_parameter_name_1: &str, very_long_parameter_name_2: &str, very_long_parameter_name_3: &str, very_long_parameter_name_4: &str) -> Result<VeryLongReturnTypeNameThatShouldAlsoWrapProperly, VeryLongErrorTypeNameThatShouldWrapAsWell> {
    println!("这是一行非常长的代码，测试代码块内的换行功能是否正常工作。这行代码应该能够自动换行以适应图片的宽度，而不应该导致水平滚动条出现。");
    Ok(VeryLongReturnTypeNameThatShouldAlsoWrapProperly::new())
}
\`\`\`

## 长表格行测试

| 列标题1 | 列标题2 | 这是一个非常长的表格列标题，测试表格是否能够正确处理长内容 | 列标题4 |
|---------|---------|----------------------------------------------------------|---------|
| 单元格1 | 单元格2 | 这是一个非常长的表格单元格内容，测试表格换行功能是否正常工作。这行内容应该能够自动换行以适应表格的宽度，而不应该超出表格边界 | 单元格4 |

## 长引用测试

> 这是一段非常长的引用文本，测试引用块内的换行功能是否正常工作。引用文本通常包含较长的段落，因此换行处理对于引用块特别重要。这段引用文本应该能够自动换行以适应图片的宽度，而不应该导致水平滚动或文本截断。这是为了确保所有类型的Markdown元素都能正确地处理长文本换行问题。

## 长列表项测试

- 这是一个非常长的列表项内容，测试列表项内的换行功能是否正常工作。列表项有时会包含较长的说明文本，因此换行处理对于列表项也非常重要。这个列表项应该能够自动换行以适应图片的宽度。
- 第二个列表项，内容较短

## 连续长段落测试

这是第一个长段落，内容非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常长。测试连续长段落的换行和段落间距是否正常。

这是第二个长段落，内容同样非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常非常长。两个连续长段落之间应该有适当的间距，并且每个段落都应该正确换行。

## 中文长文本测试

这是一段中文长文本测试，中文文本的换行规则可能与英文不同。中文文本通常可以在任何字符之间换行，而不需要像英文那样在单词之间换行。这段中文文本应该能够根据图片宽度自动适应并正确换行，而不会出现不美观的断行或者超出边界的情况。中文文本测试对于支持多语言的应用程序非常重要，尤其是对于像我们这样的全球化应用。

## 混合中英文长文本测试

这是一段混合了中文和English的长文本测试，混合文本的换行可能会更加复杂，因为需要同时处理中文和英文的换行规则。This is a mixed Chinese and English long text test, which may be more complicated for line wrapping because it needs to handle both Chinese and English line breaking rules. 混合文本应该能够正确地在适当的位置换行，无论是中文字符之间还是英文单词之间。This mixed text should be able to wrap correctly at appropriate positions, whether between Chinese characters or between English words.

EOF

echo "测试长文本Markdown文件已创建: $TEST_MD_FILE"

# 编译并运行测试代码
echo "编译测试代码..."
cat > src/bin/test_long_text.rs << EOF
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
    let markdown_content = std::fs::read_to_string("data/pic/test/test_long_text.md")?;
    
    println!("开始生成长文本测试图片...");
    let output_path = PathBuf::from("data/pic/test/test_long_text.png");
    
    // 生成图片
    let result_path = image_generator.create_image_from_markdown(&markdown_content, &output_path)?;
    
    println!("长文本测试图片生成成功: {}", result_path.display());
    Ok(())
}
EOF

echo "构建测试程序..."
cargo build --bin test_long_text

echo "运行测试程序..."
cargo run --bin test_long_text

# 检查图片是否生成成功
if [ -f "$TEST_OUT_IMAGE" ]; then
    echo "测试成功 ✅"
    echo "生成的图片: $TEST_OUT_IMAGE"
    
    # 获取文件大小
    FILE_SIZE=$(du -h "$TEST_OUT_IMAGE" | cut -f1)
    echo "图片大小: $FILE_SIZE"
    
    echo "请检查生成的图片是否正确处理了长文本的换行。"
else
    echo "测试失败 ❌"
    echo "图片未生成"
fi 