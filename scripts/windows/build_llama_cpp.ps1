<#
  Clone + build llama.cpp on Windows (Release), producing llama-cli.exe.

  Usage (Windows PowerShell):
    powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\windows\build_llama_cpp.ps1

  Output:
    Prints the expected path to `llama-cli.exe` on success.
#>

param(
    [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$llamaDir = Join-Path $RepoRoot 'llama.cpp'

if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
    throw "git not found on PATH. Install Git for Windows first."
}
if (-not (Get-Command cmake -ErrorAction SilentlyContinue)) {
    throw "cmake not found on PATH. Install CMake first."
}

if (-not (Test-Path -LiteralPath $llamaDir)) {
    Write-Host "Cloning llama.cpp into: $llamaDir"
    git clone --depth 1 https://github.com/ggerganov/llama.cpp $llamaDir
} else {
    Write-Host "llama.cpp already exists at: $llamaDir"
}

Push-Location $llamaDir
try {
    Write-Host "Configuring (Release)..."
    cmake -S . -B build -DCMAKE_BUILD_TYPE=Release

    Write-Host "Building..."
    cmake --build build --config Release

    $exe = Join-Path $llamaDir 'build\bin\Release\llama-cli.exe'
    if (-not (Test-Path -LiteralPath $exe)) {
        # Some generators place binaries in build\bin without Release folder.
        $exe2 = Join-Path $llamaDir 'build\bin\llama-cli.exe'
        if (Test-Path -LiteralPath $exe2) { $exe = $exe2 }
    }

    if (-not (Test-Path -LiteralPath $exe)) {
        throw "Build completed but llama-cli.exe not found in expected locations. Search under $llamaDir\\build\\bin."
    }

    Write-Host ""
    Write-Host "SUCCESS: llama.cpp built."
    Write-Host ("LLAMA_CPP_EXE={0}" -f $exe)
    Write-Host ""
} finally {
    Pop-Location
}

