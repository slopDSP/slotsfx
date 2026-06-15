# SlotsFX Automator Build Script
# Run this script in PowerShell to build the plugin and copy it to your DAW's scan folders.

$ErrorActionPreference = "Stop"

# Ensure Cargo is in the environment path for this session
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    $cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
    if (Test-Path $cargoBin) {
        $env:PATH = "$cargoBin;$env:PATH"
    } else {
        Write-Error "Cargo installation not found. Please install Rust from https://rustup.rs"
    }
}

# 1. Build the embedded web UI (Vite) so Rust's include_dir! picks up the latest CSS/JS
$uiWebDir = "ui_web"
if (Test-Path "$uiWebDir/package.json") {
    Write-Host "Building web UI bundle (ui_web)..." -ForegroundColor Cyan
    Push-Location $uiWebDir
    if (-not (Test-Path "node_modules")) {
        Write-Host "Installing UI dependencies (npm install)..." -ForegroundColor Cyan
        npm install
    }
    npm run build
    Pop-Location
} else {
    Write-Warning "ui_web/package.json not found - skipping web UI build"
}

# 2. Compile the plugin (incremental compilation is automatic)
Write-Host "Compiling SlotsFX (Using separate target directory to prevent IDE lockouts)..." -ForegroundColor Cyan
cargo build --release -j 1 --target-dir target/build-release

# Output path of the compiled DLL
$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$dllPath = Join-Path $scriptRoot "target\build-release\release\slotsfx.dll"

if (-not (Test-Path $dllPath)) {
    Write-Error "Build failed or DLL not found at $dllPath"
}

$dllSizeKB = [math]::Round((Get-Item $dllPath).Length / 1KB, 0)
Write-Host "Built DLL size: $dllSizeKB KB" -ForegroundColor Cyan

# 3. Create VST3 bundle structure
$vst3BundleDir = Join-Path $scriptRoot "target\build-release\release\slotsfx.vst3"
$vst3DllPath = "$vst3BundleDir\Contents\x64"

if (Test-Path $vst3BundleDir) {
    Remove-Item -Recurse -Force $vst3BundleDir
}

Write-Host "Creating VST3 bundle structure..." -ForegroundColor Cyan
New-Item -ItemType Directory -Path $vst3DllPath -Force | Out-Null

# Copy DLL into the bundle
Copy-Item -Path $dllPath -Destination $vst3DllPath -Force

# Create Info.plist for VST3 discovery
$infoPlistContent = @"
<?xml version="1.0" encoding="UTF-8"?>

<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist>
  <dict>
    <key>CFBundleExecutable</key>
    <string>slotsfx</string>
    <key>CFBundleIconFile</key>
    <string></string>
    <key>CFBundleIdentifier</key>
    <string>com.slopdsp.slotsfx</string>
    <key>CFBundleName</key>
    <string>SlotsFX</string>
    <key>CFBundleDisplayName</key>
    <string>SlotsFX</string>
    <key>CFBundlePackageType</key>
    <string>VST3</string>
    <key>CFBundleSignature</key>
    <string>????</string>
    <key>CFBundleShortVersionString</key>
    <string>0.1.0</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>NSHumanReadableCopyright</key>
    <string>Copyright 2024 SlopDSP</string>
    <key>NSHighResolutionCapable</key>
    <true/>
  </dict>
</plist>
"@

$infoPlistPath = "$vst3BundleDir\Contents\Info.plist"
[System.IO.File]::WriteAllText($infoPlistPath, $infoPlistContent, [System.Text.Encoding]::UTF8)

# Create PkgInfo file
$pkgInfoContent = "VST3????"
$pkgInfoPath = "$vst3BundleDir\Contents\PkgInfo"
[System.IO.File]::WriteAllText($pkgInfoPath, $pkgInfoContent, [System.Text.Encoding]::ASCII)

Write-Host "Created Info.plist and PkgInfo" -ForegroundColor Cyan

Write-Host "VST3 bundle created at: $vst3BundleDir" -ForegroundColor Green

# 4. Define standard destination paths
$vst3DestDir = "C:\Program Files\Common Files\VST3"
$clapDestDir = "C:\Program Files\Common Files\CLAP"

Write-Host "Deploying to DAW plugins directories..." -ForegroundColor Cyan

# VST3 Deployment
if (Test-Path $vst3DestDir) {
    $vst3Target = Join-Path $vst3DestDir "slotsfx.vst3"
    Write-Host "Copying VST3 bundle to $vst3Target"
    try {
        if (Test-Path $vst3Target) {
            Remove-Item -Recurse -Force $vst3Target
        }
        Copy-Item -Path $vst3BundleDir -Destination $vst3DestDir -Recurse -Force
        Write-Host "VST3 bundle copy successful!" -ForegroundColor Green
    } catch {
        Write-Warning "Could not copy bundle to $vst3DestDir."
    }

    # Also deploy single-file VST3 (some DAWs scan for these)
    $vst3FileTarget = Join-Path $vst3DestDir "slotsfx.vst3.dll"
    try {
        if (Test-Path $vst3FileTarget) { Remove-Item -Force $vst3FileTarget }
        # Actually we want slotsfx.vst3 as a file, not slotsfx.vst3.dll
        # Use a different approach: create a .vst3 that IS the DLL
        # Some DAWs accept slotsfx.vst3 as a single file (it's a renamed DLL)
        $vst3File = Join-Path $vst3DestDir "slotsfx_flat.vst3"
        if (Test-Path $vst3File) { Remove-Item -Force $vst3File }
        Copy-Item -Path $dllPath -Destination $vst3File -Force
        Write-Host "Single-file VST3 (slotsfx_flat.vst3) deployed for DAWs that scan for files" -ForegroundColor Green
    } catch {
        Write-Warning "Could not copy single-file VST3."
    }
} else {
    Write-Warning "VST3 folder not found at $vst3DestDir. Skipped VST3 copy."
}

# CLAP Deployment
if (Test-Path $clapDestDir) {
    $clapTarget = Join-Path $clapDestDir "slotsfx.clap"
    Write-Host "Copying CLAP to $clapTarget"
    try {
        Copy-Item -Path $dllPath -Destination $clapTarget -Force
        Write-Host "CLAP copy successful!" -ForegroundColor Green
    } catch {
        Write-Warning "Could not copy to $clapDestDir. Make sure your DAW is closed and you run PowerShell as Administrator."
    }
} else {
    Write-Warning "CLAP folder not found at $clapDestDir. Skipped CLAP copy."
}

Write-Host "Build and deployment script completed!" -ForegroundColor Green
