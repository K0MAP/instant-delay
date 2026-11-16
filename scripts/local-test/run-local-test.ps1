#!/usr/bin/env pwsh
<#
.SYNOPSIS
    Run complete local test stack for InstantDelay
    FFmpeg -> InstantDelay (1935) -> Relay -> MediaMTX (1936) -> HLS (8888) -> VLC

.PREREQUISITES
    - ffmpeg in PATH (winget install Gyan.FFmpeg)
    - MediaMTX in PATH (winget install bluenviron.mediamtx)
    - VLC in PATH (winget install VideoLAN.VLC)
    - InstantDelay built (cargo build --release in src-tauri)

.USAGE
    cd scripts\local-test
    .\run-local-test.ps1
#>

param(
    [switch]$NoVlc,
    [switch]$NoFfmpeg,
    [int]$DelaySec = 10,
    [int]$TestDurationSec = 30
)

$ErrorActionPreference = "Stop"

# Colors for output
$Green = [ConsoleColor]::Green
$Yellow = [ConsoleColor]::Yellow
$Red = [ConsoleColor]::Red
$Cyan = [ConsoleColor]::Cyan
$Gray = [ConsoleColor]::DarkGray

function Write-Status($msg) { Write-Host "[$(Get-Date -Format 'HH:mm:ss')] $msg" -ForegroundColor $Cyan }
function Write-Success($msg) { Write-Host "[$(Get-Date -Format 'HH:mm:ss')] ✓ $msg" -ForegroundColor $Green }
function Write-Warning($msg) { Write-Host "[$(Get-Date -Format 'HH:mm:ss')] ⚠ $msg" -ForegroundColor $Yellow }
function Write-ErrorMsg($msg) { Write-Host "[$(Get-Date -Format 'HH:mm:ss')] ✗ $msg" -ForegroundColor $Red }
function Write-Debug($msg) { Write-Host "[$(Get-Date -Format 'HH:mm:ss')] $msg" -ForegroundColor $Gray }

# Paths
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$ProjectRoot = Split-Path (Split-Path $ScriptDir)
$SrcTauriDir = Join-Path $ProjectRoot "src-tauri"
$BinaryPath = Join-Path $SrcTauriDir "target\debug\instant-delay.exe"
$MediaMtxConfig = Join-Path $ScriptDir "mediamtx.yml"

# Check prerequisites
Write-Status "Checking prerequisites..."

$missing = @()
if (-not (Get-Command ffmpeg -ErrorAction SilentlyContinue)) { $missing += "ffmpeg" }
if (-not (Get-Command mediamtx -ErrorAction SilentlyContinue)) { $missing += "mediamtx" }
if (-not $NoVlc -and -not (Get-Command vlc -ErrorAction SilentlyContinue)) { $missing += "vlc" }
if (-not (Test-Path $BinaryPath)) { $missing += "instant-delay binary (run 'cargo build' in src-tauri)" }

if ($missing.Count -gt 0) {
    Write-ErrorMsg "Missing prerequisites:"
    $missing | ForEach-Object { Write-ErrorMsg "  - $_" }
    Write-Host ""
    Write-Host "Install with:" -ForegroundColor $Yellow
    Write-Host "  winget install Gyan.FFmpeg" -ForegroundColor $Gray
    Write-Host "  winget install bluenviron.mediamtx" -ForegroundColor $Gray
    Write-Host "  winget install VideoLAN.VLC" -ForegroundColor $Gray
    Write-Host "  cd src-tauri; cargo build" -ForegroundColor $Gray
    exit 1
}

Write-Success "All prerequisites found"

# Cleanup function
$processes = @()
function Cleanup {
    Write-Status "Cleaning up processes..."
    foreach ($p in $processes) {
        if (-not $p.HasExited) {
            try { Stop-Process -Id $p.Id -Force } catch { }
        }
    }
}
# Register cleanup on exit
$global:cleanupRegistered = $true

