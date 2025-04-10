# Rust Discord AI机器人

基于Rust实现的高性能Discord机器人，用于回答问题并生成图片形式的回答。

## 功能特点

- 通过斜线命令向AI提问，获取图片形式的回答
- 支持提供图片链接，AI可分析图片内容
- 保存历史会话，方便查询过去的问答记录
- 自动清理旧图片文件，节省存储空间
- 支持Windows和Linux/WSL环境

## 系统要求

- Rust 1.56+
- wkhtmltopdf / wkhtmltoimage (用于图片生成)
- Discord机器人令牌
- FastGPT API 访问令牌

## 安装依赖

### Windows

1. 安装Rust: https://www.rust-lang.org/tools/install
2. 安装wkhtmltopdf: https://wkhtmltopdf.org/downloads.html
3. 下载中文字体: LXGWWenKaiGBScreen.ttf

### Linux/WSL

```bash
# 安装Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 安装wkhtmltopdf
sudo apt-get update
sudo apt-get install -y wkhtmltopdf
```

## 配置

1. 复制`.env.example`为`.env`
2. 编辑`.env`文件，填入以下配置:
   - Discord机器人令牌
   - FastGPT API URL和令牌
   - 字体路径和其他配置项

配置示例:

```
# Discord配置
DISCORD_TOKEN=your_discord_token_here
DISCORD_CHANNEL_WHITELIST=  # 频道ID列表，使用逗号分隔，留空允许所有频道

# FastGPT配置
FASTGPT_API_URL=https://fastgpt.example.com/api/v1/chat/completions
FASTGPT_AUTH_TOKEN=your_fastgpt_token_here

# 图片生成配置
FONT_PATHS=./LXGWWenKaiGBScreen.ttf  # 字体路径，多个路径使用逗号分隔
FONT_SIZE=20  # 字体大小
PADDING=30  # 内边距

# 会话配置
SESSION_EXPIRY=3600  # 会话过期时间（秒）
```

## FastGPT API格式

本项目支持最新的FastGPT API请求格式：

```json
{
  "chatId": "my_chatId",
  "stream": false,
  "detail": false,
  "responseChatItemId": "my_responseChatItemId",
  "variables": {
    "uid": "user_id",
    "name": "用户名"
  },
  "messages": [
    {
      "role": "user",
      "content": "用户问题"
    }
  ]
}
```

FastGPT响应格式：

```json
{
  "id": "response_id",
  "model": "",
  "usage": {
    "prompt_tokens": 1,
    "completion_tokens": 1,
    "total_tokens": 1
  },
  "choices": [
    {
      "message": {
        "role": "assistant",
        "content": "AI回复内容"
      },
      "finish_reason": "stop",
      "index": 0
    }
  ]
}
```

## 编译和运行

### Windows

```bash
# 编译
cargo build --release

# 运行
.\run.bat
```

或直接双击`run.bat`文件。

### Linux/WSL

```bash
# 设置执行权限
chmod +x run.sh

# 编译和运行
./run.sh
```

### WSL特定启动脚本

如果你使用WSL，可以使用专门的启动脚本:

```bash
chmod +x wsl_run.sh
./wsl_run.sh
```

## Discord斜线命令

机器人提供以下斜线命令:

- `/答疑bot [问题] [图片url]` - 向AI提问并获取图片形式的回答
- `/历史会话` - 查看你的历史会话列表
- `/帮助` - 获取机器人使用指南
- `/存储统计 [详细信息]` - 查看会话存储状态和统计信息

## 项目结构

```
src/
├── api/            # API客户端模块
│   ├── mod.rs
│   └── models.rs
├── config/         # 配置处理模块
│   └── mod.rs
├── discord/        # Discord机器人模块
│   ├── mod.rs
│   └── commands.rs
├── image/          # 图像生成模块
│   └── mod.rs
├── session/        # 会话管理模块
│   └── mod.rs
└── main.rs         # 主程序入口

assets/
├── fonts/          # 字体文件
│   └── LXGWWenKaiGBScreen.ttf

scripts/
├── run.sh          # Linux运行脚本
├── run.bat         # Windows运行脚本
├── run_debug.sh    # 调试模式运行脚本
├── wsl_run.sh      # WSL运行脚本
└── tests/          # 测试脚本
    ├── test_discord.sh
    ├── test_fastgpt.sh
    ├── test_image_gen.sh
    └── test_long_text.sh

docker/             # Docker相关文件
├── Dockerfile
└── docker_run.sh

data/               # 数据目录
├── logs/           # 日志文件
├── pic/            # 图片文件
│   └── temp/       # 临时图片文件
└── sessions/       # 会话数据
    ├── [session_id]/  # 每个会话的目录
    │   ├── input.txt        # 用户输入
    │   ├── response.md      # AI响应的Markdown
    │   ├── response_*.png   # 生成的图片
    │   └── user_id.txt      # 用户ID
    └── ...
```

## 存储结构

数据存储在`data`目录下:

```
data/
├── logs/           # 日志文件
├── pic/            # 图片文件
│   └── temp/       # 临时图片文件
└── sessions/       # 会话数据
    ├── [session_id]/  # 每个会话的目录
    │   ├── input.txt        # 用户输入
    │   ├── response.md      # AI响应的Markdown
    │   ├── response_*.png   # 生成的图片
    │   └── user_id.txt      # 用户ID
    └── ...
```

## 许可证

本项目采用 [知识共享署名-非商业性使用-禁止演绎 4.0 国际许可协议（CC BY-NC-ND 4.0）](https://creativecommons.org/licenses/by-nc-nd/4.0/deed.zh) 进行许可。

这意味着您可以：
- **分享**：在任何媒介或格式中复制、发行本作品
- **署名**：必须给出适当的署名，提供指向本许可证的链接，同时标明是否对原始作品作了修改

但必须遵循以下限制：
- **非商业性使用**：不得将本作品用于商业目的
- **禁止演绎**：如果您再混合、转换、或者基于本作品进行创作，您不能发布修改后的作品
- **不得增加额外限制**：您不能适用法律术语或者技术措施从而限制其他人做许可证允许的事情

## 贡献

欢迎提交问题报告和功能建议。 