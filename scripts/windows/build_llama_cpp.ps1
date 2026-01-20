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

function Resolve-CMakePath {
    $cmd = Get-Command cmake -ErrorAction SilentlyContinue
    if ($cmd -and $cmd.Source) { return $cmd.Source }

    $candidates = @(
        "$env:ProgramFiles\CMake\bin\cmake.exe",
        "${env:ProgramFiles(x86)}\CMake\bin\cmake.exe",
        "$env:LocalAppData\Programs\CMake\bin\cmake.exe"
    )

    foreach ($p in $candidates) {
        if ($p -and (Test-Path -LiteralPath $p -PathType Leaf)) { return $p }
    }

    # Last resort: search a few common roots (can be slow; keep bounded).
    foreach ($root in @("$env:ProgramFiles", "${env:ProgramFiles(x86)}")) {
        if (-not $root -or -not (Test-Path -LiteralPath $root -PathType Container)) { continue }
        $found = Get-ChildItem -LiteralPath $root -Recurse -File -Filter "cmake.exe" -ErrorAction SilentlyContinue |
            Select-Object -First 1 -ExpandProperty FullName
        if ($found) { return $found }
    }

    return $null
}

if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
    throw "git not found on PATH. Install Git for Windows first."
}

$cmakeExe = Resolve-CMakePath
if (-not $cmakeExe) {
    throw "cmake.exe not found. Install CMake (Kitware) and/or add it to PATH. Example: C:\Program Files\CMake\bin\cmake.exe"
}
Write-Host ("Using CMake: {0}" -f $cmakeExe)

if (-not (Test-Path -LiteralPath $llamaDir)) {
    Write-Host "Cloning llama.cpp into: $llamaDir"
    git clone --depth 1 https://github.com/ggerganov/llama.cpp $llamaDir
} else {
    Write-Host "llama.cpp already exists at: $llamaDir"
}

Push-Location $llamaDir
try {
    Write-Host "Configuring (Release)..."
    & $cmakeExe -S . -B build -DCMAKE_BUILD_TYPE=Release

    Write-Host "Building..."
    & $cmakeExe --build build --config Release

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

