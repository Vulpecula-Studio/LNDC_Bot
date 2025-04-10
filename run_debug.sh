#!/bin/bash

# 设置颜色
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # 无颜色

# 检查是否已经有实例在运行
RUNNING_INSTANCES=$(ps aux | grep "rust_discord_bot" | grep -v grep | grep -v "run_debug.sh" | wc -l)
if [ $RUNNING_INSTANCES -gt 0 ]; then
    echo -e "${RED}错误: 已经有Discord机器人实例在运行${NC}"
    echo -e "${YELLOW}请先终止已有实例再运行新的实例${NC}"
    ps aux | grep "rust_discord_bot" | grep -v grep | grep -v "run_debug.sh"
    echo -e "${YELLOW}可使用 kill -9 PID 命令终止进程${NC}"
    exit 1
fi

# 清理临时文件
echo -e "${GREEN}清理临时文件...${NC}"
find ./data -name "temp_*" -delete 2>/dev/null || true

# 设置环境变量为调试模式
export RUST_LOG=serenity=debug,poise=debug,rust_discord_bot=trace

# 创建日志目录
mkdir -p data/logs

LOG_FILE="data/logs/debug_$(date +%Y%m%d_%H%M%S).log"
echo -e "${GREEN}正在以调试模式启动机器人...${NC}"
echo -e "${YELLOW}日志级别: $RUST_LOG${NC}"
echo -e "${YELLOW}日志文件: $LOG_FILE${NC}"

# 编译并运行项目
cargo run --bin rust_discord_bot 2>&1 | tee "$LOG_FILE" 