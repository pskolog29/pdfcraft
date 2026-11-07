//! Arif Pdf Editör: printing through classic GDI, used when the Direct2D / XPS print path
//! (`Windows.Data.Pdf` drawn into an `ID2D1PrintControl`) fails for a driver. Some v3 (GDI)
//! drivers — seen with a Brother MFC-L8690CDW — refuse that path with "Render error: No such
//! interface supported (0x80040003)" before any page is accepted, and the job is cancelled.
//! Here every sheet of the print-ready PDF is rendered by PdfCraft's own renderer (the one the
//! preview uses) and sent to the printer's device context with `StretchDIBits`, with copies,
//! collation, two-sided, colour and orientation set in the driver's own DEVMODE. Nothing but
//! the GDI spooler API is used, so it works with every installed driver.

use std::sync::Arc;

use pdfcraft_engine::print::spool;
use pdfcraft_render::{PRINT_TAG, PageRenderer, RenderConfig, RenderRequest};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateDCW, DEVMODEW, DIB_RGB_COLORS, DM_COLLATE, DM_COLOR, DM_COPIES, DM_DUPLEX, DM_ORIENTATION,
    DMCOLLATE_FALSE, DMCOLLATE_TRUE, DMCOLOR_COLOR, DMCOLOR_MONOCHROME, DMDUP_HORIZONTAL, DMDUP_SIMPLEX, DMDUP_VERTICAL, DMORIENT_LANDSCAPE,
    DMORIENT_PORTRAIT, DeleteDC, GetDeviceCaps, HALFTONE, HDC, HORZRES, LOGPIXELSX, LOGPIXELSY, PHYSICALHEIGHT, PHYSICALOFFSETX,
    PHYSICALOFFSETY, PHYSICALWIDTH, SRCCOPY, SetStretchBltMode, StretchDIBits, VERTRES,
};
use windows::Win32::Graphics::Printing::{ClosePrinter, DocumentPropertiesW, OpenPrinterW, PRINTER_HANDLE};
use windows::Win32::Storage::Xps::{AbortDoc, DOCINFOW, EndDoc, EndPage, StartDocW, StartPage};
use windows::core::PCWSTR;

const DM_OUT_BUFFER: u32 = 2;
const DM_IN_BUFFER: u32 = 8;
/// Raster resolution for the sheets (capped by the printer's own).
const MAX_DPI: f32 = 300.0;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// The driver's DEVMODE for `printer` with the job's settings merged in (the driver validates it).
fn devmode(printer: &[u16], job: &spool::Job, landscape: bool) -> Result<Vec<u8>, String> {
    unsafe {
        let mut h = PRINTER_HANDLE::default();
        OpenPrinterW(PCWSTR(printer.as_ptr()), &mut h, None).map_err(|e| format!("OpenPrinter: {e}"))?;
        let result = (|| {
            let size = DocumentPropertiesW(None, h, PCWSTR(printer.as_ptr()), None, None, 0);
            if size < std::mem::size_of::<DEVMODEW>() as i32 {
                return Err("DocumentProperties: the driver gave no settings".to_string());
            }
            // u64 storage keeps the DEVMODE aligned.
            let mut buf = vec![0u64; (size as usize).div_ceil(8)];
            let dm = buf.as_mut_ptr() as *mut DEVMODEW;
            if DocumentPropertiesW(None, h, PCWSTR(printer.as_ptr()), Some(dm), None, DM_OUT_BUFFER) < 0 {
                return Err("DocumentProperties: reading the driver's settings failed".to_string());
            }
            let d = &mut *dm;
            d.dmFields |= DM_COPIES | DM_COLLATE | DM_COLOR | DM_ORIENTATION;
            d.Anonymous1.Anonymous1.dmCopies = job.copies.clamp(1, 999) as i16;
            d.dmCollate = if job.collate { DMCOLLATE_TRUE } else { DMCOLLATE_FALSE };
            d.dmColor = if job.grayscale { DMCOLOR_MONOCHROME } else { DMCOLOR_COLOR };
            d.Anonymous1.Anonymous1.dmOrientation = if landscape { DMORIENT_LANDSCAPE as i16 } else { DMORIENT_PORTRAIT as i16 };
            if let Some(dup) = job.duplex {
                d.dmFields |= DM_DUPLEX;
                d.dmDuplex = match dup {
                    spool::Duplex::Off => DMDUP_SIMPLEX,
                    spool::Duplex::LongEdge => DMDUP_VERTICAL,
                    spool::Duplex::ShortEdge => DMDUP_HORIZONTAL,
                };
            }
            if DocumentPropertiesW(None, h, PCWSTR(printer.as_ptr()), Some(dm), Some(dm), DM_IN_BUFFER | DM_OUT_BUFFER) < 0 {
                return Err("DocumentProperties: the driver refused the settings".to_string());
            }
            let bytes = std::slice::from_raw_parts(buf.as_ptr() as *const u8, size as usize).to_vec();
            Ok(bytes)
        })();
        let _ = ClosePrinter(h);
        result
    }
}

struct Dc(HDC);
impl Drop for Dc {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteDC(self.0);
        }
    }
}

