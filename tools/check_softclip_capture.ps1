param([int]$Tolerance = 4)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$total = 0
$failures = 0
$worst = 0
foreach ($suffix in @('', '-scrolled', '-viewport', '-disabled', '-margins')) {
    $capturePath = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot "../target/fairygui-demo-softclip$suffix.png"))
    $bitmap = [System.Drawing.Bitmap]::new($capturePath)
    try {
        foreach ($sample in (Import-Csv (Join-Path $PSScriptRoot "../target/softclip-samples$suffix.csv"))) {
            $x = [int][Math]::Floor([double]$sample.x)
            $y = [int][Math]::Floor([double]$sample.y)
            $pixel = $bitmap.GetPixel($x, $y)
            $actual = @($pixel.R, $pixel.G, $pixel.B)
            $expected = @([int]$sample.r, [int]$sample.g, [int]$sample.b)
            $channelError = 0
            for ($i=0; $i -lt 3; $i++) { $channelError = [Math]::Max($channelError, [Math]::Abs($actual[$i]-$expected[$i])) }
            $worst = [Math]::Max($worst, $channelError)
            $total++
            if ($channelError -gt $Tolerance) {
                $failures++
                Write-Output "FAIL $suffix panel=$($sample.panel) at=($x,$y) expected=$expected actual=$actual error=$channelError"
            }
        }
    } finally { $bitmap.Dispose() }
}
Write-Output "$total soft clip GPU samples, $failures failures, maximum channel error $worst/255 (tolerance $Tolerance)"
if ($failures) { exit 1 }
