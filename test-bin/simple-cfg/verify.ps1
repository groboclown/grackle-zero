# Verify that:
#   - The compiled program only directly depends on kernel32.dll
#   - The compiled program has the CFG settings enabled.
# Requires that either 'dumpbin' or 'llvm-readobj' are installed and available on your env:Path.

param(
    [Parameter(Mandatory = $true)]
    [string]$ExePath
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"


# Run the assertions.
function Assert {
    param([string]$ResolvedExe)

    $llvmReadObj = Get-Command llvm-readobj -ErrorAction SilentlyContinue
    if ($null -ne $llvmReadObj) {
        $imports = Get-ImportsLlvm -ResolvedExe $ResolvedExe -LlvmReadObj $llvmReadObj.Source
        Assert-OnlyKernel32 -Dlls $($imports[0]) -ToolName "llvm-readobj" -RawOutput $($imports[1])
        Assert-CfgLlvm -ResolvedExe $ResolvedExe -LlvmReadObj $llvmReadObj.Source
        Write-Host "Verification Success (llvm toolchain)" -ForegroundColor Green
        return
    }

    $dumpbin = Get-Command dumpbin -ErrorAction SilentlyContinue
    if ($null -ne $dumpbin) {
        $imports = Get-ImportsDumpbin -ResolvedExe $ResolvedExe -Dumpbin $dumpbin.Source
        Assert-OnlyKernel32 -Dlls $($imports[0]) -ToolName "dumpbin" -RawOutput $($imports[1])
        Assert-CfgDumpbin -ResolvedExe $ResolvedExe -Dumpbin $dumpbin.Source
        Write-Host "Verification Success (msvc toolchain)" -ForegroundColor Green
        return
    }

    Write-Host "Verification failed: neither llvm-readobj nor dumpbin was found in PATH." -ForegroundColor Red
    exit 1
}

# Validate that only kernel32.dll is in the direct dependencies.
function Assert-OnlyKernel32 {
    param([string[]]$Dlls, [string]$ToolName, [string]$RawOutput)

    $normalized = @(Get-NormalizedDlls -Dlls $Dlls)
    if ($normalized.Count -ne 1 -or $normalized[0] -ne "kernel32.dll") {
        Write-Host "Verification failed: expected only kernel32.dll import ($ToolName)." -ForegroundColor Red
        Write-Host "Detected imports: $($normalized -join ', ')"
        Write-Host ""
        Write-Host $RawOutput
        exit 1
    }
}

# Validate that the executable has CFG enabled.
function Assert-CfgLlvm {
    param([string]$ResolvedExe, [string]$LlvmReadObj)
    $cfgOutput = & $LlvmReadObj --coff-load-config $ResolvedExe 2>&1 | Out-String
    if ($LASTEXITCODE -ne 0) {
        Write-Host "llvm-readobj failed while checking load config." -ForegroundColor Red
        Write-Host $cfgOutput
        exit 1
    }
    if ($cfgOutput -notmatch 'CF_INSTRUMENTED') {
        Write-Host "Verification failed: CFG flag CF_INSTRUMENTED not present." -ForegroundColor Red
        Write-Host $cfgOutput
        exit 1
    }    
}

function Assert-CfgDumpbin {
    param([string]$ResolvedExe, [string]$Dumpbin)
    $cfgOutput = & $Dumpbin /nologo /loadconfig $ResolvedExe 2>&1 | Out-String
    if ($LASTEXITCODE -ne 0) {
        Write-Host "dumpbin failed while checking load config." -ForegroundColor Red
        Write-Host $cfgOutput
        exit 1
    }
    if ($cfgOutput -notmatch '(?i)\bCF\s+Instrumented\b') {
        Write-Host "Verification failed: CFG flag 'CF Instrumented' not present." -ForegroundColor Red
        Write-Host $cfgOutput
        exit 1
    }
}

# Find the DLL imports
function Get-ImportsLlvm {
    param([string]$ResolvedExe, [string]$LlvmReadObj)

    $importsOutput = & $LlvmReadObj --coff-imports $ResolvedExe 2>&1 | Out-String
    if ($LASTEXITCODE -ne 0) {
        Write-Host "llvm-readobj failed while checking imports." -ForegroundColor Red
        Write-Host $importsOutput
        exit 1
    }
    $imports = @()
    foreach ($line in ($importsOutput -split "`r?`n")) {
        if ($line -match '^\s*Name:\s*(.+)$') {
            $imports += $Matches[1].Trim()
        }
    }
    return @($imports, $importsOutput)
}

function Get-ImportsDumpbin {
    param([string]$ResolvedExe, [string]$Dumpbin)

    $importsOutput = & $Dumpbin /nologo /dependents $ResolvedExe 2>&1 | Out-String
    if ($LASTEXITCODE -ne 0) {
        Write-Host "dumpbin failed while checking imports." -ForegroundColor Red
        Write-Host $importsOutput
        exit 1
    }
    $imports = @()
    foreach ($line in ($importsOutput -split "`r?`n")) {
        if ($line -match '^\s*([A-Za-z0-9_.-]+\.dll)\s*$') {
            $imports += $Matches[1].Trim()
        }
    }
    return @($imports, $importsOutput)
}


# Turn the DLLs into an easily checked string.
function Get-NormalizedDlls {
    param([string[]]$Dlls)
    return @($Dlls | ForEach-Object { $_.Trim().ToLowerInvariant() } | Select-Object -Unique)
}


# ---------------------------------------------------------------------
# Main

if (-not (Test-Path -LiteralPath $ExePath)) {
    Write-Host "Verification failed: executable not found: $ExePath" -ForegroundColor Red
    exit 1
}

$resolvedExe = (Resolve-Path -LiteralPath $ExePath).Path
Assert -ResolvedExe $resolvedExe
