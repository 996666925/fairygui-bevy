param([int]$Tolerance = 3)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$total = 0
$failures = 0
$worst = 0
foreach ($suffix in @('', '-updated', '-resized', '-viewport')) {
    $path = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot "../target/fairygui-demo-compositor$suffix.png"))
    $bitmap = [System.Drawing.Bitmap]::new($path)
    try {
        foreach ($sample in (Import-Csv (Join-Path $PSScriptRoot "../target/compositor-samples$suffix.csv"))) {
            $x = [int][Math]::Floor([double]$sample.x)
            $y = [int][Math]::Floor([double]$sample.y)
            if ($x -lt 0 -or $y -lt 0 -or $x -ge $bitmap.Width -or $y -ge $bitmap.Height) { throw "Probe outside capture: $x,$y" }
            $pixel = $bitmap.GetPixel($x, $y)
            $actual = @($pixel.R, $pixel.G, $pixel.B)
            $expected = @([int]$sample.r, [int]$sample.g, [int]$sample.b)
            $error = 0
            for ($i=0; $i -lt 3; $i++) { $error = [Math]::Max($error, [Math]::Abs($actual[$i]-$expected[$i])) }
            $worst = [Math]::Max($worst, $error)
            $total++
            if ($error -gt $Tolerance) {
                $failures++
                Write-Output "FAIL $suffix panel=$($sample.panel) probe=$($sample.probe) at=($x,$y) expected=$expected actual=$actual error=$error"
            }
        }
        foreach ($sample in (Import-Csv (Join-Path $PSScriptRoot "../target/compositor-texts$suffix.csv"))) {
            $matching = 0
            $expected = @([int]$sample.r,[int]$sample.g,[int]$sample.b)
            for ($y=[int][Math]::Ceiling([double]$sample.top); $y -lt [int][Math]::Floor([double]$sample.bottom); $y++) {
                for ($x=[int][Math]::Ceiling([double]$sample.left); $x -lt [int][Math]::Floor([double]$sample.right); $x++) {
                    $pixel=$bitmap.GetPixel($x,$y)
                    if ([Math]::Abs($pixel.R-$expected[0]) -le $Tolerance -and [Math]::Abs($pixel.G-$expected[1]) -le $Tolerance -and [Math]::Abs($pixel.B-$expected[2]) -le $Tolerance) { $matching++ }
                }
            }
            if ($matching -lt 8) { $failures++; Write-Output "FAIL $suffix text panel=$($sample.panel) matching=$matching expected=$expected" }
        }
    } finally { $bitmap.Dispose() }
}
Write-Output "$total compositor GPU probes and 32 text regions, $failures failures, maximum channel error $worst/255 (tolerance $Tolerance)"
if ($failures) { exit 1 }
