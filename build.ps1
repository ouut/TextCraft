<#
.SYNOPSIS
    TextCraft 3.0 Windows PowerShell 一键构建与打包脚本
.DESCRIPTION
    自动检测环境、编译 server 二进制、组装发布包并生成标准 zip 归档
#>

param (
    [switch]$Debug,
    [switch]$Test,
    [switch]$Clean,
    [switch]$NoArchive,
    [string]$Target = "",
    [string]$OutDir = "dist"
)

$ErrorActionPreference = "Stop"

Write-Host "======================================================" -ForegroundColor Cyan
Write-Host "      TextCraft 3.0 Windows 本地构建与打包系统        " -ForegroundColor Green
Write-Host "======================================================" -ForegroundColor Cyan

# 1. 检查 Rust 工具链
Write-Host "`n[1/5] 检查本地编译工具链..." -ForegroundColor Yellow
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Error "未检测到 cargo 命令！请先从 https://rustup.rs 安装 Rust 工具链。"
    exit 1
}

$rustcVer = (rustc --version)
Write-Host "  ✓ Rustc: $rustcVer" -ForegroundColor Green

# 2. 检测环境
Write-Host "`n[2/5] 检测系统环境与架构..." -ForegroundColor Yellow
$arch = if ([System.Environment]::Is64BitOperatingSystem) { "x86_64" } else { "x86" }
$packageName = "textcraft-windows-$arch"
Write-Host "  ✓ 目标包名称: $packageName" -ForegroundColor Green

# 3. 清理与测试
if ($Clean) {
    Write-Host "`n[3/5] 执行 cargo clean..." -ForegroundColor Yellow
    cargo clean
}

if ($Test) {
    Write-Host "`n[3/5] 执行本地测试 (cargo test)..." -ForegroundColor Yellow
    cargo test
    Write-Host "  ✓ 测试全部通过！" -ForegroundColor Green
}

# 4. 编译
$profile = if ($Debug) { "debug" } else { "release" }
$buildArgs = @("build", "--bin", "server")
if (-not $Debug) { $buildArgs += "--release" }
if ($Target) { $buildArgs += @("--target", $Target) }

Write-Host "`n[4/5] 编译 server 二进制 ($profile)..." -ForegroundColor Yellow
Write-Host "  执行: cargo $($buildArgs -join ' ')" -ForegroundColor Cyan
& cargo @buildArgs

$binSource = if ($Target) {
    "target\$Target\$profile\server.exe"
} else {
    "target\$profile\server.exe"
}

if (-not (Test-Path $binSource)) {
    Write-Error "未找到编译产物: $binSource"
    exit 1
}

$binItem = Get-Item $binSource
Write-Host "  ✓ 编译成功: $binSource ($([math]::Round($binItem.Length / 1MB, 2)) MB)" -ForegroundColor Green

# 5. 打包组装
Write-Host "`n[5/5] 组装发布包与生成归档..." -ForegroundColor Yellow
$stagingDir = Join-Path $OutDir "staging\$packageName"
if (Test-Path $stagingDir) { Remove-Item -Recurse -Force $stagingDir }
New-Item -ItemType Directory -Path $stagingDir -Force | Out-Null
New-Item -ItemType Directory -Path $OutDir -Force | Out-Null

Copy-Item $binSource -Destination "$stagingDir\server.exe"
Copy-Item -Recurse "scripts" -Destination "$stagingDir\"
Copy-Item "README.md" -Destination "$stagingDir\"

if ($NoArchive) {
    Write-Host "`n✓ 完成！产物位于: $stagingDir" -ForegroundColor Green
    exit 0
}

$zipFile = Join-Path $OutDir "$packageName.zip"
if (Test-Path $zipFile) { Remove-Item -Force $zipFile }

Write-Host "  -> 压缩归档至: $zipFile..." -ForegroundColor Cyan
Compress-Archive -Path "$stagingDir\*" -DestinationPath $zipFile -Force

$zipItem = Get-Item $zipFile
$hash = (Get-FileHash -Path $zipFile -Algorithm SHA256).Hash

Write-Host "`n======================================================" -ForegroundColor Green
Write-Host "               构建与打包成功！🎉                     " -ForegroundColor Green
Write-Host "======================================================" -ForegroundColor Green
Write-Host "  产出归档: $zipFile" -ForegroundColor Cyan
Write-Host "  归档大小: $([math]::Round($zipItem.Length / 1MB, 2)) MB"
Write-Host "  SHA256:   $hash"
Set-Content -Path "$zipFile.sha256" -Value "$hash  $packageName.zip"
Write-Host "`n解压后在控制台运行 .\server.exe，并在浏览器打开 http://127.0.0.1:8081 即可体验！"
