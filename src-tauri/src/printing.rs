use serde::Deserialize;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PrintScale {
    Fit,
    Actual,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PrintOrientation {
    Portrait,
    Landscape,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrintOptions {
    pub scale: PrintScale,
    pub orientation: PrintOrientation,
}

#[derive(Debug, Clone, Copy)]
struct PrinterMetrics {
    dpi_x: i32,
    dpi_y: i32,
    printable_width: i32,
    printable_height: i32,
    physical_width: i32,
    physical_height: i32,
    offset_x: i32,
    offset_y: i32,
}

#[derive(Debug, PartialEq)]
struct PagePlacement {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

// Coordinates are device pixels relative to the printable origin, not the paper edge.
fn page_placement(
    width: f32,
    height: f32,
    printer: PrinterMetrics,
    scale: PrintScale,
) -> Result<PagePlacement, String> {
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return Err("Cannot print a page with invalid dimensions.".into());
    }
    if printer.dpi_x <= 0
        || printer.dpi_y <= 0
        || printer.printable_width <= 0
        || printer.printable_height <= 0
        || printer.physical_width < printer.printable_width
        || printer.physical_height < printer.printable_height
        || printer.offset_x < 0
        || printer.offset_y < 0
    {
        return Err("The printer reported invalid paper dimensions or resolution.".into());
    }
    let natural_width = f64::from(width) * f64::from(printer.dpi_x) / 72.0;
    let natural_height = f64::from(height) * f64::from(printer.dpi_y) / 72.0;
    let factor = match scale {
        PrintScale::Fit => (f64::from(printer.printable_width) / natural_width)
            .min(f64::from(printer.printable_height) / natural_height),
        PrintScale::Actual => 1.0,
    };
    let width = (natural_width * factor).round();
    let height = (natural_height * factor).round();
    if width < 1.0 || height < 1.0 || width > f64::from(i32::MAX) || height > f64::from(i32::MAX) {
        return Err("This page is too large or too narrow to print at the selected scale.".into());
    }
    let (x, y) = match scale {
        PrintScale::Fit => (
            (f64::from(printer.printable_width) - width) / 2.0,
            (f64::from(printer.printable_height) - height) / 2.0,
        ),
        PrintScale::Actual => (
            (f64::from(printer.physical_width) - width) / 2.0 - f64::from(printer.offset_x),
            (f64::from(printer.physical_height) - height) / 2.0 - f64::from(printer.offset_y),
        ),
    };
    Ok(PagePlacement {
        x: x.round() as i32,
        y: y.round() as i32,
        width: width as i32,
        height: height as i32,
    })
}

const MAX_RASTER_PIXELS: f64 = 16_000_000.0;
const MAX_RASTER_HEIGHT: f64 = 12_000.0;

fn render_width(
    page_width: f32,
    page_height: f32,
    placement: &PagePlacement,
    printer: PrinterMetrics,
) -> Result<u32, String> {
    let ratio = f64::from(page_height) / f64::from(page_width);
    if !ratio.is_finite() || ratio <= 0.0 || printer.dpi_x <= 0 {
        return Err("Cannot render a print page with invalid dimensions.".into());
    }
    // PdfEngine accepts widths from 64 to 2400. Keep tall custom pages bounded too.
    let width = (f64::from(placement.width) / f64::from(printer.dpi_x) * 300.0)
        .max(64.0)
        .min(2400.0)
        .min((MAX_RASTER_PIXELS / ratio).sqrt())
        .min(MAX_RASTER_HEIGHT / ratio)
        .floor();
    if width < 64.0 {
        return Err("This page is too tall or narrow to print safely.".into());
    }
    Ok(width as u32)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn printer() -> PrinterMetrics {
        PrinterMetrics {
            dpi_x: 600,
            dpi_y: 300,
            printable_width: 4800,
            printable_height: 3000,
            physical_width: 5100,
            physical_height: 3300,
            offset_x: 100,
            offset_y: 75,
        }
    }
    #[test]
    fn fit_uses_printable_area_and_preserves_physical_aspect_at_unequal_dpi() {
        let p = page_placement(612.0, 792.0, printer(), PrintScale::Fit).unwrap();
        assert_eq!(
            p,
            PagePlacement {
                x: 82,
                y: 0,
                width: 4636,
                height: 3000
            }
        );
    }
    #[test]
    fn actual_size_centers_on_physical_sheet_and_accounts_for_hard_margins() {
        let p = page_placement(612.0, 792.0, printer(), PrintScale::Actual).unwrap();
        assert_eq!(
            p,
            PagePlacement {
                x: -100,
                y: -75,
                width: 5100,
                height: 3300
            }
        );
        let smaller = page_placement(306.0, 396.0, printer(), PrintScale::Actual).unwrap();
        assert_eq!(
            smaller,
            PagePlacement {
                x: 1175,
                y: 750,
                width: 2550,
                height: 1650
            }
        );
    }
    #[test]
    fn invalid_dimensions_and_printer_metrics_are_rejected() {
        for width in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert!(page_placement(width, 792.0, printer(), PrintScale::Fit).is_err());
        }
        let mut broken = printer();
        broken.dpi_x = 0;
        assert!(page_placement(612.0, 792.0, broken, PrintScale::Fit).is_err());
        assert!(page_placement(f32::MAX, 1.0, printer(), PrintScale::Actual).is_err());
    }
    #[test]
    fn raster_resolution_tracks_physical_size_and_limits_large_pages() {
        let small = page_placement(144.0, 144.0, printer(), PrintScale::Actual).unwrap();
        assert_eq!(render_width(144.0, 144.0, &small, printer()).unwrap(), 600);
        let letter = page_placement(612.0, 792.0, printer(), PrintScale::Actual).unwrap();
        assert_eq!(
            render_width(612.0, 792.0, &letter, printer()).unwrap(),
            2400
        );
        let tall = page_placement(72.0, 7200.0, printer(), PrintScale::Actual).unwrap();
        let width = render_width(72.0, 7200.0, &tall, printer()).unwrap();
        assert!(u64::from(width) * u64::from(width) * 100 <= 16_000_000);
        assert!(width * 100 <= 12000);
        assert!(render_width(1.0, 1_000_000.0, &letter, printer()).is_err());
    }
}

struct PreparedPrint {
    engine: crate::PdfEngine,
    document: crate::DocumentInfo,
    // This directory outlives the opened PDF; Drop closes it before removing files.
    _directory: tempfile::TempDir,
}
impl PreparedPrint {
    fn new(engine: crate::PdfEngine, request: crate::ExportRequest) -> Result<Self, String> {
        if request.pages.is_empty() {
            return Err("Choose at least one page to print.".into());
        }
        let directory = tempfile::Builder::new()
            .prefix("folio-print-")
            .tempdir()
            .map_err(|e| e.to_string())?;
        let path = directory.path().join("Folio print.pdf");
        engine
            .export_pdf(request, &path)
            .map_err(|e| e.to_string())?;
        let document = engine
            .open_document_for_printing(&path)
            .map_err(|e| e.to_string())?;
        Ok(Self {
            engine,
            document,
            _directory: directory,
        })
    }
}
impl Drop for PreparedPrint {
    fn drop(&mut self) {
        let _ = self.engine.close_document(&self.document.id);
        // TempDir removes the files after this method returns.
    }
}

fn png_to_bgra(png: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    let mut reader =
        image::ImageReader::with_format(std::io::Cursor::new(png), image::ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(2400);
    limits.max_image_height = Some(MAX_RASTER_HEIGHT as u32);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|e| format!("Could not decode the print page: {e}"))?
        .into_rgba8();
    let (width, height) = image.dimensions();
    if u64::from(width) * u64::from(height) > MAX_RASTER_PIXELS as u64 {
        return Err("The print page exceeded the raster memory limit.".into());
    }
    let mut pixels = image.into_raw();
    for pixel in pixels.chunks_exact_mut(4) {
        let alpha = u32::from(pixel[3]);
        for channel in &mut pixel[..3] {
            *channel = ((u32::from(*channel) * alpha + 255 * (255 - alpha) + 127) / 255) as u8;
        }
        pixel.swap(0, 2);
        pixel[3] = 255;
    }
    Ok((width, height, pixels))
}
#[cfg(test)]
#[path = "../tests/support/native_printing_cases.rs"]
mod pipeline_tests;

/// Opens the native printer dialog and prints the edited selected pages.
/// Run on a blocking thread. Returns false when the user cancels.
pub fn print_document(
    engine: crate::PdfEngine,
    request: crate::ExportRequest,
    options: PrintOptions,
) -> Result<bool, String> {
    #[cfg(windows)]
    {
        let prepared = PreparedPrint::new(engine, request)?;
        windows::show_dialog_and_print(&prepared, options)
    }
    #[cfg(not(windows))]
    {
        let _ = (engine, request, options);
        Err("Native printing is currently available on Windows.".into())
    }
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::mem::size_of;
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::Foundation::{
        GetLastError, GlobalFree, ERROR_CANCELLED, ERROR_PRINT_CANCELLED,
    };
    use windows_sys::Win32::Graphics::Gdi::*;
    use windows_sys::Win32::Storage::Xps::{
        AbortDoc, EndDoc, EndPage, StartDocW, StartPage, DOCINFOW,
    };
    use windows_sys::Win32::System::Memory::{GlobalLock, GlobalUnlock};
    use windows_sys::Win32::UI::Controls::Dialogs::*;

    // The dialog can replace all three handles, including when it is cancelled.
    struct PrinterDialog(PRINTDLGW);
    impl Drop for PrinterDialog {
        fn drop(&mut self) {
            unsafe {
                if !self.0.hDC.is_null() {
                    DeleteDC(self.0.hDC);
                }
                if !self.0.hDevMode.is_null() {
                    GlobalFree(self.0.hDevMode);
                }
                if !self.0.hDevNames.is_null() {
                    GlobalFree(self.0.hDevNames);
                }
            }
        }
    }

    pub(super) fn show_dialog_and_print(
        prepared: &PreparedPrint,
        options: PrintOptions,
    ) -> Result<bool, String> {
        let mut dialog = PrinterDialog(PRINTDLGW {
            lStructSize: size_of::<PRINTDLGW>() as u32,
            Flags: PD_RETURNDEFAULT,
            nCopies: 1,
            ..Default::default()
        });
        // Read defaults without printing. Never call StartDoc before the visible dialog confirms.
        unsafe {
            PrintDlgW(&mut dialog.0);
            if !dialog.0.hDevMode.is_null() {
                let mode = GlobalLock(dialog.0.hDevMode).cast::<DEVMODEW>();
                if !mode.is_null() {
                    (*mode).dmFields |= DM_ORIENTATION;
                    (*mode).Anonymous1.Anonymous1.dmOrientation = match options.orientation {
                        PrintOrientation::Portrait => DMORIENT_PORTRAIT as i16,
                        PrintOrientation::Landscape => DMORIENT_LANDSCAPE as i16,
                    };
                    GlobalUnlock(dialog.0.hDevMode);
                }
            }
            // Page selection is already applied to the exported PDF. Printer properties
            // control paper and final orientation; the driver handles copies/collation.
            dialog.0.Flags = PD_RETURNDC
                | PD_USEDEVMODECOPIESANDCOLLATE
                | PD_NOSELECTION
                | PD_NOPAGENUMS
                | PD_HIDEPRINTTOFILE;
            dialog.0.nMinPage = 1;
            dialog.0.nMaxPage = 1;
            dialog.0.nFromPage = 1;
            dialog.0.nToPage = 1;
            if PrintDlgW(&mut dialog.0) == 0 {
                let error = CommDlgExtendedError();
                return if error == 0 {
                    Ok(false)
                } else {
                    Err(format!(
                        "Windows could not open the print dialog (error 0x{error:08X})."
                    ))
                };
            }
        }
        if dialog.0.hDC.is_null() {
            return Err("The selected printer did not provide a print device.".into());
        }
        spool_document(prepared, dialog.0.hDC, options.scale, None)
    }

    struct PrintJob {
        dc: HDC,
        active: bool,
    }
    impl Drop for PrintJob {
        fn drop(&mut self) {
            if self.active {
                unsafe {
                    AbortDoc(self.dc);
                }
            }
        }
    }

    fn printer_error(action: &str) -> String {
        format!(
            "The printer could not {action}: {}",
            std::io::Error::last_os_error()
        )
    }

    pub(super) fn spool_document(
        prepared: &PreparedPrint,
        dc: HDC,
        scale: PrintScale,
        output: Option<&[u16]>,
    ) -> Result<bool, String> {
        // This function borrows the DC. The dialog (or virtual-printer test) owns it.
        let printer = unsafe {
            PrinterMetrics {
                dpi_x: GetDeviceCaps(dc, LOGPIXELSX as i32),
                dpi_y: GetDeviceCaps(dc, LOGPIXELSY as i32),
                printable_width: GetDeviceCaps(dc, HORZRES as i32),
                printable_height: GetDeviceCaps(dc, VERTRES as i32),
                physical_width: GetDeviceCaps(dc, PHYSICALWIDTH as i32),
                physical_height: GetDeviceCaps(dc, PHYSICALHEIGHT as i32),
                offset_x: GetDeviceCaps(dc, PHYSICALOFFSETX as i32),
                offset_y: GetDeviceCaps(dc, PHYSICALOFFSETY as i32),
            }
        };
        // Validate every page before a printer job starts; this only stores geometry.
        let pages = prepared
            .document
            .pages
            .iter()
            .map(|page| {
                let placement = page_placement(page.width, page.height, printer, scale)?;
                let width = render_width(page.width, page.height, &placement, printer)?;
                Ok((placement, width))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let title: Vec<u16> = "Folio document\0".encode_utf16().collect();
        let info = DOCINFOW {
            cbSize: size_of::<DOCINFOW>() as i32,
            lpszDocName: title.as_ptr(),
            lpszOutput: output.map_or(null(), |path| path.as_ptr()),
            ..Default::default()
        };
        unsafe {
            if StartDocW(dc, &info) <= 0 {
                let error = GetLastError();
                if error == ERROR_CANCELLED || error == ERROR_PRINT_CANCELLED {
                    return Ok(false);
                }
                return Err(printer_error("start the document"));
            }
        }
        let mut job = PrintJob { dc, active: true };
        for (index, (placement, target_width)) in pages.iter().enumerate() {
            let png = prepared
                .engine
                .render_page(&prepared.document.id, index, *target_width)
                .map_err(|e| format!("Could not render print page {}: {e}", index + 1))?;
            let (width, height, pixels) = png_to_bgra(&png)?;
            drop(png);
            let bitmap = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width as i32,
                    biHeight: -(height as i32), // top-down BGRA rows from the PNG
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB,
                    ..Default::default()
                },
                ..Default::default()
            };
            unsafe {
                if StartPage(dc) <= 0 {
                    return Err(printer_error("start a page"));
                }
                if SetMapMode(dc, MM_TEXT) == 0
                    || SetStretchBltMode(dc, HALFTONE) == 0
                    || SetBrushOrgEx(dc, 0, 0, null_mut()) == 0
                {
                    return Err(printer_error("configure page rendering"));
                }
                let copied = StretchDIBits(
                    dc,
                    placement.x,
                    placement.y,
                    placement.width,
                    placement.height,
                    0,
                    0,
                    width as i32,
                    height as i32,
                    pixels.as_ptr().cast(),
                    &bitmap,
                    DIB_RGB_COLORS,
                    SRCCOPY,
                );
                if copied == 0 || copied == GDI_ERROR as i32 {
                    return Err(printer_error("draw a page"));
                }
                if EndPage(dc) <= 0 {
                    return Err(printer_error("finish a page"));
                }
            }
            // The raster is dropped here; the next page is never resident at the same time.
        }
        unsafe {
            if EndDoc(dc) <= 0 {
                return Err(printer_error("finish the document"));
            }
        }
        job.active = false;
        Ok(true)
    }
}
