#!/bin/bash
# Example script that reads HTTP request from stdin and outputs HTTP response
# This demonstrates the proxy_request_stdin feature with HTTP response parsing

# Read the entire HTTP request from stdin
REQUEST=$(cat)

# Extract some information from the request
METHOD=$(echo "$REQUEST" | head -1 | awk '{print $1}')
PATH=$(echo "$REQUEST" | head -1 | awk '{print $2}')

# Check for process metadata headers
UID=$(echo "$REQUEST" | grep -i "^X-Forwarded-Uid:" | cut -d: -f2 | tr -d ' ')
USERNAME=$(echo "$REQUEST" | grep -i "^X-Forwarded-Username:" | cut -d: -f2 | tr -d ' ')
PID=$(echo "$REQUEST" | grep -i "^X-Forwarded-Pid:" | cut -d: -f2 | tr -d ' ')
PROCESS=$(echo "$REQUEST" | grep -i "^X-Forwarded-Process-Name:" | cut -d: -f2 | tr -d ' ')

# Generate HTTP response with custom status code and headers
cat <<EOF
HTTP/1.1 200 OK
Content-Type: application/json
X-Custom-Header: ProcessAware

{
  "method": "$METHOD",
  "path": "$PATH",
  "process_metadata": {
    "uid": "$UID",
    "username": "$USERNAME",
    "pid": "$PID",
    "executable": "$PROCESS"
  },
  "message": "Request processed successfully"
}
EOF
