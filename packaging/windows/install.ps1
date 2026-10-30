# Install mudcrab's command line for this user, so `mudcrab` runs from any
# terminal: mudcrab.exe is copied into %LOCALAPPDATA%\Programs\mudcrab, and
# that folder put on the user's PATH. No administrator rights are needed.
# Running it again (for a newer version) replaces the program.
#
# From the unzipped folder, in PowerShell:
#   powershell -ExecutionPolicy Bypass -File .\install.ps1
#
# The GUI's installer does the same for the copy it carries, so this is only
# for the command line on its own.

$ErrorActionPreference = 'Stop'
$dir = Join-Path $env:LOCALAPPDATA 'Programs\mudcrab'
New-Item -ItemType Directory -Force -Path $dir | Out-Null
Copy-Item (Join-Path $PSScriptRoot 'mudcrab.exe') $dir -Force

$parts = @([Environment]::GetEnvironmentVariable('Path', 'User') -split ';' | Where-Object { $_ })
if ($parts -notcontains $dir) {
    [Environment]::SetEnvironmentVariable('Path', (($parts + $dir) -join ';'), 'User')
    Write-Host "Added $dir to your PATH."
}
Write-Host "mudcrab is installed in $dir. Open a new terminal and run: mudcrab --help"
