Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-PowerShellScripts {
    param(
        [Parameter(Mandatory)]
        [string]$RepoRoot
    )

    # Keep it simple: parse every *.ps1 in the repo, including backups.
    Get-ChildItem -Path $RepoRoot -Recurse -File -Filter '*.ps1'
}

function Assert-PowerShellParses {
    param(
        [Parameter(Mandatory)]
        [System.IO.FileInfo[]]$Files
    )

    $failed = @()

    foreach ($f in $Files) {
        $tokens = $null
        $errors = $null
        [void][System.Management.Automation.Language.Parser]::ParseFile($f.FullName, [ref]$tokens, [ref]$errors)

        if ($errors -and $errors.Count -gt 0) {
            $failed += [PSCustomObject]@{
                Path   = $f.FullName
                Errors = ($errors | ForEach-Object { $_.Message }) -join "`n"
            }
        }
    }

    if ($failed.Count -gt 0) {
        Write-Host "PowerShell parse failures:`n"
        foreach ($x in $failed) {
            Write-Host "----"
            Write-Host $x.Path
            Write-Host $x.Errors
        }
        throw ("{0} PowerShell script(s) failed to parse." -f $failed.Count)
    }
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$files = Get-PowerShellScripts -RepoRoot $repoRoot
Assert-PowerShellParses -Files $files
Write-Host ("OK: Parsed {0} PowerShell script(s)." -f $files.Count)
