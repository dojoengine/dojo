# Function to download and extract a zip file
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

# Function to add a directory to PATH
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

# Check if version parameter is provided
if ($args.Count -eq 0) {
    $dojoVersion = "1.5.0"
} else {
    $dojoVersion = $args[0]
}

# Create installation directory
$installDir = Join-Path $env:USERPROFILE ".dojo"
if (-not (Test-Path $installDir)) {
    New-Item -ItemType Directory -Path $installDir | Out-Null
}

# Download versions.json
$versionsUrl = "https://raw.githubusercontent.com/dojoengine/dojo/main/versions.json"
$versionsPath = Join-Path $installDir "versions.json"
Write-Host "Downloading versions.json..."
Invoke-WebRequest -Uri $versionsUrl -OutFile $versionsPath

# Parse versions.json
$versions = Get-Content $versionsPath | ConvertFrom-Json

# Check if the requested version exists
if (-not $versions.$dojoVersion) {
    Write-Host "Error: Version $dojoVersion not found in versions.json"
    Write-Host "Available versions:"
    $versions.PSObject.Properties.Name | ForEach-Object { Write-Host "- $_" }
    exit 1
}

$versionInfo = $versions.$dojoVersion

# Get the first version from the arrays for each tool
$toriiVersion = $versionInfo.torii[0]
$katanaVersion = $versionInfo.katana[0]

# Download and install Dojo
$dojoUrl = "https://github.com/dojoengine/dojo/releases/download/v$dojoVersion/dojo_v$dojoVersion`_win32_amd64.zip"
$dojoDir = Join-Path $installDir "dojo"
Download-And-Extract -Url $dojoUrl -Destination $dojoDir

# Download and install Torii
$toriiUrl = "https://github.com/dojoengine/torii/releases/download/v$toriiVersion/torii_v$toriiVersion`_win32_amd64.zip"
$toriiDir = Join-Path $installDir "torii"
Download-And-Extract -Url $toriiUrl -Destination $toriiDir

# Download and install Torii
$toriiUrl = "https://github.com/dojoengine/torii/releases/download/v$toriiVersion/torii_v$toriiVersion`_win32_amd64.zip"
$toriiDir = Join-Path $installDir "torii"
Download-And-Extract -Url $toriiUrl -Destination $toriiDir

# Download and install Katana
$katanaUrl = "https://github.com/dojoengine/katana/releases/download/v$katanaVersion/katana_v$katanaVersion`_win32_amd64.zip"
$katanaDir = Join-Path $installDir "katana"
Download-And-Extract -Url $katanaUrl -Destination $katanaDir

# Add binaries to PATH
Add-ToPath -Path $dojoDir
Add-ToPath -Path $toriiDir
Add-ToPath -Path $katanaDir

Write-Host "Installation complete! Dojo, Torii and Katana have been installed to $installDir"
Write-Host "Please restart your terminal to use the new PATH settings."
