#!/bin/bash

echo "测试Docker镜像..."

# 检查镜像是否存在
if ! docker image inspect rust_discord_bot:test &>/dev/null; then
  echo "错误：镜像rust_discord_bot:test不存在，请先构建镜像"
  exit 1
fi

# 创建测试容器并运行简单测试
echo "创建测试容器..."
CONTAINER_ID=$(docker create --name rust_bot_test rust_discord_bot:test sh -c "ls -la && ldd ./rust_discord_bot")

# 启动容器
echo "启动测试容器..."
docker start -a $CONTAINER_ID

# 检查退出状态
EXIT_CODE=$?
if [ $EXIT_CODE -ne 0 ]; then
  echo "错误：测试失败，退出代码: $EXIT_CODE"
  docker rm $CONTAINER_ID
  exit $EXIT_CODE
fi

# 清理
echo "清理测试容器..."
docker rm $CONTAINER_ID

echo "测试成功完成！" 