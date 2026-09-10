$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$folioRoot = Split-Path -Parent $PSScriptRoot
$iconDir = Join-Path $folioRoot 'src-tauri/icons'
[IO.Directory]::CreateDirectory($iconDir) | Out-Null
$bitmap = [Drawing.Bitmap]::new(256, 256)
$graphics = [Drawing.Graphics]::FromImage($bitmap)
$graphics.SmoothingMode = [Drawing.Drawing2D.SmoothingMode]::AntiAlias
$background = [Drawing.SolidBrush]::new([Drawing.ColorTranslator]::FromHtml('#b85338'))
$paper = [Drawing.SolidBrush]::new([Drawing.ColorTranslator]::FromHtml('#fff9ee'))
$fold = [Drawing.SolidBrush]::new([Drawing.ColorTranslator]::FromHtml('#e8c9b6'))
$pen = [Drawing.Pen]::new($background, 10)
$pen.StartCap = [Drawing.Drawing2D.LineCap]::Round
$pen.EndCap = [Drawing.Drawing2D.LineCap]::Round
$shape = [Drawing.Drawing2D.GraphicsPath]::new()
try {
    $shape.AddArc(0, 0, 112, 112, 180, 90)
    $shape.AddArc(143, 0, 112, 112, 270, 90)
    $shape.AddArc(143, 143, 112, 112, 0, 90)
    $shape.AddArc(0, 143, 112, 112, 90, 90)
    $shape.CloseFigure()
    $graphics.FillPath($background, $shape)
    $graphics.FillPolygon($paper, [Drawing.Point[]]@(
        [Drawing.Point]::new(65,43),[Drawing.Point]::new(149,43),
        [Drawing.Point]::new(192,87),[Drawing.Point]::new(192,215),
        [Drawing.Point]::new(65,215)
    ))
    $graphics.FillPolygon($fold, [Drawing.Point[]]@(
        [Drawing.Point]::new(149,43),[Drawing.Point]::new(149,87),
        [Drawing.Point]::new(192,87)
    ))
    $graphics.DrawLine($pen,91,116,156,116)
    $graphics.DrawLine($pen,91,142,144,142)
    $graphics.DrawLine($pen,91,168,128,168)
    $pngPath = Join-Path $iconDir 'icon.png'
    $bitmap.Save($pngPath,[Drawing.Imaging.ImageFormat]::Png)
    $png = [IO.File]::ReadAllBytes($pngPath)
    $stream = [IO.File]::Create((Join-Path $iconDir 'icon.ico'))
    $writer = [IO.BinaryWriter]::new($stream)
    try {
        $writer.Write([uint16]0)
        $writer.Write([uint16]1)
        $writer.Write([uint16]1)
        $writer.Write([byte]0)
        $writer.Write([byte]0)
        $writer.Write([byte]0)
        $writer.Write([byte]0)
        $writer.Write([uint16]1)
        $writer.Write([uint16]32)
        $writer.Write([uint32]$png.Length)
        $writer.Write([uint32]22)
        $writer.Write($png)
    } finally { $writer.Dispose() }
} finally {
    $shape.Dispose()
    $pen.Dispose()
    $background.Dispose()
    $paper.Dispose()
    $fold.Dispose()
    $graphics.Dispose()
    $bitmap.Dispose()
}
Write-Host "Icons generated in $iconDir"
