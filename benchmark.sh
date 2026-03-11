#!/bin/bash
# 性能对比测试脚本
# 用于测试优化前后的性能差异

set -e

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${BLUE}ℹ${NC} $1"; }
log_success() { echo -e "${GREEN}✅${NC} $1"; }
log_warning() { echo -e "${YELLOW}⚠️${NC} $1"; }
log_error() { echo -e "${RED}❌${NC} $1"; }

# 配置
TEST_URL="${TEST_URL:-https://localhost:8443}"
DURATION="${DURATION:-30}"
THREADS="${THREADS:-4}"
CONNECTIONS="${CONNECTIONS:-100}"
RESULTS_DIR="./benchmark-results"

echo "🚀 性能测试脚本"
echo "==============================================="
echo "测试 URL: $TEST_URL"
echo "持续时间: ${DURATION}s"
echo "线程数: $THREADS"
echo "并发连接: $CONNECTIONS"
echo "==============================================="
echo ""

# 检查依赖
check_dependencies() {
    local missing=0

    if ! command -v wrk &> /dev/null; then
        log_warning "未找到 wrk，尝试安装..."
        if command -v apt &> /dev/null; then
            sudo apt install -y wrk
        else
            log_error "请手动安装 wrk: https://github.com/wg/wrk"
            missing=1
        fi
    fi

    if ! command -v ab &> /dev/null; then
        log_warning "未找到 ab (Apache Bench)"
        log_info "可选安装: sudo apt install apache2-utils"
    fi

    if ! command -v h2load &> /dev/null; then
        log_warning "未找到 h2load (HTTP/2 测试工具)"
        log_info "可选安装: sudo apt install nghttp2-client"
    fi

    return $missing
}

check_dependencies || exit 1

# 创建结果目录
mkdir -p "$RESULTS_DIR"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
RESULT_FILE="$RESULTS_DIR/benchmark_$TIMESTAMP.txt"

log_info "结果将保存到: $RESULT_FILE"
echo ""

# 测试函数
run_wrk_test() {
    local name=$1
    local url=$2

    log_info "运行 wrk 测试: $name"
    echo "=== $name ===" >> "$RESULT_FILE"
    echo "测试时间: $(date)" >> "$RESULT_FILE"
    echo "" >> "$RESULT_FILE"

    wrk -t"$THREADS" -c"$CONNECTIONS" -d"${DURATION}s" --latency "$url" 2>&1 | tee -a "$RESULT_FILE"
    echo "" >> "$RESULT_FILE"
    echo "---" >> "$RESULT_FILE"
    echo "" >> "$RESULT_FILE"
}

run_ab_test() {
    local name=$1
    local url=$2
    local requests=$((CONNECTIONS * 100))

    if ! command -v ab &> /dev/null; then
        return
    fi

    log_info "运行 Apache Bench 测试: $name"
    echo "=== Apache Bench: $name ===" >> "$RESULT_FILE"
    echo "测试时间: $(date)" >> "$RESULT_FILE"
    echo "" >> "$RESULT_FILE"

    ab -n "$requests" -c "$CONNECTIONS" -k "$url" 2>&1 | tee -a "$RESULT_FILE"
    echo "" >> "$RESULT_FILE"
    echo "---" >> "$RESULT_FILE"
    echo "" >> "$RESULT_FILE"
}

run_h2load_test() {
    local name=$1
    local url=$2
    local requests=$((CONNECTIONS * 100))

    if ! command -v h2load &> /dev/null; then
        return
    fi

    log_info "运行 h2load 测试 (HTTP/2): $name"
    echo "=== h2load (HTTP/2): $name ===" >> "$RESULT_FILE"
    echo "测试时间: $(date)" >> "$RESULT_FILE"
    echo "" >> "$RESULT_FILE"

    h2load -n "$requests" -c "$CONNECTIONS" -m 10 "$url" 2>&1 | tee -a "$RESULT_FILE"
    echo "" >> "$RESULT_FILE"
    echo "---" >> "$RESULT_FILE"
    echo "" >> "$RESULT_FILE"
}

