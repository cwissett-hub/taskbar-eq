# Read-only: sample the REAL taskbar pixels behind the weather widget, DPI-aware.
Add-Type -AssemblyName System.Drawing

Add-Type @'
using System;
using System.Runtime.InteropServices;
public class Dpi {
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
}
'@
[void][Dpi]::SetProcessDPIAware()   # BEFORE any capture, per the DPI rule

# Widget rect measured via UI Automation (physical px)
$x = 1385; $y = 1140; $w = 190; $h = 60

$bmp = New-Object System.Drawing.Bitmap($w, $h)
$g   = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($x, $y, 0, 0, (New-Object System.Drawing.Size($w, $h)))
$g.Dispose()

$out = Join-Path $PSScriptRoot 'taskbar-widget.png'
$bmp.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)

# Verify the capture is real, not the all-black locked-session failure
$hist = @{}
$rs = @(); $gs = @(); $bs = @()
for ($py = 0; $py -lt $h; $py += 2) {
  for ($px = 0; $px -lt $w; $px += 2) {
    $c = $bmp.GetPixel($px, $py)
    $rs += $c.R; $gs += $c.G; $bs += $c.B
    $k = '{0:X2}{1:X2}{2:X2}' -f $c.R, $c.G, $c.B
    if ($hist.ContainsKey($k)) { $hist[$k]++ } else { $hist[$k] = 1 }
  }
}
$bmp.Dispose()

$stat = { param($a) "min={0} max={1} mean={2}" -f ($a | Measure-Object -Minimum).Minimum,
                                                  ($a | Measure-Object -Maximum).Maximum,
                                                  [math]::Round((($a | Measure-Object -Average).Average),1) }
"saved: $out"
"sampled $($rs.Count) pixels"
"  R  $(& $stat $rs)"
"  G  $(& $stat $gs)"
"  B  $(& $stat $bs)"
$distinct = $hist.Keys.Count
"  distinct colours: $distinct  ->  $(if ($distinct -le 2) { 'SUSPECT (blank/black capture)' } else { 'capture looks real' })"
""
"TOP 8 COLOURS (the taskbar surface behind the widget):"
$hist.GetEnumerator() | Sort-Object Value -Descending | Select-Object -First 8 | ForEach-Object {
  "  #{0}   {1,5} px" -f $_.Key, $_.Value
}
""
"--- theme registry ---"
$p = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize'
try {
  $t = Get-ItemProperty $p -ErrorAction Stop
  "AppsUseLightTheme   : $($t.AppsUseLightTheme)   (0 = dark)"
  "SystemUsesLightTheme: $($t.SystemUsesLightTheme)   (0 = dark taskbar)"
} catch { "theme keys unreadable" }
try {
  $d = Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Accent' -ErrorAction Stop
  "AccentColorMenu     : 0x{0:X8}" -f $d.AccentColorMenu
  "StartColorMenu      : 0x{0:X8}" -f $d.StartColorMenu
} catch { "accent keys unreadable" }
try {
  $dw = Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\DWM' -ErrorAction Stop
  "ColorPrevalence     : $($dw.ColorPrevalence)   (1 = accent on taskbar)"
  "EnableTransparency  : $((Get-ItemProperty $p).EnableTransparency)"
} catch { "dwm keys unreadable" }
