Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public class W32c {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern int GetWindowText(IntPtr h, StringBuilder sb, int max);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
}
"@
[W32c]::SetProcessDPIAware() | Out-Null
$proc = Get-Process pigeon -ErrorAction Stop | Select-Object -First 1
$target = [uint32]$proc.Id
Write-Output ("pid: " + $target)
$found = @()
$cb = [W32c+EnumProc]{
  param($h, $l)
  $p2 = [uint32]0
  [W32c]::GetWindowThreadProcessId($h, [ref]$p2) | Out-Null
  if ($p2 -eq $script:target -and [W32c]::IsWindowVisible($h)) {
    $sb = New-Object System.Text.StringBuilder 256
    [W32c]::GetWindowText($h, $sb, 256) | Out-Null
    $r = New-Object W32c+RECT
    [W32c]::GetWindowRect($h, [ref]$r) | Out-Null
    if (($r.Right - $r.Left) -gt 100) {
      $script:found += [pscustomobject]@{ H=$h; Title=$sb.ToString(); L=$r.Left; T=$r.Top; W=($r.Right-$r.Left); Hh=($r.Bottom-$r.Top) }
    }
  }
  return $true
}
[W32c]::EnumWindows($cb, [IntPtr]::Zero) | Out-Null
$found | ForEach-Object { Write-Output ("win: [{0}] {1}x{2} @{3},{4}" -f $_.Title, $_.W, $_.Hh, $_.L, $_.T) }
$main = $found | Sort-Object { $_.W * $_.Hh } -Descending | Select-Object -First 1
if (-not $main) { Write-Output 'no window'; exit 1 }
[W32c]::SetForegroundWindow($main.H) | Out-Null
Start-Sleep -Milliseconds 900
$bmp = New-Object System.Drawing.Bitmap ([int]$main.W), ([int]$main.Hh)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen([int]$main.L, [int]$main.T, 0, 0, (New-Object System.Drawing.Size ([int]$main.W), ([int]$main.Hh)))
$bmp.Save('F:\Vibe Coding\Pigeon\_win.png', [System.Drawing.Imaging.ImageFormat]::Png)
Write-Output "saved ${($main.W)}x${($main.Hh)}"
