#!/bin/bash

# 设置颜色
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # 无颜色

# 切换到脚本所在目录
cd "$(dirname "$0")"

echo -e "${GREEN}正在WSL环境中启动Discord机器人...${NC}"

# 检查wkhtmltoimage是否安装
if ! command -v wkhtmltoimage &> /dev/null; then
    echo -e "${RED}错误: wkhtmltoimage 未安装${NC}"
    echo -e "${YELLOW}请执行以下命令安装:${NC}"
    echo -e "sudo apt-get update && sudo apt-get install -y wkhtmltopdf"
    exit 1
fi

# 检查字体文件
FONT_FILE="./LXGWWenKaiGBScreen.ttf"
if [ ! -f "$FONT_FILE" ]; then
    echo -e "${YELLOW}警告: 字体文件 $FONT_FILE 不存在${NC}"
    echo -e "${YELLOW}尝试复制../LXGWWenKaiGBScreen.ttf${NC}"
    
    if [ -f "../LXGWWenKaiGBScreen.ttf" ]; then
        cp ../LXGWWenKaiGBScreen.ttf .
        echo -e "${GREEN}字体文件已复制${NC}"
    else
        echo -e "${RED}错误: 无法找到字体文件${NC}"
        echo -e "${YELLOW}请下载字体文件并放置在当前目录${NC}"
        exit 1
    fi
fi

# 为WSL环境设置正确的权限
chmod +x run.sh

# 检查是否需要编译
if [ ! -f "./target/release/rust_discord_bot" ] || [ "$1" == "--rebuild" ]; then
    echo -e "${GREEN}正在编译项目...${NC}"
    cargo build --release
    
    if [ $? -ne 0 ]; then
        echo -e "${RED}编译失败！${NC}"
        exit 1
    fi
    
    echo -e "${GREEN}编译完成${NC}"
else
    echo -e "${GREEN}使用现有的编译文件${NC}"
fi

# 创建必要的目录
mkdir -p ./data/pic/temp
mkdir -p ./data/sessions
mkdir -p ./data/logs

# 创建日志文件
LOG_FILE="./data/logs/wsl_bot_$(date +%Y%m%d_%H%M%S).log"
echo -e "${GREEN}日志将同时输出到: $LOG_FILE${NC}"

# 设置日志级别为info，使INFO级别的日志输出到终端
export RUST_LOG=info,rust_discord_bot=info,rust_discord_bot::api=debug

# 运行程序，将输出同时写入终端和日志文件
echo -e "${GREEN}启动Discord机器人...${NC}"
./target/release/rust_discord_bot | tee -a "$LOG_FILE"

# 捕获退出状态
EXIT_CODE=$?

if [ $EXIT_CODE -ne 0 ]; then
    echo -e "${RED}程序异常退出，退出码: $EXIT_CODE${NC}"
    echo -e "${YELLOW}检查日志获取更多信息${NC}"
    exit $EXIT_CODE
fi

echo -e "${GREEN}程序正常退出${NC}"
exit 0 