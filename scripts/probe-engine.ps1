param([string]$PdfPath = '', [int]$RenderWidth = 1200)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$folioRoot = Split-Path -Parent $PSScriptRoot
if (!$PdfPath) { $PdfPath = Join-Path $folioRoot 'examples/Welcome to Folio.pdf' }
$dll = (Join-Path $folioRoot 'src-tauri/resources/pdfium/pdfium.dll').Replace('\','/')
if (!(Test-Path -LiteralPath $dll)) { throw 'Run scripts/fetch-pdfium.ps1 first.' }
$source = @"
using System;
using System.Runtime.InteropServices;
public static class FolioEngineProbe {
    private const string Dll = "$dll";
    [DllImport(Dll)] public static extern void FPDF_InitLibrary();
    [DllImport(Dll)] public static extern void FPDF_DestroyLibrary();
    [DllImport(Dll)] public static extern IntPtr FPDF_LoadDocument([MarshalAs(UnmanagedType.LPUTF8Str)] string path, string password);
    [DllImport(Dll)] public static extern int FPDF_GetPageCount(IntPtr document);
    [DllImport(Dll)] public static extern IntPtr FPDF_LoadPage(IntPtr document, int index);
    [DllImport(Dll)] public static extern float FPDF_GetPageWidthF(IntPtr page);
    [DllImport(Dll)] public static extern float FPDF_GetPageHeightF(IntPtr page);
    [DllImport(Dll)] public static extern void FPDF_ClosePage(IntPtr page);
    [DllImport(Dll)] public static extern void FPDF_CloseDocument(IntPtr document);
    [DllImport(Dll)] public static extern IntPtr FPDFBitmap_Create(int width, int height, int alpha);
    [DllImport(Dll)] public static extern void FPDFBitmap_FillRect(IntPtr bitmap, int left, int top, int width, int height, uint color);
    [DllImport(Dll)] public static extern IntPtr FPDFBitmap_GetBuffer(IntPtr bitmap);
    [DllImport(Dll)] public static extern int FPDFBitmap_GetStride(IntPtr bitmap);
    [DllImport(Dll)] public static extern void FPDFBitmap_Destroy(IntPtr bitmap);
    [DllImport(Dll)] public static extern void FPDF_RenderPageBitmap(IntPtr bitmap, IntPtr page, int x, int y, int width, int height, int rotation, int flags);
}
"@
Add-Type -TypeDefinition $source
$artifactDir = Join-Path $folioRoot 'artifacts/engine-probe'
[IO.Directory]::CreateDirectory($artifactDir) | Out-Null
[FolioEngineProbe]::FPDF_InitLibrary()
$document = [IntPtr]::Zero
try {
    $timer = [Diagnostics.Stopwatch]::StartNew()
    $document = [FolioEngineProbe]::FPDF_LoadDocument($PdfPath, $null)
    if ($document -eq [IntPtr]::Zero) { throw "PDFium could not open $PdfPath" }
    $loadMs = $timer.Elapsed.TotalMilliseconds
    $count = [FolioEngineProbe]::FPDF_GetPageCount($document)
    $pages = @()
    for ($i = 0; $i -lt $count; $i++) {
        $page = [FolioEngineProbe]::FPDF_LoadPage($document, $i)
        $bitmap = [IntPtr]::Zero
        try {
            $width = [FolioEngineProbe]::FPDF_GetPageWidthF($page)
            $height = [FolioEngineProbe]::FPDF_GetPageHeightF($page)
            $pixelHeight = [int][Math]::Round($RenderWidth * $height / $width)
            $bitmap = [FolioEngineProbe]::FPDFBitmap_Create($RenderWidth, $pixelHeight, 1)
            [FolioEngineProbe]::FPDFBitmap_FillRect($bitmap, 0, 0, $RenderWidth, $pixelHeight, [uint32]::MaxValue)
            $timer.Restart()
            [FolioEngineProbe]::FPDF_RenderPageBitmap($bitmap, $page, 0, 0, $RenderWidth, $pixelHeight, 0, 1)
            $renderMs = $timer.Elapsed.TotalMilliseconds
            $image = [Drawing.Bitmap]::new($RenderWidth, $pixelHeight,
                [FolioEngineProbe]::FPDFBitmap_GetStride($bitmap),
                [Drawing.Imaging.PixelFormat]::Format32bppArgb,
                [FolioEngineProbe]::FPDFBitmap_GetBuffer($bitmap))
            try { $image.Save((Join-Path $artifactDir "page-$($i+1).png"), [Drawing.Imaging.ImageFormat]::Png) }
            finally { $image.Dispose() }
            $pages += [ordered]@{ page=$i+1; width=$width; height=$height; renderMs=[Math]::Round($renderMs,2) }
        } finally {
            if ($bitmap -ne [IntPtr]::Zero) { [FolioEngineProbe]::FPDFBitmap_Destroy($bitmap) }
            [FolioEngineProbe]::FPDF_ClosePage($page)
        }
    }
    $result = [ordered]@{file=$PdfPath; pageCount=$count; loadMs=[Math]::Round($loadMs,2); renderWidth=$RenderWidth; pages=$pages; note='Direct engine probe, not end-to-end app performance.'}
    $json = $result | ConvertTo-Json -Depth 4
    $json | Set-Content -LiteralPath (Join-Path $artifactDir 'results.json')
    Write-Output $json
} finally {
    if ($document -ne [IntPtr]::Zero) { [FolioEngineProbe]::FPDF_CloseDocument($document) }
    [FolioEngineProbe]::FPDF_DestroyLibrary()
}
