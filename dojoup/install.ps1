# Dojo toolchain installer for Windows
# Usage: powershell -ExecutionPolicy Bypass -File install.ps1 [version]

function Download-And-Extract {
    param (
        [string]$Url,
        [string]$Destination
    )

    $tempFile = [System.IO.Path]::GetTempPath() + [System.Guid]::NewGuid().ToString() + ".zip"
    Write-Host "Downloading from $Url..."
    Invoke-WebRequest -Uri $Url -OutFile $tempFile

    Write-Host "Extracting to $Destination..."
    Expand-Archive -Path $tempFile -DestinationPath $Destination -Force
    Remove-Item $tempFile
}

function Get-LatestVersion {
    param (
        [string]$Repo
    )

    $release = Invoke-RestMethod -Uri "https://api.github.com/repos/dojoengine/$Repo/releases/latest"
    return $release.tag_name -replace '^v', ''
}

function Add-ToPath {
    param (
        [string]$Path
    )

    $currentPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ($currentPath -notlike "*$Path*") {
        [Environment]::SetEnvironmentVariable("Path", "$currentPath;$Path", "User")
        Write-Host "Added $Path to PATH"
    }
}

$installDir = Join-Path $env:USERPROFILE ".dojo\bin"
if (-not (Test-Path $installDir)) {
    New-Item -ItemType Directory -Path $installDir | Out-Null
}

# Tools and their GitHub repos
$tools = @(
    @{ Name = "dojo";   Repo = "dojo" },
    @{ Name = "katana"; Repo = "katana" },
    @{ Name = "torii";  Repo = "torii" }
)

foreach ($tool in $tools) {
    $version = Get-LatestVersion -Repo $tool.Repo
    $url = "https://github.com/dojoengine/$($tool.Repo)/releases/download/v$version/$($tool.Name)_v$($version)_win32_amd64.zip"
    Download-And-Extract -Url $url -Destination $installDir
    Write-Host "Installed $($tool.Name) v$version"
}

Add-ToPath -Path $installDir

Write-Host ""
Write-Host "Dojo toolchain installation complete!"
Write-Host "Please restart your terminal to use the new PATH settings."
