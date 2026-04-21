#!/bin/bash
# PGO (Profile-Guided Optimization) 构建脚本
# 使用真实工作负载数据优化编译，可提升 10-30% 性能

set -e

PGO_DIR="/tmp/pgo-data"
BACKUP_DIR="./pgo-backup"
AUTO_RUN=${AUTO_RUN:-false}
BENCHMARK_TIME=${BENCHMARK_TIME:-30}

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() { echo -e "${BLUE}ℹ${NC} $1"; }
log_success() { echo -e "${GREEN}✅${NC} $1"; }
log_warning() { echo -e "${YELLOW}⚠️${NC} $1"; }
log_error() { echo -e "${RED}❌${NC} $1"; }

echo "🚀 开始 PGO 优化构建流程..."

# 检查依赖
check_dependencies() {
    local missing=0

    if ! command -v llvm-profdata &> /dev/null; then
        log_error "未找到 llvm-profdata 工具"
        echo "   请安装: sudo apt install llvm"
        missing=1
    fi

    if ! command -v npm &> /dev/null; then
        log_error "未找到 npm"
        missing=1
    fi

    if [ ! -f "package.json" ]; then
        log_error "未找到 package.json，请在项目根目录运行此脚本"
        missing=1
    fi

    return $missing
}

check_dependencies || exit 1

# 备份旧的 PGO 数据
backup_old_data() {
    if [ -d "$PGO_DIR" ] && [ -f "$PGO_DIR/merged.profdata" ]; then
        log_info "备份旧的 PGO 数据..."
        mkdir -p "$BACKUP_DIR"
        local timestamp=$(date +%Y%m%d_%H%M%S)
        cp -r "$PGO_DIR" "$BACKUP_DIR/pgo-data-$timestamp"
        log_success "备份完成: $BACKUP_DIR/pgo-data-$timestamp"
    fi
}

# 清理旧数据
cleanup_old_data() {
    log_info "清理旧的 PGO 数据..."
    rm -rf "$PGO_DIR"
    mkdir -p "$PGO_DIR"
}

# 构建 instrumented 版本
build_instrumented() {
    echo ""
    log_info "步骤 1/4: 构建 instrumented 版本..."
    local start_time=$(date +%s)

    RUSTFLAGS="-C profile-generate=$PGO_DIR" npm run tauri:build

    local end_time=$(date +%s)
    local duration=$((end_time - start_time))
    log_success "Instrumented 版本构建完成 (耗时: ${duration}s)"
}

# 收集性能数据
collect_profile_data() {
    echo ""
    log_warning "请执行以下操作收集性能数据："
    echo "   1. 运行应用程序"
    echo "   2. 执行典型工作负载（启动代理、发送请求等）"
    echo "   3. 运行至少 ${BENCHMARK_TIME} 秒以收集足够数据"
    echo "   4. 关闭程序"
    echo ""

    if [ "$AUTO_RUN" = true ]; then
        log_info "自动模式: 跳过手动数据收集"
    else
        read -p "完成数据收集后按 Enter 继续..."
    fi
}

# 合并性能数据
merge_profile_data() {
    echo ""
    log_info "步骤 2/4: 合并性能数据..."

    local profraw_count=$(find "$PGO_DIR" -name "*.profraw" 2>/dev/null | wc -l)

    if [ "$profraw_count" -eq 0 ]; then
        log_error "未找到性能数据文件"
        echo "   请确保已运行程序并执行了工作负载"
        exit 1
    fi

    log_info "找到 $profraw_count 个性能数据文件"

    local total_size=$(du -sh "$PGO_DIR" | cut -f1)
    log_info "数据总大小: $total_size"

    llvm-profdata merge -o "$PGO_DIR/merged.profdata" "$PGO_DIR"/*.profraw

    local merged_size=$(du -h "$PGO_DIR/merged.profdata" | cut -f1)
    log_success "性能数据合并完成 (合并后: $merged_size)"
}

# 使用 PGO 数据优化编译
build_optimized() {
    echo ""
    log_info "步骤 3/4: 使用 PGO 数据优化编译..."
    local start_time=$(date +%s)

    RUSTFLAGS="-C profile-use=$PGO_DIR/merged.profdata -C llvm-args=-pgo-warn-missing-function" \
        npm run tauri:build

    local end_time=$(date +%s)
    local duration=$((end_time - start_time))
    log_success "PGO 优化构建完成 (耗时: ${duration}s)"
}

# 显示构建结果
show_results() {
    echo ""
    log_info "步骤 4/4: 构建结果"

    if [ -d "src-tauri/target/release" ]; then
        echo ""
        echo "📦 二进制文件:"
        ls -lh src-tauri/target/release/ssl* 2>/dev/null || ls -lh src-tauri/target/release/ | grep -v "\.d$" | head -10
    fi

    echo ""
    log_success "🎉 PGO 优化完成！预期性能提升 10-30%"
    echo ""
    echo "💡 提示:"
    echo "   - PGO 数据: $PGO_DIR/"
    echo "   - 优化后的二进制: src-tauri/target/release/"
    echo "   - 建议进行性能对比测试"

    if [ -d "$BACKUP_DIR" ]; then
        echo "   - 历史备份: $BACKUP_DIR/"
    fi
}

# 清理函数
cleanup_on_error() {
    log_error "构建过程中发生错误"
    log_info "保留 PGO 数据以供调试: $PGO_DIR/"
    exit 1
}

trap cleanup_on_error ERR

# 主流程
backup_old_data
cleanup_old_data
build_instrumented
collect_profile_data
merge_profile_data
build_optimized
show_results
