#!/bin/bash

# 设置颜色
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # 无颜色

# 检查是否已经有实例在运行
RUNNING_INSTANCES=$(ps aux | grep "rust_discord_bot" | grep -v grep | grep -v "run.sh" | wc -l)
if [ $RUNNING_INSTANCES -gt 0 ]; then
    echo -e "${RED}错误: 已经有Discord机器人实例在运行${NC}"
    echo -e "${YELLOW}请先终止已有实例再运行新的实例${NC}"
    ps aux | grep "rust_discord_bot" | grep -v grep | grep -v "run.sh"
    echo -e "${YELLOW}可使用 kill -9 PID 命令终止进程${NC}"
    exit 1
fi

# 检查wkhtmltoimage是否安装
if ! command -v wkhtmltoimage &> /dev/null; then
    echo -e "${RED}错误: wkhtmltoimage 未安装${NC}"
    echo -e "${YELLOW}请执行以下命令安装:${NC}"
    echo -e "sudo apt-get update && sudo apt-get install -y wkhtmltopdf"
    exit 1
fi

# 检查字体文件
FONT_FILE="./assets/fonts/LXGWWenKaiGBScreen.ttf"
if [ ! -f "$FONT_FILE" ]; then
    echo -e "${YELLOW}警告: 字体文件 $FONT_FILE 不存在${NC}"
    echo -e "${YELLOW}尝试从项目根目录复制${NC}"
    
    if [ -f "./LXGWWenKaiGBScreen.ttf" ]; then
        mkdir -p ./assets/fonts
        cp ./LXGWWenKaiGBScreen.ttf ./assets/fonts/
        echo -e "${GREEN}字体文件已复制到assets/fonts/目录${NC}"
    else
        echo -e "${RED}错误: 无法找到字体文件${NC}"
        echo -e "${YELLOW}请下载字体文件并放置在assets/fonts目录${NC}"
        exit 1
    fi
fi

# 清理临时文件
echo -e "${GREEN}清理临时文件...${NC}"
find ./data -name "temp_*" -delete

# 检查是否需要编译
if [ ! -f "./target/release/rust_discord_bot" ] || [ "$1" == "--rebuild" ]; then
    echo -e "${GREEN}正在编译项目...${NC}"
    # 明确指定二进制文件
    cargo build --release --bin rust_discord_bot
    
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
LOG_FILE="./data/logs/bot_$(date +%Y%m%d_%H%M%S).log"
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