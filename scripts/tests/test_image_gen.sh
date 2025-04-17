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
# 图片生成测试 - 长文本效果

这是一个**粗体**文本，这是*斜体*文本。下面将展示各种长文本和格式效果。

## 代码示例

\`\`\`rust
fn main() {
    println!("Hello, World!");
    
    // 这是一个简单的循环示例
    for i in 0..10 {
        println!("当前循环次数: {}", i);
    }
    
    // 测试函数
    let result = calculate_sum(5, 10);
    println!("计算结果: {}", result);
}

fn calculate_sum(a: i32, b: i32) -> i32 {
    a + b
}
\`\`\`

## 长段落测试

这是一段较长的文本内容，用于测试图片生成时的文本换行和排版效果。我们需要确保长文本能够正确地在生成的图片中显示，包括自动换行、缩进和对齐等。这段文字会比较长，目的是测试图片生成器处理长文本的能力。我们现在使用了更大的字体和灰色背景，希望能提升整体的可读性。

再添加一段长文本，进一步测试多段落的效果。段落之间应当有适当的间距，而且每个段落内部的文字应当对齐良好。在处理中文时，要特别注意标点符号的处理，确保它们不会单独出现在行首。另外，中文和英文、数字混排时的间距也需要合理。

### 列表测试

1. 第一项 - 这是一个比较长的列表项内容，测试列表项中的长文本换行效果
2. 第二项 - 另一个长列表项，包含更多的文字内容，用来验证列表的显示效果
3. 第三项 - 这里包含了 **粗体** 和 *斜体* 以及 \`代码\` 等多种格式

- 无序列表项1 - 较长的无序列表项内容测试
- 无序列表项2 - 另一个长内容的无序列表项
  - 嵌套列表项1 - 测试嵌套列表的显示效果
  - 嵌套列表项2 - 另一个嵌套列表项，内容较长以测试换行

## 表格示例

| 姓名 | 年龄 | 职业 | 个人简介 |
|------|------|------|---------|
| 张三 | 25 | 程序员 | 这是一段较长的个人简介，用于测试表格中长文本的显示效果 |
| 李四 | 30 | 设计师 | 另一段长文本，用于测试表格中的文本换行和对齐方式 |
| 王五 | 28 | 产品经理 | 第三段较长的描述文本，测试表格单元格中的长文本显示 |

## 引用块测试

> 这是一段引用文本。这是一段比较长的引用文本，用于测试引用块中的长文本显示效果。引用块通常用于展示别人说的话或者特别需要强调的内容。
>
> 这是引用块的第二段文本，测试多段落引用的显示效果。引用块应该有明显的视觉区分，通常在左侧有一条竖线。

## 混合内容测试

下面是混合了多种格式的内容，包括**粗体**、*斜体*、\`代码\`、[链接](https://example.com)等：

混合格式的长文本段落，包含**粗体文字**和*斜体文字*以及\`代码片段\`和[链接文本](https://example.com)，测试这些不同格式在一段长文本中的显示效果。特别需要关注的是不同格式文本之间的过渡是否自然，以及整体的可读性是否良好。

---

### 最终测试

这是最后一段测试文本，用于验证图片生成功能是否正常工作，特别是对于长文本的处理能力。现在我们使用了灰色背景和更大的字体，希望能够提升整体的阅读体验。
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