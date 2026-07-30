# Read-only probe: find the Win11 taskbar and the Widgets/weather button geometry.
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes

Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public class TB {
  [DllImport("user32.dll", SetLastError=true)] public static extern IntPtr FindWindow(string c, string w);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
}
'@

$tray = [TB]::FindWindow("Shell_TrayWnd", $null)
if ($tray -eq [IntPtr]::Zero) { "Shell_TrayWnd NOT FOUND"; exit 1 }

$r = New-Object TB+RECT
[void][TB]::GetWindowRect($tray, [ref]$r)
$dpi = [TB]::GetDpiForWindow($tray)

"TASKBAR  hwnd=0x{0:X}" -f [int64]$tray
"  rect   L={0} T={1} R={2} B={3}" -f $r.Left, $r.Top, $r.Right, $r.Bottom
"  size   {0} x {1} px (physical)" -f ($r.Right - $r.Left), ($r.Bottom - $r.Top)
"  dpi    {0}  (scale {1}%)" -f $dpi, [math]::Round($dpi / 96 * 100)
""

# Walk the taskbar's UI Automation subtree looking for the widgets / weather button.
$tbEl = [System.Windows.Automation.AutomationElement]::FromHandle($tray)
$all  = $tbEl.FindAll(
          [System.Windows.Automation.TreeScope]::Descendants,
          [System.Windows.Automation.Condition]::TrueCondition)

"UI AUTOMATION DESCENDANTS ({0} total) - named elements only:" -f $all.Count
foreach ($el in $all) {
  try {
    $n = $el.Current.Name
    if ([string]::IsNullOrWhiteSpace($n)) { continue }
    $b = $el.Current.BoundingRectangle
    "  [{0,-22}] {1,-42} X={2,5} Y={3,5} W={4,4} H={5,4}" -f `
      $el.Current.ControlType.ProgrammaticName.Replace('ControlType.',''),
      $n.Substring(0, [math]::Min(42, $n.Length)),
      [int]$b.X, [int]$b.Y, [int]$b.Width, [int]$b.Height
  } catch {}
}
