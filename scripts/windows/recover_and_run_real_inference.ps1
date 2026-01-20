<#
  Recovery + run script for Windows PowerShell.

  What it does:
  - Stashes any local changes (including untracked files)
  - Switches to branch `cursor/request-id-issue-9b2c`
  - Updates Cargo.lock if needed (so `--locked` works)
  - Sets env vars for llama.cpp backend
  - Runs the ignored real-inference QUIC test with nocapture

  Usage (PowerShell):
    pwsh -NoProfile -File .\scripts\windows\recover_and_run_real_inference.ps1 `
      -LlamaExe "E:\rust\llama-rs\target\release\llama-cli.exe" `
      -GgufPath "E:\rust\llamaModels\YOUR_MODEL.gguf"

  Optional:
    -Threads 8
    -NoMmap 1
    -Strict 1
#>

param(
    [Parameter(Mandatory)]
    [string]$LlamaExe,

    [Parameter(Mandatory)]
    [string]$GgufPath,

    [int]$Threads = 8,

    [ValidateSet(0,1)]
    [int]$NoMmap = 1,

    [ValidateSet(0,1)]
    [int]$Strict = 1
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Start-RunTranscript {
    # Create a log file under repo root so users can paste errors even if an editor extension hides them.
    $repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
    $logDir = Join-Path $repoRoot 'recovery_logs'
    New-Item -ItemType Directory -Path $logDir -Force | Out-Null
    $stamp = Get-Date -Format 'yyyyMMdd_HHmmss'
    $logPath = Join-Path $logDir ("recover_and_run_real_inference_{0}.log" -f $stamp)
    Start-Transcript -Path $logPath -Append | Out-Null
    Write-Host ("Transcript: {0}" -f $logPath)
    return $logPath
}

function Assert-FileExists {
    param([Parameter(Mandatory)][string]$Path)
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "File not found: $Path"
    }
}

function Run {
    param(
        [Parameter(Mandatory)][string]$Exe,
        [Parameter(Mandatory)][string[]]$Args,
        [Parameter()][string]$Why = ""
    )
    if ($Why) { Write-Host "`n==> $Why" }
    $joined = ($Args | ForEach-Object { if ($_ -match '\s') { '"' + $_ + '"' } else { $_ } }) -join ' '
    Write-Host ("    {0} {1}" -f $Exe, $joined)
    & $Exe @Args
    if ($LASTEXITCODE -ne 0) {
        throw ("Command failed (exit={0}): {1} {2}" -f $LASTEXITCODE, $Exe, $joined)
    }
}

Assert-FileExists -Path $LlamaExe
Assert-FileExists -Path $GgufPath

try {
    $logPath = Start-RunTranscript

    Write-Host "Repo:      $((Get-Location).Path)"
    Write-Host "Llama exe:  $LlamaExe"
    Write-Host "GGUF path:  $GgufPath"

    # 1) Stash local changes (safe)
    Run -Why "Stash any local changes" -Exe "git" -Args @("status","--porcelain")
    Run -Exe "git" -Args @("stash","push","-u","-m","recovery stash before real inference run")

    # 2) Switch to the branch with the real-inference backend + test
    Run -Why "Fetch branch" -Exe "git" -Args @("fetch","origin","cursor/request-id-issue-9b2c")
    Run -Why "Switch branch" -Exe "git" -Args @("switch","cursor/request-id-issue-9b2c")
    Run -Why "Pull latest" -Exe "git" -Args @("pull","origin","cursor/request-id-issue-9b2c")

    # 3) Ensure Cargo.lock is in sync (so --locked works)
    # If this fails due to network restrictions, you can try --offline afterwards.
    Run -Why "Generate lockfile (may hit network once)" -Exe "cargo" -Args @("generate-lockfile")

    # 4) Configure env for real inference
    $env:PUNCH_INFERENCE_BACKEND = "llama_cpp"
    $env:LLAMA_CPP_EXE = $LlamaExe
    $env:LLAMA_GGUF_PATH = $GgufPath
    $env:LLAMA_THREADS = "$Threads"
    $env:LLAMA_NO_MMAP = "$NoMmap"

    if ($Strict -eq 1) {
        $env:PUNCH_STRICT_DISTRIBUTED = "1"
    } else {
        Remove-Item Env:PUNCH_STRICT_DISTRIBUTED -ErrorAction SilentlyContinue
    }

    Write-Host "`nEnvironment:"
    Write-Host "  PUNCH_INFERENCE_BACKEND=$env:PUNCH_INFERENCE_BACKEND"
    Write-Host "  LLAMA_CPP_EXE=$env:LLAMA_CPP_EXE"
    Write-Host "  LLAMA_GGUF_PATH=$env:LLAMA_GGUF_PATH"
    Write-Host "  LLAMA_THREADS=$env:LLAMA_THREADS"
    Write-Host "  LLAMA_NO_MMAP=$env:LLAMA_NO_MMAP"
    Write-Host "  PUNCH_STRICT_DISTRIBUTED=$env:PUNCH_STRICT_DISTRIBUTED"

    # 5) Run the ignored test
    Run -Why "Run real inference QUIC test (ignored)" -Exe "cargo" -Args @("test","--locked","--test","e2e_quic_real_inference_distributed_serving_tests","--","--ignored","--nocapture")

    Write-Host "`nDONE: Real inference test completed successfully."
} finally {
    try { Stop-Transcript | Out-Null } catch {}
}