# 系统信息
log_info "收集系统信息..."
echo "=== 系统信息 ===" >> "$RESULT_FILE"
echo "主机名: $(hostname)" >> "$RESULT_FILE"
echo "内核: $(uname -r)" >> "$RESULT_FILE"
echo "CPU: $(lscpu | grep 'Model name' | cut -d: -f2 | xargs)" >> "$RESULT_FILE"
echo "CPU 核心数: $(nproc)" >> "$RESULT_FILE"
echo "内存: $(free -h | grep Mem | awk '{print $2}')" >> "$RESULT_FILE"
echo "测试时间: $(date)" >> "$RESULT_FILE"
echo "" >> "$RESULT_FILE"
echo "---" >> "$RESULT_FILE"
echo "" >> "$RESULT_FILE"

# 编译信息
if [ -f "Cargo.toml" ]; then
    log_info "收集编译信息..."
    echo "=== 编译配置 ===" >> "$RESULT_FILE"
    echo "Rust 版本: $(rustc --version)" >> "$RESULT_FILE"
    echo "" >> "$RESULT_FILE"
    echo "Cargo.toml [profile.release]:" >> "$RESULT_FILE"
    sed -n '/\[profile.release\]/,/^$/p' Cargo.toml >> "$RESULT_FILE"
    echo "" >> "$RESULT_FILE"

    if [ -f ".cargo/config.toml" ]; then
        echo ".cargo/config.toml:" >> "$RESULT_FILE"
        cat .cargo/config.toml >> "$RESULT_FILE"
        echo "" >> "$RESULT_FILE"
    fi

    echo "---" >> "$RESULT_FILE"
    echo "" >> "$RESULT_FILE"
fi

# 运行测试
log_success "开始性能测试..."
echo ""

# 1. 基础 HTTP 测试
run_wrk_test "HTTP 吞吐量测试" "$TEST_URL"

# 2. Apache Bench 测试
run_ab_test "Apache Bench 测试" "$TEST_URL"

# 3. HTTP/2 测试
run_h2load_test "HTTP/2 性能测试" "$TEST_URL"

# 生成摘要
log_info "生成测试摘要..."
echo ""
echo "=== 测试摘要 ===" >> "$RESULT_FILE"

# 提取 wrk 关键指标
if grep -q "Requests/sec" "$RESULT_FILE"; then
    echo "wrk 测试结果:" >> "$RESULT_FILE"
    grep "Requests/sec" "$RESULT_FILE" | head -1 >> "$RESULT_FILE"
    grep "Latency" "$RESULT_FILE" | head -1 >> "$RESULT_FILE"
    grep "Transfer/sec" "$RESULT_FILE" | head -1 >> "$RESULT_FILE"
    echo "" >> "$RESULT_FILE"
fi

# 提取 ab 关键指标
if grep -q "Requests per second" "$RESULT_FILE"; then
    echo "Apache Bench 测试结果:" >> "$RESULT_FILE"
    grep "Requests per second" "$RESULT_FILE" | head -1 >> "$RESULT_FILE"
    grep "Time per request" "$RESULT_FILE" | head -2 >> "$RESULT_FILE"
    echo "" >> "$RESULT_FILE"
fi

# 提取 h2load 关键指标
if grep -q "finished in" "$RESULT_FILE"; then
    echo "h2load (HTTP/2) 测试结果:" >> "$RESULT_FILE"
    grep "finished in" "$RESULT_FILE" | head -1 >> "$RESULT_FILE"
    grep "requests:" "$RESULT_FILE" | head -1 >> "$RESULT_FILE"
    echo "" >> "$RESULT_FILE"
fi

echo "---" >> "$RESULT_FILE"

log_success "测试完成！"
echo ""
log_info "详细结果: $RESULT_FILE"
echo ""

# 显示摘要
if [ -f "$RESULT_FILE" ]; then
    echo "📊 测试摘要:"
    echo "==============================================="
    tail -20 "$RESULT_FILE"
    echo "==============================================="
fi

echo ""
log_info "提示:"
echo "   - 多次运行测试以获得稳定结果"
echo "   - 对比优化前后的结果文件"
echo "   - 关注 QPS、延迟、吞吐量等关键指标"
echo ""

# 性能分析建议
log_info "性能分析建议:"
echo "   1. CPU 分析: perf record -g ./target/release/SSLProxyManager"
echo "   2. 内存分析: valgrind --tool=massif ./target/release/SSLProxyManager"
echo "   3. 火焰图: cargo flamegraph --bin SSLProxyManager"
echo "   4. 编译时间: cargo clean && time cargo build --release"
echo ""

log_success "🎉 所有测试完成！"
