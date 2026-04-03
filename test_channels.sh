#!/bin/bash
source .env
curl -s -H "Authorization: Bearer $SLACK_USER_TOKENS" "https://slack.com/api/conversations.list?types=public_channel,private_channel&exclude_archived=true" | jq '.channels | length, .error, .warning'