/// Print `pdf` (the print-ready, already imposed PDF) on `job.printer` (or the default) via GDI.
pub fn submit(pdf: &[u8], job: &spool::Job, progress: &mut dyn FnMut(usize, usize)) -> Result<String, String> {
    let name = match &job.printer {
        Some(p) => p.clone(),
        None => winsafe::GetDefaultPrinter().map_err(|e| format!("no default printer: {e}"))?,
    };
    let bytes = Arc::new(pdf.to_vec());
    let info = pdfcraft_render::inspect(bytes.clone(), None).map_err(|e| format!("{e:?}"))?;
    let sheets: Vec<(f32, f32)> = info.pages.iter().map(|p| (p.width, p.height)).collect();
    let first = *sheets.first().ok_or("the print-ready PDF has no pages")?;
    let landscape = first.0 > first.1 + 0.5;
    let printer = wide(&name);
    let dm = devmode(&printer, job, landscape)?;
    unsafe {
        let hdc = CreateDCW(PCWSTR(wide("WINSPOOL").as_ptr()), PCWSTR(printer.as_ptr()), PCWSTR::null(), Some(dm.as_ptr() as *const DEVMODEW));
        if hdc.is_invalid() {
            return Err(format!("CreateDC failed for {name}"));
        }
        let dc = Dc(hdc);
        let cap = |i| GetDeviceCaps(Some(dc.0), i);
        let (lx, ly) = (cap(LOGPIXELSX).max(72) as f32, cap(LOGPIXELSY).max(72) as f32);
        let (pw, ph) = (cap(PHYSICALWIDTH), cap(PHYSICALHEIGHT));
        let (ox, oy) = (cap(PHYSICALOFFSETX), cap(PHYSICALOFFSETY));
        let (hr, vr) = (cap(HORZRES), cap(VERTRES));
        let title = wide(&job.title);
        let di = DOCINFOW { cbSize: std::mem::size_of::<DOCINFOW>() as i32, lpszDocName: PCWSTR(title.as_ptr()), ..Default::default() };
        if StartDocW(dc.0, &di) <= 0 {
            return Err(format!("StartDoc failed for {name}"));
        }
        let mut renderer = PageRenderer::new(bytes, RenderConfig::default());
        let dpi = MAX_DPI.min(lx.max(ly));
        progress(0, sheets.len());
        for (i, &(w, h)) in sheets.iter().enumerate() {
            let scale = pdfcraft_render::effective_scale(w, h, dpi / 72.0);
            let r = renderer.render(RenderRequest { page: i, scale, tag: PRINT_TAG, ..Default::default() });
            if let Some(e) = r.error {
                let _ = AbortDoc(dc.0);
                return Err(format!("sheet {}: {e}", i + 1));
            }
            // Premultiplied RGBA over white paper -> 32-bit BGRX, top-down.
            let mut px = r.rgba.to_vec();
            for p in px.as_chunks_mut::<4>().0 {
                let gap = 255 - u32::from(p[3]);
                let (rr, gg, bb) = ((u32::from(p[0]) + gap).min(255), (u32::from(p[1]) + gap).min(255), (u32::from(p[2]) + gap).min(255));
                let (rr, gg, bb) = if job.grayscale {
                    let y = ((299 * rr + 587 * gg + 114 * bb) / 1000).min(255);
                    (y, y, y)
                } else {
                    (rr, gg, bb)
                };
                *p = [bb as u8, gg as u8, rr as u8, 255];
            }
            // Where the sheet goes, in device pixels relative to the printable area's origin.
            let (dw, dh) = (w / 72.0 * lx, h / 72.0 * ly);
            let (x, y, dw, dh) = match job.scaling {
                spool::Scaling::Exact => (-(ox as f32) + (pw as f32 - dw) / 2.0, -(oy as f32) + (ph as f32 - dh) / 2.0, dw, dh),
                spool::Scaling::ShrinkToFit => {
                    let k = (hr as f32 / dw).min(vr as f32 / dh).min(1.0);
                    let (fw, fh) = (dw * k, dh * k);
                    ((hr as f32 - fw) / 2.0, (vr as f32 - fh) / 2.0, fw, fh)
                }
            };
            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: r.width as i32,
                    biHeight: -(r.height as i32),
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            if StartPage(dc.0) <= 0 {
                let _ = AbortDoc(dc.0);
                return Err("StartPage failed".into());
            }
            SetStretchBltMode(dc.0, HALFTONE);
            let done = StretchDIBits(
                dc.0,
                x.round() as i32,
                y.round() as i32,
                dw.round() as i32,
                dh.round() as i32,
                0,
                0,
                r.width as i32,
                r.height as i32,
                Some(px.as_ptr() as *const _),
                &bmi,
                DIB_RGB_COLORS,
                SRCCOPY,
            );
            if done == 0 || EndPage(dc.0) <= 0 {
                let _ = AbortDoc(dc.0);
                return Err(format!("sheet {} could not be sent to the printer", i + 1));
            }
            progress(i + 1, sheets.len());
        }
        if EndDoc(dc.0) <= 0 {
            return Err("EndDoc failed".into());
        }
    }
    Ok(format!("spooled {} sheet(s) (GDI)", sheets.len()))
}
