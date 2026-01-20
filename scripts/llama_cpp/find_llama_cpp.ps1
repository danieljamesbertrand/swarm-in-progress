Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

<#
  Finds the llama.cpp executable (llama-cli) for this repo.

  Usage (PowerShell):
    pwsh -NoProfile -File .\scripts\llama_cpp\find_llama_cpp.ps1

  Optional:
    $env:LLAMA_CPP_EXE = 'C:\path\to\llama-cli.exe'
#>

function Test-Exe {
    param([Parameter(Mandatory)][string]$Path)
    try {
        return (Test-Path -LiteralPath $Path) -and -not (Get-Item -LiteralPath $Path).PSIsContainer
    } catch {
        return $false
    }
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path

Write-Host ("Repo root: {0}" -f $repoRoot)

# 1) Explicit override
if ($env:LLAMA_CPP_EXE -and (Test-Exe -Path $env:LLAMA_CPP_EXE)) {
    Write-Host ("FOUND via LLAMA_CPP_EXE: {0}" -f $env:LLAMA_CPP_EXE)
    exit 0
}

# 2) Common repo-relative locations (if you cloned/built llama.cpp inside this repo)
$candidates = @(
    (Join-Path $repoRoot 'llama.cpp\build\bin\Release\llama-cli.exe'),
    (Join-Path $repoRoot 'llama.cpp\build\bin\Release\llama.exe'),
    (Join-Path $repoRoot 'llama.cpp\build\bin\Debug\llama-cli.exe'),
    (Join-Path $repoRoot 'llama.cpp\build\bin\Debug\llama.exe'),
    (Join-Path $repoRoot 'llama.cpp\build\bin\llama-cli.exe'),
    (Join-Path $repoRoot 'llama.cpp\build\bin\llama.exe'),
    (Join-Path $repoRoot 'llama.cpp\bin\llama-cli.exe'),
    (Join-Path $repoRoot 'llama.cpp\bin\llama.exe')
)

foreach ($p in $candidates) {
    if (Test-Exe -Path $p) {
        Write-Host ("FOUND in repo: {0}" -f $p)
        exit 0
    }
}

# 3) PATH lookup
foreach ($name in @('llama-cli.exe','llama.exe','llama-cli','llama')) {
    $cmd = Get-Command $name -ErrorAction SilentlyContinue
    if ($cmd -and $cmd.Source -and (Test-Exe -Path $cmd.Source)) {
        Write-Host ("FOUND on PATH: {0}" -f $cmd.Source)
        exit 0
    }
}

Write-Host ""
Write-Host "NOT FOUND: llama.cpp executable (llama-cli)."
Write-Host ""
Write-Host "Expected Windows build output paths (inside a llama.cpp clone):"
Write-Host "  - llama.cpp\build\bin\Release\llama-cli.exe"
Write-Host "  - llama.cpp\build\bin\Release\llama.exe"
Write-Host ""
Write-Host "If you want to build llama.cpp on Windows (from a llama.cpp folder):"
Write-Host "  cmake -S . -B build -DCMAKE_BUILD_TYPE=Release"
Write-Host "  cmake --build build --config Release -j"
Write-Host ""
Write-Host "If you build in WSL instead, the executable will be:"
Write-Host "  - llama.cpp/build/bin/llama-cli   (inside WSL filesystem)"
Write-Host ""
Write-Host "Tip: set an explicit path once you have it:"
Write-Host "  `$env:LLAMA_CPP_EXE='C:\path\to\llama-cli.exe'"
Write-Host ""

exit 1

