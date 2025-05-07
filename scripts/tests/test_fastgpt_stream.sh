#!/bin/bash

source .env

echo "测试FastGPT流式API连接..."
echo "使用的API URL: $FASTGPT_API_URL"
echo "使用的令牌: ${FASTGPT_AUTH_TOKEN:0:10}..."

declare -A body=(
  [chatId]="stream_test_$(date +%s)"
  [responseChatItemId]="stream_resp_$(date +%s)"
)
# 构建JSON请求体，开启流式stream:true
json_body=$(cat <<EOF
{
  "stream": true,
  "detail": true,
  "messages": [{"role": "user", "content": "请流式回复一句：测试流式成功"}]
}
EOF
)

echo "请求体：$json_body"

echo "开始接收流式响应："
curl -s -N -X POST "$FASTGPT_API_URL" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $FASTGPT_AUTH_TOKEN" \
  -d "$json_body" |
# 解析并展示event和data
while IFS= read -r line; do
  if [[ $line == event:* ]]; then
    event="${line#event: }"
    echo "收到事件: $event"
  elif [[ $line == data:* ]]; then
    payload="${line#data: }"
    if [[ "$payload" == "[DONE]" ]]; then
      echo "===== 流式结束 ====="
      break
    fi
    echo "接收到数据块 (event: ${event:-message}): $payload"
  fi
done