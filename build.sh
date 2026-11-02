#!/usr/bin/env bash
# ==============================================================================
# TextCraft 本地构建与打包脚本 (Local Build & Packaging Script)
#
# 功能：
# 1. 自动检测本地系统环境（Linux / macOS / Windows）与 CPU 架构（x86_64 / arm64）
# 2. 检查 Rust/Cargo 工具链及编译依赖
# 3. 编译发布版 server 单二进制程序（与 GitHub Actions 构建对齐）
# 4. 组装发布包目录（包含二进制、scripts/ 游戏脚本、README.md）
# 5. 生成标准压缩归档（.tar.gz 或 .zip）及 SHA256 校验码至 dist/ 目录
# ==============================================================================

set -euo pipefail

# 终端彩色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m' # No Color

# 脚本根路径
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "${SCRIPT_DIR}"

# 默认配置
PROFILE="release"
CARGO_FLAG="--release"
TARGET_OVERRIDE=""
RUN_TESTS=false
CLEAN_BUILD=false
NO_ARCHIVE=false
OUT_DIR="${SCRIPT_DIR}/dist"

# 帮助说明
usage() {
    echo -e "${BOLD}TextCraft 本地一键构建与打包脚本${NC}

${BOLD}用法：${NC}
  ./build.sh [选项]

${BOLD}选项：${NC}
  ${CYAN}--release${NC}             编译 Release 优化版本 (默认)
  ${CYAN}--debug${NC}               编译 Debug 开发调试版本
  ${CYAN}--target <triple>${NC}     指定跨平台编译 Target (例如 x86_64-unknown-linux-gnu)
  ${CYAN}--test${NC}                打包前先执行全工作区测试 (cargo test --workspace)
  ${CYAN}--clean${NC}               编译前清理旧的构建缓存 (cargo clean)
  ${CYAN}--no-archive${NC}          仅生成 staging 目录，不压缩归档
  ${CYAN}--out-dir <path>${NC}      指定产出物存放目录 (默认: ./dist)
  ${CYAN}-h, --help${NC}            显示此帮助信息

${BOLD}示例：${NC}
  ./build.sh                     # 自动检测本地环境并一键打包 release
  ./build.sh --test              # 运行测试通过后打包
  ./build.sh --target x86_64-pc-windows-msvc  # 指定 Windows 目标交叉编译"
    exit 0
}

# 参数解析
while [[ $# -gt 0 ]]; do
    case "$1" in
        --release)
            PROFILE="release"
            CARGO_FLAG="--release"
            shift
            ;;
        --debug)
            PROFILE="debug"
            CARGO_FLAG=""
            shift
            ;;
        --target)
            TARGET_OVERRIDE="$2"
            shift 2
            ;;
        --test)
            RUN_TESTS=true
            shift
            ;;
        --clean)
            CLEAN_BUILD=true
            shift
            ;;
        --no-archive)
            NO_ARCHIVE=true
            shift
            ;;
        --out-dir)
            OUT_DIR="$2"
            shift 2
            ;;
        -h|--help)
            usage
            ;;
        *)
            echo -e "${RED}错误：未知参数: $1${NC}"
            usage
            ;;
    esac
done

echo -e "${BOLD}${BLUE}======================================================${NC}"
echo -e "${BOLD}${CYAN}      TextCraft 3.0 本地自动化构建与打包系统           ${NC}"
echo -e "${BOLD}${BLUE}======================================================${NC}"

# 1. 检查 Rust 工具链
echo -e "\n${BOLD}[1/5] 检查本地编译工具链...${NC}"
if ! command -v cargo &> /dev/null; then
    echo -e "${RED}错误: 未检测到 cargo 命令！${NC}"
    echo -e "请先安装 Rust 工具链: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

RUSTC_VER=$(rustc --version)
CARGO_VER=$(cargo --version)
HOST_TARGET=$(rustc -vV | grep "^host:" | awk '{print $2}')
echo -e "  ✓ Rustc: ${GREEN}${RUSTC_VER}${NC}"
echo -e "  ✓ Cargo: ${GREEN}${CARGO_VER}${NC}"
echo -e "  ✓ 宿主主机 Target: ${GREEN}${HOST_TARGET}${NC}"

# 2. 自动检测操作系统与架构
echo -e "\n${BOLD}[2/5] 检测本地操作系统与芯片架构...${NC}"
RAW_OS="$(uname -s)"
RAW_ARCH="$(uname -m)"

