param(
    [int]$Tolerance = 2,
    [string]$CapturePrefix = 'fairygui-demo-color-filters',
    [string]$SamplesPrefix = 'color-filter-samples'
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

function To-Linear([double]$Value) {
    if ($Value -le 0.04045) { return $Value / 12.92 }
    return [Math]::Pow(($Value + 0.055) / 1.055, 2.4)
}
function To-Srgb([double]$Value) {
    if ($Value -le 0.0031308) { return $Value * 12.92 }
    return 1.055 * [Math]::Pow($Value, 1.0 / 2.4) - 0.055
}

$total = 0
$failures = 0
$worst = 0
foreach ($suffix in @('', '-updated')) {
    $capturePath = Join-Path $PSScriptRoot "../target/$CapturePrefix$suffix.png"
    $samplesPath = Join-Path $PSScriptRoot "../target/$SamplesPrefix$suffix.csv"
    $bitmap = [System.Drawing.Bitmap]::new([IO.Path]::GetFullPath($capturePath))
    try {
        foreach ($sample in (Import-Csv -LiteralPath $samplesPath)) {
            $actual = $bitmap.GetPixel([int][Math]::Floor([double]$sample.x), [int][Math]::Floor([double]$sample.y))
            $expected = foreach ($channel in @('r', 'g', 'b')) {
                # Bevy UI uses straight-alpha blending against this white panel
                # in linear space, then the screenshot encodes sRGB.
                $value = (To-Linear ([double]$sample.$channel)) * [double]$sample.a + (1.0 - [double]$sample.a)
                [int][Math]::Round(255.0 * (To-Srgb $value))
            }
            $channels = @($actual.R, $actual.G, $actual.B)
            $channelError = 0
            for ($i = 0; $i -lt 3; $i++) { $channelError = [Math]::Max($channelError, [Math]::Abs($channels[$i] - $expected[$i])) }
            $worst = [Math]::Max($worst, $channelError)
            $total++
            if ($channelError -gt $Tolerance) {
                $failures++
                Write-Output "FAIL $suffix panel=$($sample.panel) band=$($sample.band) at=($($sample.x),$($sample.y)) expected=$expected actual=$channels error=$channelError"
            }
        }
    } finally { $bitmap.Dispose() }
}
Write-Output "$total GPU samples, $failures failures, maximum channel error $worst/255 (tolerance $Tolerance)"
if ($failures) { exit 1 }
