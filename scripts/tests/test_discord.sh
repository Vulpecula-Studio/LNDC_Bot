#!/bin/bash

source .env

echo "测试Discord API连接..."
echo "使用的Token前10个字符: ${DISCORD_TOKEN:0:10}..."

# 测试获取当前用户信息
response=$(curl -s -X GET "https://discord.com/api/v10/users/@me" \
  -H "Authorization: Bot $DISCORD_TOKEN")

echo ""
echo "API 响应:"
echo "$response" | jq .

# 检查是否成功
username=$(echo "$response" | jq -r '.username')
error_code=$(echo "$response" | jq -r '.code')
error_message=$(echo "$response" | jq -r '.message')

if [ -n "$username" ] && [ "$username" != "null" ]; then
  echo ""
  echo "测试结果: 成功 ✅"
  echo "机器人名称: $username"
  echo "机器人ID: $(echo "$response" | jq -r '.id')"
  
  # 获取机器人所在的服务器列表
  echo ""
  echo "正在获取服务器列表..."
  guilds=$(curl -s -X GET "https://discord.com/api/v10/users/@me/guilds" \
    -H "Authorization: Bot $DISCORD_TOKEN")
  
  guild_count=$(echo "$guilds" | jq '. | length')
  echo "机器人所在的服务器数量: $guild_count"
  
  if [ "$guild_count" -gt 0 ]; then
    echo "服务器列表:"
    echo "$guilds" | jq -r '.[] | "  - \(.name) (ID: \(.id))"'
  fi
  
else
  echo ""
  echo "测试结果: 失败 ❌"
  if [ -n "$error_code" ] && [ "$error_code" != "null" ]; then
    echo "错误代码: $error_code"
    echo "错误信息: $error_message"
  else
    echo "没有收到有效回复内容"
  fi
fi 