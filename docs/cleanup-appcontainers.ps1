param(
    [Parameter(Mandatory = $true)]
    [string]$Prefix
)

# The profile deletion requires a Win32 API call, as PowerShell does not provide a native cmdlet for this.
Add-Type @"
using System;
using System.Runtime.InteropServices;

public static class AppContainerNative {
    [DllImport("userenv.dll", CharSet = CharSet.Unicode)]
    public static extern int DeleteAppContainerProfile(string pszAppContainerName);
}
"@

# AppContainer profiles for current user are registered here:
$baseKey = "HKCU:\Software\Classes\Local Settings\Software\Microsoft\Windows\CurrentVersion\AppContainer\Mappings"

if (-not (Test-Path $baseKey)) {
    Write-Output "No AppContainer profiles found for current user."
    return
}

$deleted = 0
$failed  = 0

Get-ChildItem $baseKey | ForEach-Object {
    $sidKey = $_.PSPath
    $displayName = $_.GetValue("DisplayName")
    $moniker = $_.GetValue("Moniker")

    if ($displayName -and $displayName.StartsWith($Prefix)) {
        Write-Output "Attempting to delete AppContainer '$displayName' ($moniker)"

        $result = [AppContainerNative]::DeleteAppContainerProfile($displayName)

        if ($result -eq 0) {
            Write-Output "-- Deleted"
            $deleted++
        }
        else {
            Write-Warning "-- Failed to delete (Error code: $result)"
            $failed++
        }
    }
}

Write-Output ""
Write-Output "Completed."
Write-Output "Deleted: $deleted"
Write-Output "Failed:  $failed"