# Start MediaMTX
Write-Status "Starting MediaMTX on ports 1936 (RTMP) / 8888 (HLS)..."
$mediamtx = Start-Process mediamtx -ArgumentList $MediaMtxConfig -PassThru -WindowStyle Hidden
$processes += $mediamtx
Start-Sleep 2

if ($mediamtx.HasExited) {
    Write-ErrorMsg "MediaMTX failed to start"
    exit 1
}
Write-Success "MediaMTX started (PID: $($mediamtx.Id))"

# Start InstantDelay
Write-Status "Starting InstantDelay (RTMP ingest 1935, Web UI 8081, Overlay 8080)..."
$env:INSTANTDELAY_ALLOW_PRIVATE_RELAY = "1"
$env:RUST_LOG = "info,instant_delay=debug,rtmp=debug"

$instantDelay = Start-Process $BinaryPath -PassThru -WindowStyle Hidden
$processes += $instantDelay
Start-Sleep 3

if ($instantDelay.HasExited) {
    Write-ErrorMsg "InstantDelay failed to start"
    exit 1
}
Write-Success "InstantDelay started (PID: $($instantDelay.Id))"

# Wait for servers to be ready
Write-Status "Waiting for servers to be ready..."
$ready = $false
for ($i = 0; $i -lt 30; $i++) {
    try {
        $health = Invoke-RestMethod -Uri "http://127.0.0.1:8081/health" -Method Get -TimeoutSec 2 -ErrorAction Stop
        if ($health.status -eq "ok") { $ready = $true; break }
    } catch { }
    Start-Sleep 1
}

if (-not $ready) {
    Write-ErrorMsg "Servers not ready after 30 seconds"
    Cleanup
    exit 1
}
Write-Success "Servers are ready"

# Add relay output via API
Write-Status "Adding relay output to MediaMTX..."
try {
    $body = @{
        id = "local-test-sink"
        name = "Local MediaMTX"
        platform = "Custom"
        url = "rtmp://127.0.0.1:1936/live/test"
        stream_key = "test"
        enabled = $true
        priority = 0
        backup = $false
    } | ConvertTo-Json -Depth 3

    Invoke-RestMethod -Uri "http://127.0.0.1:8081/api/platforms" -Method Post -Body $body -ContentType "application/json" -TimeoutSec 5 | Out-Null
    Write-Success "Relay output configured"
} catch {
    Write-Warning "Could not add relay via API (may already exist): $_"
}

# Set delay
Write-Status "Setting delay to ${DelaySec}s..."
try {
    $body = @{ delay_sec = $DelaySec } | ConvertTo-Json
    Invoke-RestMethod -Uri "http://127.0.0.1:8081/api/delay" -Method Post -Body $body -ContentType "application/json" -TimeoutSec 5 | Out-Null
    Write-Success "Delay set to ${DelaySec}s"
} catch {
    Write-Warning "Could not set delay via API: $_"
}

# Start ffmpeg publisher
if (-not $NoFfmpeg) {
    Write-Status "Starting ffmpeg test publisher (${TestDurationSec}s)..."
    $fontPath = "C:/Windows/Fonts/consola.ttf"
    if (-not (Test-Path $fontPath)) { $fontPath = "C:/Windows/Fonts/arial.ttf" }

    $filter = "drawtext=fontfile='$fontPath':text='%{localtime\:%X}':x=24:y=24:fontsize=64:fontcolor=white:box=1:boxcolor=black@0.6,drawtext=fontfile='$fontPath':text='InstantDelay test':x=24:y=104:fontsize=32:fontcolor=yellow"

    $ffmpegArgs = @(
        "-hide_banner", "-loglevel", "info", "-re",
        "-f", "lavfi", "-i", "testsrc2=size=1280x720:rate=30",
        "-f", "lavfi", "-i", "sine=frequency=440:sample_rate=48000",
        "-vf", $filter,
        "-c:v", "libx264", "-preset", "veryfast", "-tune", "zerolatency",
        "-g", "60", "-keyint_min", "60", "-sc_threshold", "0",
        "-b:v", "2500k", "-pix_fmt", "yuv420p",
        "-c:a", "aac", "-b:a", "128k",
        "-t", $TestDurationSec.ToString(),
        "-f", "flv", "rtmp://127.0.0.1:1935/live/test"
    )

    $ffmpeg = Start-Process ffmpeg -ArgumentList $ffmpegArgs -PassThru -WindowStyle Hidden
    $processes += $ffmpeg
    Write-Success "ffmpeg started (PID: $($ffmpeg.Id))"
}

