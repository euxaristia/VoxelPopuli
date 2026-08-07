# VoxelPopuli Interactive Placement Test Harness
# Launches the game binary, simulates single and continuous right-click placements, and captures a visual screenshot.

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

public class WinInput {
    [DllImport("user32.dll")]
    public static extern void mouse_event(uint dwFlags, int dx, int dy, uint dwData, int dwExtraInfo);

    [DllImport("user32.dll")]
    public static extern bool SetCursorPos(int X, int Y);

    public const uint MOUSEEVENTF_LEFTDOWN = 0x0002;
    public const uint MOUSEEVENTF_LEFTUP = 0x0004;
    public const uint MOUSEEVENTF_RIGHTDOWN = 0x0008;
    public const uint MOUSEEVENTF_RIGHTUP = 0x0010;
    public const uint MOUSEEVENTF_MOVE = 0x0001;
}
"@

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms

Write-Output "Building game binary..."
cargo build --release

if ($LASTEXITCODE -ne 0) {
    Write-Error "Cargo build failed!"
    exit 1
}

Write-Output "Launching VoxelPopuli..."
$proc = Start-Process -FilePath "target\release\VoxelPopuli.exe" -PassThru

# Wait for OpenGL window & chunk streaming
Start-Sleep -Seconds 5

$wshell = New-Object -ComObject WScript.Shell
$activated = $wshell.AppActivate("VoxelPopuli")
Write-Output "Game window activated: $activated"
Start-Sleep -Milliseconds 500

# Send ESC to resume playing state if on pause menu
[System.Windows.Forms.SendKeys]::SendWait("{ESC}")
Start-Sleep -Milliseconds 500

# Position mouse cursor in window center
$screen = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$centerX = [int]($screen.Width / 2)
$centerY = [int]($screen.Height / 2)
[WinInput]::SetCursorPos($centerX, $centerY)
Start-Sleep -Milliseconds 200

# Test 1: Single right-click block placement
Write-Output "Executing single right-click placement..."
[WinInput]::mouse_event([WinInput]::MOUSEEVENTF_RIGHTDOWN, 0, 0, 0, 0)
Start-Sleep -Milliseconds 80
[WinInput]::mouse_event([WinInput]::MOUSEEVENTF_RIGHTUP, 0, 0, 0, 0)
Start-Sleep -Milliseconds 400

# Test 2: Continuous right-click hold & drag placement
Write-Output "Executing continuous hold & drag linear placement..."
[WinInput]::mouse_event([WinInput]::MOUSEEVENTF_RIGHTDOWN, 0, 0, 0, 0)
for ($i = 0; $i -lt 12; $i++) {
    [WinInput]::mouse_event([WinInput]::MOUSEEVENTF_MOVE, 12, 0, 0, 0)
    Start-Sleep -Milliseconds 100
}
[WinInput]::mouse_event([WinInput]::MOUSEEVENTF_RIGHTUP, 0, 0, 0, 0)
Start-Sleep -Milliseconds 500

# Capture verification screenshot
$bmp = New-Object System.Drawing.Bitmap($screen.Width, $screen.Height)
$graphics = [System.Drawing.Graphics]::FromImage($bmp)
$graphics.CopyFromScreen(0, 0, 0, 0, $bmp.Size)

$outPath = "assets\test_placing_result.png"
$bmp.Save($outPath, [System.Drawing.Imaging.ImageFormat]::Png)
$graphics.Dispose()
$bmp.Dispose()
Write-Output "Saved verification screenshot to $outPath"

# Terminate process cleanly
if (-not $proc.HasExited) {
    Stop-Process -Id $proc.Id -Force
}
Write-Output "Test harness completed successfully."
