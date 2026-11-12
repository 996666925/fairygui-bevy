param(
    [int]$Tolerance = 2,
    [string]$CapturePrefix = 'fairygui-demo-virtual-list',
    [string]$SamplesPrefix = 'virtual-list-samples',
    [string[]]$Suffixes = @('', '-500', '-bottom', '-top', '-resized')
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$total = 0
$failed = 0
$worst = 0
foreach ($suffix in $Suffixes) {
    $bitmap = [System.Drawing.Bitmap]::new([IO.Path]::GetFullPath((Join-Path $PSScriptRoot "../target/$CapturePrefix$suffix.png")))
    try {
        $samples = @(Import-Csv -LiteralPath (Join-Path $PSScriptRoot "../target/$SamplesPrefix$suffix.csv"))
        if ($samples.Count -lt 2) { throw "Missing virtual row samples for $suffix" }
        foreach ($sample in $samples) {
            $x = [int][Math]::Floor([double]$sample.x)
            $y = [int][Math]::Floor([double]$sample.y)
            $actual = $bitmap.GetPixel($x, $y)
            $channels = @($actual.R, $actual.G, $actual.B)
            $expected = @('r', 'g', 'b') | ForEach-Object { [int][Math]::Round(255.0 * [double]$sample.$_) }
            $error = 0
            for ($i = 0; $i -lt 3; $i++) { $error = [Math]::Max($error, [Math]::Abs($channels[$i] - $expected[$i])) }
            $total++
            $worst = [Math]::Max($worst, $error)
            if ($error -gt $Tolerance) {
                $failed++
                Write-Output "FAIL $suffix row=$($sample.row) at=($x,$y) expected=$expected actual=$channels error=$error"
            }
        }
    } finally { $bitmap.Dispose() }
}
Write-Output "$total virtual row GPU samples, $failed failures, maximum channel error $worst/255 (tolerance $Tolerance)"
if ($failed) { exit 1 }
