Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Text;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public class W32 {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern int GetWindowText(IntPtr h, StringBuilder sb, int max);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
}
"@
$target = [uint32]9888
$found = @()
$cb = [W32+EnumProc]{
  param($h, $l)
  $pid2 = [uint32]0
  [W32]::GetWindowThreadProcessId($h, [ref]$pid2) | Out-Null
  if ($pid2 -eq $script:target -and [W32]::IsWindowVisible($h)) {
    $sb = New-Object System.Text.StringBuilder 256
    [W32]::GetWindowText($h, $sb, 256) | Out-Null
    $r = New-Object W32+RECT
    [W32]::GetWindowRect($h, [ref]$r) | Out-Null
    $script:found += [pscustomobject]@{ H=$h; Title=$sb.ToString(); L=$r.Left; T=$r.Top; W=($r.Right-$r.Left); Hh=($r.Bottom-$r.Top) }
  }
  return $true
}
[W32]::EnumWindows($cb, [IntPtr]::Zero) | Out-Null
$found | ForEach-Object { Write-Output ("win: {0} [{1}] {2}x{3} @{4},{5}" -f $_.Title, $_.H, $_.W, $_.Hh, $_.L, $_.T) }
# 取面积最大的窗口
$main = $found | Sort-Object { $_.W * $_.Hh } -Descending | Select-Object -First 1
if (-not $main) { Write-Output 'no window found'; exit 1 }
[W32]::SetForegroundWindow($main.H) | Out-Null
Start-Sleep -Milliseconds 800
$bmp = New-Object System.Drawing.Bitmap ([int]$main.W), ([int]$main.Hh)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen([int]$main.L, [int]$main.T, 0, 0, (New-Object System.Drawing.Size ([int]$main.W), ([int]$main.Hh)))
$bmp.Save('F:\Vibe Coding\Pigeon\_win.png', [System.Drawing.Imaging.ImageFormat]::Png)
Write-Output 'saved _win.png'
