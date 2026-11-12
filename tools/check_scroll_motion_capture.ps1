param([int]$Tolerance = 2)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$total = 0
$failed = 0
$worst = 0
foreach ($suffix in @('-release-snap', '-pull-start', '-bounce-start', '-pull-end', '-bounce-end')) {
    $bitmap = [System.Drawing.Bitmap]::new([IO.Path]::GetFullPath((Join-Path $PSScriptRoot "../target/fairygui-demo-list-bars$suffix.png")))
    try {
        $samples = @(Import-Csv -LiteralPath (Join-Path $PSScriptRoot "../target/list-motion-samples$suffix.csv"))
        if ($samples.Count -lt 4) { throw "Missing scroll motion samples: $suffix" }
        foreach ($sample in $samples) {
            $pixel = $bitmap.GetPixel([int][Math]::Floor([double]$sample.x), [int][Math]::Floor([double]$sample.y))
            $actual = @($pixel.R, $pixel.G, $pixel.B)
            $total++
            if ($sample.kind -eq 'outside') {
                foreach ($stripe in @(@(51,153,204), @(204,102,51))) {
                    $error = 0
                    for ($i=0; $i -lt 3; $i++) { $error = [Math]::Max($error,[Math]::Abs($actual[$i]-$stripe[$i])) }
                    if ($error -le $Tolerance) { $failed++; Write-Output "FAIL $suffix stripe outside clip at $($sample.x),$($sample.y)" }
                }
            } else {
                $expected = @('r','g','b') | ForEach-Object { [int][Math]::Round(255*[double]$sample.$_) }
                $error = 0
                for ($i=0; $i -lt 3; $i++) { $error = [Math]::Max($error,[Math]::Abs($actual[$i]-$expected[$i])) }
                $worst = [Math]::Max($worst,$error)
                if ($error -gt $Tolerance) { $failed++; Write-Output "FAIL $suffix row=$($sample.row) expected=$expected actual=$actual error=$error" }
            }
        }
    } finally { $bitmap.Dispose() }
}
Write-Output "$total scroll motion row/clip GPU samples, $failed failures, maximum row channel error $worst/255 (tolerance $Tolerance)"
if ($failed) { exit 1 }
