Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public class W32b {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern int GetWindowText(IntPtr h, StringBuilder sb, int max);
  [DllImport("user32.dll")] public static extern int GetClassName(IntPtr h, StringBuilder sb, int max);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
}
"@
$screenW = 1555
$rows = @()
$cb = [W32b+EnumProc]{
  param($h, $l)
  if ([W32b]::IsWindowVisible($h)) {
    $r = New-Object W32b+RECT
    [W32b]::GetWindowRect($h, [ref]$r) | Out-Null
    # 屏幕右上 220x160 区域内的窗口
    if ($r.Right -gt ($script:screenW - 220) -and $r.Top -lt 160 -and ($r.Right - $r.Left) -gt 0) {
      $pid2 = [uint32]0
      [W32b]::GetWindowThreadProcessId($h, [ref]$pid2) | Out-Null
      $sb = New-Object System.Text.StringBuilder 256
      [W32b]::GetWindowText($h, $sb, 256) | Out-Null
      $cn = New-Object System.Text.StringBuilder 256
      [W32b]::GetClassName($h, $cn, 256) | Out-Null
      $pname = ''
      try { $pname = (Get-Process -Id $pid2 -ErrorAction Stop).ProcessName } catch {}
      $script:rows += [pscustomobject]@{
        Pid=$pid2; Proc=$pname; Title=$sb.ToString(); Class=$cn.ToString();
        L=$r.Left; T=$r.Top; W=($r.Right-$r.Left); H=($r.Bottom-$r.Top)
      }
    }
  }
  return $true
}
[W32b]::EnumWindows($cb, [IntPtr]::Zero) | Out-Null
$rows | Sort-Object W | ForEach-Object {
  Write-Output ("{0} | {1} | [{2}] {3}x{4} @{5},{6} | class={7}" -f $_.Proc, $_.Title, $_.Pid, $_.W, $_.H, $_.L, $_.T, $_.Class)
}
