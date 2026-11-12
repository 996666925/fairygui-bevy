param([int]$Tolerance = 2)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$total = 0
$failed = 0
$worst = 0
foreach ($suffix in @('-armed','-locked','-complete','-restored','-footer-armed','-footer-locked','-footer-restored')) {
    $bitmap = [System.Drawing.Bitmap]::new([IO.Path]::GetFullPath((Join-Path $PSScriptRoot "../target/fairygui-demo-pull-refresh$suffix.png")))
    try {
        $samples = @(Import-Csv -LiteralPath (Join-Path $PSScriptRoot "../target/pull-refresh-samples$suffix.csv"))
        if ($samples.Count -lt 2) { throw "Missing pull samples: $suffix" }
        foreach ($sample in $samples) {
            $pixel = $bitmap.GetPixel([int][Math]::Floor([double]$sample.x),[int][Math]::Floor([double]$sample.y))
            $actual = @($pixel.R,$pixel.G,$pixel.B)
            $expected = @('r','g','b') | ForEach-Object { [int][Math]::Round(255*[double]$sample.$_) }
            $error = 0
            for ($i=0; $i -lt 3; $i++) { $error = [Math]::Max($error,[Math]::Abs($actual[$i]-$expected[$i])) }
            $total++
            $worst = [Math]::Max($worst,$error)
            if ($error -gt $Tolerance) { $failed++; Write-Output "FAIL $suffix part=$($sample.part) expected=$expected actual=$actual error=$error" }
        }
    } finally { $bitmap.Dispose() }
}
Write-Output "$total pull-refresh GPU samples, $failed failures, maximum channel error $worst/255 (tolerance $Tolerance)"
if ($failed) { exit 1 }
