#!/bin/bash
# 一键清除端口占用脚本
# 适用于 Termux/Android 环境

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# 默认端口列表：后端 8080, 前端 HTTPS 8889, 前端 HTTP 5173
DEFAULT_PORTS=(8080 8889 5173)

# 获取占用端口的 PID
get_port_pid() {
    local port=$1
    # Termux 下使用 netstat 或 ss
    if command -v netstat &> /dev/null; then
        netstat -tlnp 2>/dev/null | grep ":$port " | awk '{print $7}' | cut -d'/' -f1 | head -1
    elif command -v ss &> /dev/null; then
        ss -tlnp 2>/dev/null | grep ":$port " | grep -oP 'pid=\K[0-9]+' | head -1
    elif command -v fuser &> /dev/null; then
        fuser "$port/tcp" 2>/dev/null | awk '{print $1}'
    fi
}

# 根据 PID 获取进程名
get_process_name() {
    local pid=$1
    if [ -n "$pid" ]; then
        ps -p "$pid" -o comm= 2>/dev/null || echo "unknown"
    fi
}

# 终止进程
kill_process() {
    local pid=$1
    local port=$2
    local name=$(get_process_name "$pid")
    
    echo -e "${YELLOW}发现进程占用端口 $port:${NC}"
    echo "  PID: $pid"
    echo "  进程: $name"
    
    # 先尝试正常终止
    kill "$pid" 2>/dev/null
    
    # 等待进程结束
    local count=0
    while [ $count -lt 10 ]; do
        if ! ps -p "$pid" > /dev/null 2>&1; then
            echo -e "${GREEN}✓ 端口 $port 已释放${NC}"
            return 0
        fi
        sleep 0.5
        count=$((count + 1))
    done
    
    # 强制终止
    echo -e "${YELLOW}进程未响应，强制终止...${NC}"
    kill -9 "$pid" 2>/dev/null
    sleep 0.5
    
    if ! ps -p "$pid" > /dev/null 2>&1; then
        echo -e "${GREEN}✓ 端口 $PORT 已释放（强制）${NC}"
        return 0
    else
        echo -e "${RED}✗ 无法终止进程 $pid${NC}"
        return 1
    fi
}

# 检查并清除单个端口
clear_port() {
    local port=$1
    echo -e "检查端口 ${YELLOW}$port${NC} ..."
    
    local pid=$(get_port_pid "$port")
    
    if [ -n "$pid" ] && [ "$pid" -gt 0 ] 2>/dev/null; then
        kill_process "$pid" "$port"
    else
        echo -e "  ${GREEN}✓ 端口空闲${NC}"
    fi
}

# 主逻辑
main() {
    echo "=========================================="
    echo "       端口清理脚本 (Termux 版)"
    echo "=========================================="
    echo ""
    
    local ports=("$@")
    
    # 如果没有参数，使用默认端口
    if [ ${#ports[@]} -eq 0 ]; then
        ports=("${DEFAULT_PORTS[@]}")
        echo "未指定端口，检查默认端口: ${DEFAULT_PORTS[*]}"
        echo ""
    fi
    
    local cleared=0
    local failed=0
    
    for port in "${ports[@]}"; do
        if clear_port "$port"; then
            cleared=$((cleared + 1))
        else
            failed=$((failed + 1))
        fi
        echo ""
    done
    
    echo "=========================================="
    echo -e "完成: ${GREEN}释放 $cleared 个${NC}, ${RED}失败 $failed 个${NC}"
    echo "=========================================="
}

# 运行
main "$@"