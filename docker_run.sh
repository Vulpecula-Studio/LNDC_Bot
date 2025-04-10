#!/bin/bash

# 设置颜色
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # 无颜色

# 镜像和容器名称
IMAGE_NAME="rust-discord-bot"
CONTAINER_NAME="rust-discord-bot"

# 确保在脚本所在目录运行
cd "$(dirname "$0")"

# 显示帮助
if [ "$1" == "-h" ] || [ "$1" == "--help" ]; then
    echo -e "${GREEN}Rust Discord Bot Docker 运行脚本${NC}"
    echo -e "用法: $0 [命令]"
    echo
    echo -e "命令:"
    echo -e "  build      构建Docker镜像"
    echo -e "  run        运行Docker容器"
    echo -e "  stop       停止Docker容器"
    echo -e "  restart    重启Docker容器"
    echo -e "  logs       查看容器日志"
    echo -e "  shell      进入容器Shell"
    echo -e "  status     查看容器状态"
    exit 0
fi

# 构建Docker镜像
build_image() {
    echo -e "${GREEN}构建Docker镜像...${NC}"
    docker build -t $IMAGE_NAME .
}

# 运行Docker容器
run_container() {
    echo -e "${GREEN}运行Docker容器...${NC}"
    
    # 检查容器是否已存在
    if docker ps -a | grep -q $CONTAINER_NAME; then
        echo -e "${YELLOW}容器已存在，正在停止和移除...${NC}"
        docker stop $CONTAINER_NAME
        docker rm $CONTAINER_NAME
    fi
    
    # 运行新容器
    docker run -d \
        --name $CONTAINER_NAME \
        -v "$(pwd)/data:/app/data" \
        -v "$(pwd)/.env:/app/.env" \
        --restart unless-stopped \
        $IMAGE_NAME
        
    # 显示容器状态
    show_status
}

# 停止Docker容器
stop_container() {
    echo -e "${GREEN}停止Docker容器...${NC}"
    docker stop $CONTAINER_NAME
}

# 重启Docker容器
restart_container() {
    echo -e "${GREEN}重启Docker容器...${NC}"
    docker restart $CONTAINER_NAME
    
    # 显示容器状态
    show_status
}

# 查看容器日志
show_logs() {
    echo -e "${GREEN}显示容器日志...${NC}"
    docker logs -f $CONTAINER_NAME
}

# 进入容器Shell
enter_shell() {
    echo -e "${GREEN}进入容器Shell...${NC}"
    docker exec -it $CONTAINER_NAME /bin/bash
}

# 显示容器状态
show_status() {
    echo -e "${GREEN}容器状态:${NC}"
    docker ps -a --filter "name=$CONTAINER_NAME" --format "表格 {{.ID}} {{.Names}} {{.Status}} {{.Ports}}"
}

# 执行命令
case "$1" in
    build)
        build_image
        ;;
    run)
        if ! docker images | grep -q $IMAGE_NAME; then
            echo -e "${YELLOW}镜像不存在，先构建镜像...${NC}"
            build_image
        fi
        run_container
        ;;
    stop)
        stop_container
        ;;
    restart)
        restart_container
        ;;
    logs)
        show_logs
        ;;
    shell)
        enter_shell
        ;;
    status)
        show_status
        ;;
    *)
        echo -e "${YELLOW}未指定命令，执行默认操作: 构建并运行${NC}"
        build_image
        run_container
        ;;
esac

exit 0 