DETECTED_OS="unknown"
DETECTED_ARCH="unknown"
EXE_SUFFIX=""

case "${RAW_OS}" in
    Linux*)
        DETECTED_OS="linux"
        ;;
    Darwin*)
        DETECTED_OS="macos"
        ;;
    CYGWIN*|MINGW*|MSYS*|Windows_NT*)
        DETECTED_OS="windows"
        EXE_SUFFIX=".exe"
        ;;
    *)
        echo -e "${YELLOW}警告: 未知操作系统: ${RAW_OS}，默认按 linux 处理${NC}"
        DETECTED_OS="linux"
        ;;
esac

case "${RAW_ARCH}" in
    x86_64|amd64)
        DETECTED_ARCH="x86_64"
        ;;
    aarch64|arm64)
        DETECTED_ARCH="arm64"
        ;;
    *)
        DETECTED_ARCH="${RAW_ARCH}"
        ;;
esac

# 确定编译 Target 与 Artifact 名称
EFFECTIVE_TARGET="${TARGET_OVERRIDE:-$HOST_TARGET}"
if [[ -n "${TARGET_OVERRIDE}" ]]; then
    echo -e "  ✓ 指定 Target: ${CYAN}${EFFECTIVE_TARGET}${NC}"
    # 如果指定了 Windows target，加上 .exe 后缀
    if [[ "${EFFECTIVE_TARGET}" == *"windows"* ]]; then
        EXE_SUFFIX=".exe"
        DETECTED_OS="windows"
    fi
else
    echo -e "  ✓ 系统类型: ${GREEN}${DETECTED_OS}${NC} (${RAW_OS})"
    echo -e "  ✓ 芯片架构: ${GREEN}${DETECTED_ARCH}${NC} (${RAW_ARCH})"
    echo -e "  ✓ 默认目标 Target: ${GREEN}${EFFECTIVE_TARGET}${NC}"
fi

PACKAGE_NAME="textcraft-${DETECTED_OS}-${DETECTED_ARCH}"
echo -e "  ✓ 产出物前缀: ${BOLD}${CYAN}${PACKAGE_NAME}${NC}"

# 3. 执行清理与测试（如果配置）
if [ "${CLEAN_BUILD}" = true ]; then
    echo -e "\n${BOLD}[3/5] 执行 cargo clean...${NC}"
    cargo clean
fi

if [ "${RUN_TESTS}" = true ]; then
    echo -e "\n${BOLD}[3/5] 执行本地全量测试 (cargo test)...${NC}"
    cargo test
    echo -e "  ${GREEN}✓ 全部单元与集成测试通过！${NC}"
else
    echo -e "\n${BOLD}[3/5] 跳过前置测试 (可传入 --test 启用)...${NC}"
fi

# 4. 编译构建
echo -e "\n${BOLD}[4/5] 编译 server 二进制 (${PROFILE} 模式)...${NC}"
BUILD_CMD="cargo build ${CARGO_FLAG} --bin server"
if [[ -n "${TARGET_OVERRIDE}" ]]; then
    BUILD_CMD="${BUILD_CMD} --target ${TARGET_OVERRIDE}"
fi

echo -e "  执行指令: ${CYAN}${BUILD_CMD}${NC}"
${BUILD_CMD}

# 查找产出的二进制路径
if [[ -n "${TARGET_OVERRIDE}" ]]; then
    BINARY_SOURCE="${SCRIPT_DIR}/target/${TARGET_OVERRIDE}/${PROFILE}/server${EXE_SUFFIX}"
else
    BINARY_SOURCE="${SCRIPT_DIR}/target/${PROFILE}/server${EXE_SUFFIX}"
fi

if [[ ! -f "${BINARY_SOURCE}" ]]; then
    echo -e "${RED}错误: 未找到编译产出的二进制文件: ${BINARY_SOURCE}${NC}"
    exit 1
fi

BIN_SIZE=$(ls -lh "${BINARY_SOURCE}" | awk '{print $5}')
echo -e "  ${GREEN}✓ 编译成功:${NC} ${BINARY_SOURCE} (${BOLD}${BIN_SIZE}${NC})"

# 5. 组装与打包归档 (对齐 GitHub Actions)
echo -e "\n${BOLD}[5/5] 组装发布包与生成归档...${NC}"
STAGING_DIR="${OUT_DIR}/staging/${PACKAGE_NAME}"
rm -rf "${STAGING_DIR}"
mkdir -p "${STAGING_DIR}"
mkdir -p "${OUT_DIR}"