# Start VLC player
if (-not $NoVlc) {
    Write-Status "Opening VLC to play delayed HLS stream..."
    $hlsUrl = "http://127.0.0.1:8888/live/test/index.m3u8"
    $vlc = Start-Process "vlc" -ArgumentList $hlsUrl, "--play-and-exit" -PassThru -WindowStyle Hidden
    $processes += $vlc
    Write-Success "VLC opened with HLS stream: $hlsUrl"
}

Write-Host ""
Write-Host "╔═══════════════════════════════════════════════════════════════╗" -ForegroundColor $Cyan
Write-Host "║  LOCAL TEST RUNNING                                           ║" -ForegroundColor $Cyan
Write-Host "╠═══════════════════════════════════════════════════════════════╣" -ForegroundColor $Cyan
Write-Host "║  InstantDelay RTMP Ingest:  rtmp://127.0.0.1:1935/live/test   ║" -ForegroundColor $Gray
Write-Host "║  MediaMTX RTMP:              rtmp://127.0.0.1:1936/live/test   ║" -ForegroundColor $Gray
Write-Host "║  MediaMTX HLS:               http://127.0.0.1:8888/live/test/index.m3u8  ║" -ForegroundColor $Gray
Write-Host "║  Web UI:                     http://127.0.0.1:8081             ║" -ForegroundColor $Gray
Write-Host "║  Overlay:                    http://127.0.0.1:8080             ║" -ForegroundColor $Gray
Write-Host "║  Configured Delay:           ${DelaySec}s                          ║" -ForegroundColor $Yellow
Write-Host "║  Test Duration:              ${TestDurationSec}s                          ║" -ForegroundColor $Yellow
Write-Host "╚═══════════════════════════════════════════════════════════════╝" -ForegroundColor $Cyan
Write-Host ""
Write-Host "Watch the burned-in clock in VLC - it should show time ~${DelaySec}s behind real time." -ForegroundColor $Yellow
Write-Host "Press Ctrl+C to stop all processes." -ForegroundColor $Gray
Write-Host ""

# Monitor loop
try {
    $startTime = Get-Date
    while ($true) {
        # Check if ffmpeg finished
        if ($ffmpeg -and $ffmpeg.HasExited) {
            Write-Status "ffmpeg finished. Waiting for delayed stream to complete..."
            Start-Sleep ($DelaySec + 5)
            break
        }

        # Check server status every 5 seconds
        $elapsed = (Get-Date) - $startTime
        if ([math]::Floor($elapsed.TotalSeconds) % 5 -eq 0) {
            try {
                $status = Invoke-RestMethod -Uri "http://127.0.0.1:8081/api/status" -Method Get -TimeoutSec 2 -ErrorAction Stop
                $buf = $status.buffer_stats
                $relay = $status.relay_status[0] ?? @{}
                Write-Debug "Buffer: video=$($buf.video_frames) audio=$($buf.audio_frames) delay_span=$([math]::Round($buf.delay_span_sec,1))s fill=$([math]::Round($buf.fill_percent*100,1))% | Relay: connected=$($relay.connected) bytes_sent=$($relay.bytes_sent) error=$($relay.last_error)"
            } catch { }
        }

        Start-Sleep 1
    }
} finally {
    Cleanup
    Write-Success "Test completed. All processes stopped."
}