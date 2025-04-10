#!/bin/bash

source .env

echo "测试FastGPT API连接..."
echo "使用的API URL: $FASTGPT_API_URL"
echo "使用的令牌: ${FASTGPT_AUTH_TOKEN:0:10}..."

# 构建JSON请求体 - 去掉显式的stream和detail设置，使用API默认值
json_data=$(cat << EOF
{
  "chatId": "test_$(date +%s)",
  "responseChatItemId": "resp_$(date +%s)",
  "variables": {
    "uid": "test_user_$(date +%s)",
    "name": "测试用户"
  },
  "messages": [
    {
      "role": "user",
      "content": "请简单回复一句：测试成功"
    }
  ]
}
EOF
)

# 发送请求
response=$(curl -s -X POST "$FASTGPT_API_URL" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $FASTGPT_AUTH_TOKEN" \
  -d "$json_data")

echo ""
echo "API 响应:"
echo "$response" | jq .

# 检查是否成功
content=$(echo "$response" | jq -r '.choices[0].message.content')
if [ -n "$content" ]; then
  echo ""
  echo "测试结果: 成功 ✅"
  echo "回复内容: $content"
else
  echo ""
  echo "测试结果: 失败 ❌"
  echo "没有收到有效回复内容"
fi 