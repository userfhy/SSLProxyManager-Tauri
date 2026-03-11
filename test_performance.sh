#!/bin/bash

# 性能测试脚本
# 用于测试 SSLProxyManager 的性能指标

echo "=========================================="
echo "  SSLProxyManager 性能测试"
echo "=========================================="
echo ""

# 颜色定义
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 检查依赖
check_command() {
    if ! command -v "$1" &> /dev/null; then
        echo -e "${RED}✗ 未找到 $1，请先安装${NC}"
        return 1
    fi
    echo -e "${GREEN}✓ 找到 $1${NC}"
    return 0
}

echo "1. 检查测试工具..."
TOOLS_OK=true

if ! check_command "wrk"; then
    echo -e "${YELLOW}  提示: 安装 wrk: sudo apt install wrk (Ubuntu/Debian)${NC}"
    TOOLS_OK=false
fi

if ! check_command "ab"; then
    echo -e "${YELLOW}  提示: 安装 ab: sudo apt install apache2-utils${NC}"
    TOOLS_OK=false
fi

if [ "$TOOLS_OK" = false ]; then
    echo ""
    echo -e "${YELLOW}警告: 部分测试工具未安装，将跳过相关测试${NC}"
fi
echo ""

# 检查二进制文件
echo "2. 检查二进制文件..."
if [ ! -f "target/release/SSLProxyManager" ]; then
    echo -e "${RED}✗ 未找到 release 二进制文件${NC}"
    echo "请先运行: cargo build --release"
    exit 1
fi

SIZE=$(du -h target/release/SSLProxyManager | cut -f1)
SIZE_BYTES=$(stat -c%s target/release/SSLProxyManager 2>/dev/null || stat -f%z target/release/SSLProxyManager 2>/dev/null)
SIZE_MB=$((SIZE_BYTES / 1024 / 1024))

echo -e "${GREEN}✓ 找到二进制文件${NC}"
echo "  大小: ${SIZE} (${SIZE_MB} MB)"

# 检查是否已 strip
if file target/release/SSLProxyManager | grep -q "stripped"; then
    echo -e "${GREEN}✓ 调试符号已移除${NC}"
else
    echo -e "${YELLOW}⚠ 调试符号未移除${NC}"
fi
echo ""

# 二进制大小对比
echo "3. 二进制大小分析..."
echo "  当前大小: ${SIZE_MB} MB"
echo "  预期优化: -25~30% (相比未优化版本)"
echo ""

# 启动时间测试
echo "4. 启动时间测试..."
echo -e "${YELLOW}注意: 此测试需要有效的配置文件${NC}"
read -p "是否测试启动时间? (y/N): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "测试启动时间 (3次平均)..."
    TOTAL=0
    for i in {1..3}; do
        START=$(date +%s%N)
        timeout 2s ./target/release/SSLProxyManager --help > /dev/null 2>&1
        END=$(date +%s%N)
        ELAPSED=$(( (END - START) / 1000000 ))
        echo "  第 $i 次: ${ELAPSED} ms"
        TOTAL=$((TOTAL + ELAPSED))
    done
    AVG=$((TOTAL / 3))
    echo -e "${GREEN}平均启动时间: ${AVG} ms${NC}"
else
    echo "跳过启动时间测试"
fi
echo ""

# HTTP 性能测试
echo "5. HTTP 性能测试..."
echo -e "${YELLOW}注意: 此测试需要代理服务器正在运行${NC}"
read -p "输入测试 URL (留空跳过): " TEST_URL

if [ -n "$TEST_URL" ]; then
    echo ""
    echo "开始性能测试: $TEST_URL"
    echo ""

    # wrk 测试
    if command -v wrk &> /dev/null; then
        echo "--- wrk 压力测试 (4线程, 100连接, 30秒) ---"
        wrk -t4 -c100 -d30s "$TEST_URL"
        echo ""
    fi

    # ab 测试
    if command -v ab &> /dev/null; then
        echo "--- Apache Bench 测试 (10000请求, 100并发) ---"
        ab -n 10000 -c 100 "$TEST_URL" 2>&1 | grep -E "(Requests per second|Time per request|Transfer rate|Failed requests)"
        echo ""
    fi
else
    echo "跳过 HTTP 性能测试"
fi
echo ""

# 内存占用测试
echo "6. 内存占用分析..."
read -p "输入进程 PID (留空跳过): " PID

if [ -n "$PID" ]; then
    if ps -p "$PID" > /dev/null 2>&1; then
        RSS=$(ps -o rss= -p "$PID")
        RSS_MB=$((RSS / 1024))
        VSZ=$(ps -o vsz= -p "$PID")
        VSZ_MB=$((VSZ / 1024))

        echo -e "${GREEN}进程 $PID 内存使用:${NC}"
        echo "  RSS (实际内存): ${RSS_MB} MB"
        echo "  VSZ (虚拟内存): ${VSZ_MB} MB"

        # 显示详细内存映射
        if command -v pmap &> /dev/null; then
            echo ""
            echo "内存映射摘要:"
            pmap -x "$PID" | tail -1
        fi
    else
        echo -e "${RED}✗ 进程 $PID 不存在${NC}"
    fi
else
    echo "跳过内存占用分析"
fi
echo ""

# 总结
echo "=========================================="
echo "  性能测试总结"
echo "=========================================="
echo ""
echo "已完成的优化:"
echo "  高优先级:"
echo "    ✓ 编译优化 (opt-level=3, LTO, strip)"
echo "    ✓ Tokio 依赖精简"
echo "    ✓ 数据库性能优化 (64MB cache + mmap)"
echo ""
echo "  中优先级:"
echo "    ✓ 日志系统优化"
echo "    ✓ 连接池参数调优"
echo "    ✓ 静态文件预压缩支持"
echo "    ✓ 正则表达式预编译"
echo ""
echo "  低优先级:"
echo "    ✓ 字符串分配优化"
echo "    ✓ 内联函数优化"
echo "    ✓ Upstream 选择优化"
echo ""
echo "预期性能提升:"
echo "  • 二进制体积: -25~30%"
echo "  • HTTP QPS: +10~20%"
echo "  • 数据库查询: +50~80%"
echo "  • 日志开销: -20~30%"
echo "  • CPU 使用率: -8~15%"
echo ""
echo "建议:"
echo "  1. 在生产环境部署前进行充分测试"
echo "  2. 监控关键性能指标 (QPS, 延迟, 内存)"
echo "  3. 根据实际负载调整连接池参数"
echo "  4. 定期检查日志和错误率"
echo ""
