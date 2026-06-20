param(
    [string]$EspIdfVersion = "v5.5.4"
)

$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($env:IDF_TOOLS_PATH)) {
    throw "请先设置用户环境变量 IDF_TOOLS_PATH，例如 C:\Espressif"
}

$toolsRoot = $env:IDF_TOOLS_PATH.TrimEnd("\", "/")
$frameworkPath = Join-Path $toolsRoot "frameworks\esp-idf-$EspIdfVersion"
$pythonEnvPath = Join-Path $toolsRoot "python_env\idf5.5_py3.11_env"
$libclangPath = Join-Path $toolsRoot "tools\esp-clang-libs\esp-20.1.1_20250829\esp-clang\bin"
$romElfDir = Join-Path $toolsRoot "tools\esp-rom-elfs\20241011"

$pathEntries = @(
    (Join-Path $frameworkPath "tools"),
    (Join-Path $pythonEnvPath "Scripts"),
    (Join-Path $toolsRoot "tools\cmake\3.30.2\bin"),
    (Join-Path $toolsRoot "tools\ninja\1.12.1"),
    (Join-Path $toolsRoot "tools\xtensa-esp-elf\esp-14.2.0_20260121\xtensa-esp-elf\bin"),
    (Join-Path $toolsRoot "tools\esp-clang\esp-19.1.2_20250312\esp-clang\bin"),
    $libclangPath
)

$requiredPaths = @(
    $frameworkPath,
    $pythonEnvPath,
    $libclangPath,
    $romElfDir
) + $pathEntries

foreach ($path in $requiredPaths) {
    if (-not (Test-Path -LiteralPath $path)) {
        throw "ESP-IDF 环境路径不存在：$path"
    }
}

$env:IDF_PATH = $frameworkPath
$env:IDF_PYTHON_ENV_PATH = $pythonEnvPath
$env:LIBCLANG_PATH = $libclangPath
$env:ESP_ROM_ELF_DIR = $romElfDir

$currentPathEntries = $env:PATH -split ";"
$mergedPathEntries = @()

foreach ($path in $pathEntries) {
    if ($currentPathEntries -notcontains $path) {
        $mergedPathEntries += $path
    }
}

$env:PATH = (($mergedPathEntries + $currentPathEntries) -join ";")

Write-Host "ESP-IDF $EspIdfVersion environment activated from $toolsRoot"