# 复制文件
echo -e "  -> 复制二进制: server${EXE_SUFFIX}"
cp "${BINARY_SOURCE}" "${STAGING_DIR}/server${EXE_SUFFIX}"
chmod +x "${STAGING_DIR}/server${EXE_SUFFIX}" || true

echo -e "  -> 复制 Scheme 脚本库: scripts/"
cp -r "${SCRIPT_DIR}/scripts" "${STAGING_DIR}/"

echo -e "  -> 复制开发者文档: docs/develop/"
mkdir -p "${STAGING_DIR}/docs"
cp -r "${SCRIPT_DIR}/docs/develop" "${STAGING_DIR}/docs/"

echo -e "  -> 复制说明文档: README.md"
cp "${SCRIPT_DIR}/README.md" "${STAGING_DIR}/"

if [ "${NO_ARCHIVE}" = true ]; then
    echo -e "\n${GREEN}✓ 完成！已生成未压缩的发布目录:${NC} ${STAGING_DIR}"
    ls -lh "${STAGING_DIR}"
    exit 0
fi

# 压缩归档
ARCHIVE_EXT="tar.gz"
if [[ "${DETECTED_OS}" == "windows" ]]; then
    ARCHIVE_EXT="zip"
fi
ARCHIVE_FILE="${OUT_DIR}/${PACKAGE_NAME}.${ARCHIVE_EXT}"

rm -f "${ARCHIVE_FILE}"
echo -e "  -> 正在打包为: ${BOLD}${CYAN}${ARCHIVE_FILE}${NC}..."

if [[ "${ARCHIVE_EXT}" == "tar.gz" ]]; then
    tar -czf "${ARCHIVE_FILE}" -C "${OUT_DIR}/staging" "${PACKAGE_NAME}"
elif [[ "${ARCHIVE_EXT}" == "zip" ]]; then
    if command -v 7z &> /dev/null; then
        (cd "${OUT_DIR}/staging" && 7z a -tzip "${ARCHIVE_FILE}" "${PACKAGE_NAME}" > /dev/null)
    elif command -v zip &> /dev/null; then
        (cd "${OUT_DIR}/staging" && zip -rq "${ARCHIVE_FILE}" "${PACKAGE_NAME}")
    else
        echo -e "${YELLOW}警告: 未找到 7z 或 zip 工具，改用 tar.gz 进行打包${NC}"
        ARCHIVE_FILE="${OUT_DIR}/${PACKAGE_NAME}.tar.gz"
        tar -czf "${ARCHIVE_FILE}" -C "${OUT_DIR}/staging" "${PACKAGE_NAME}"
    fi
fi

# 计算 SHA256 校验和
SHA256_VAL=""
if command -v sha256sum &> /dev/null; then
    SHA256_VAL=$(sha256sum "${ARCHIVE_FILE}" | awk '{print $1}')
elif command -v shasum &> /dev/null; then
    SHA256_VAL=$(shasum -a 256 "${ARCHIVE_FILE}" | awk '{print $1}')
fi

ARCHIVE_SIZE=$(ls -lh "${ARCHIVE_FILE}" | awk '{print $5}')

echo -e "\n${BOLD}${GREEN}======================================================${NC}"
echo -e "${BOLD}${GREEN}               构建与打包成功！🎉                     ${NC}"
echo -e "${BOLD}${GREEN}======================================================${NC}"
echo -e "  产出归档: ${BOLD}${CYAN}${ARCHIVE_FILE}${NC}"
echo -e "  归档大小: ${BOLD}${ARCHIVE_SIZE}${NC}"
if [[ -n "${SHA256_VAL}" ]]; then
    echo -e "  SHA256:   ${BOLD}${SHA256_VAL}${NC}"
    echo "${SHA256_VAL}  $(basename "${ARCHIVE_FILE}")" > "${ARCHIVE_FILE}.sha256"
    echo -e "  校验文件: ${ARCHIVE_FILE}.sha256"
fi
echo -e "\n${BOLD}解压与运行演示：${NC}"
if [[ "${ARCHIVE_EXT}" == "tar.gz" ]]; then
    echo -e "  tar -xzvf dist/$(basename "${ARCHIVE_FILE}")"
    echo -e "  cd ${PACKAGE_NAME} && ./server"
else
    echo -e "  解压 dist/$(basename "${ARCHIVE_FILE}") 后运行 .\\server.exe"
fi
echo -e "  在浏览器打开: ${CYAN}http://127.0.0.1:8081${NC}\n"
