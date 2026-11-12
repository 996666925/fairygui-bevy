param([int]$Tolerance = 4)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$failures = 0
$panels = 0
foreach ($suffix in @('', '-updated', '-composite', '-composite-filtered', '-disabled')) {
    $bitmap = [System.Drawing.Bitmap]::new([IO.Path]::GetFullPath((Join-Path $PSScriptRoot "../target/fairygui-demo-text-stroke$suffix.png")))
    try {
        foreach ($sample in (Import-Csv -LiteralPath (Join-Path $PSScriptRoot "../target/text-stroke-panels$suffix.csv"))) {
            $stroke = 0
            $fill = 0
            $outside = 0
            $x0 = [Math]::Max(0, [int][Math]::Ceiling([double]$sample.x0))
            $x1 = [Math]::Min($bitmap.Width, [int][Math]::Floor([double]$sample.x1))
            $y0 = [Math]::Max(0, [int][Math]::Ceiling([double]$sample.y0))
            $y1 = [Math]::Min($bitmap.Height, [int][Math]::Floor([double]$sample.y1))
            for ($y = $y0; $y -lt $y1; $y++) {
                for ($x = $x0; $x -lt $x1; $x++) {
                    $color = $bitmap.GetPixel($x, $y)
                    $se = [Math]::Max([Math]::Abs($color.R - [int]$sample.sr), [Math]::Max([Math]::Abs($color.G - [int]$sample.sg), [Math]::Abs($color.B - [int]$sample.sb)))
                    $fe = [Math]::Max([Math]::Abs($color.R - [int]$sample.fr), [Math]::Max([Math]::Abs($color.G - [int]$sample.fg), [Math]::Abs($color.B - [int]$sample.fb)))
                    if ($se -le $Tolerance) { $stroke++ }
                    if ($fe -le $Tolerance) { $fill++ }
                }
            }
            if ($sample.clip -eq '1') {
                for ($y = $y0; $y -lt $y1; $y++) {
                    for ($x = $x1 + 2; $x -lt [Math]::Min($bitmap.Width, $x1 + 40); $x++) {
                        $color = $bitmap.GetPixel($x, $y)
                        if ($color.R -lt 250 -or $color.G -lt 250 -or $color.B -lt 250) { $outside++ }
                    }
                }
            }
            $bad = ($fill -lt 30) -or ($sample.present -eq '1' -and $stroke -lt 30) -or ($sample.present -eq '0' -and $stroke -ne 0) -or ($outside -ne 0)
            if ($bad) { $failures++ }
            $panels++
            Write-Output "$suffix panel=$($sample.panel) stroke=$stroke fill=$fill outside_clip=$outside pass=$(-not $bad)"
        }
    } finally { $bitmap.Dispose() }
}
Write-Output "$panels stroke/foreground panels, $failures failures (channel tolerance $Tolerance)"
if ($failures) { exit 1 }
