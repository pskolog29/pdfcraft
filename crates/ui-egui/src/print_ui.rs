//! The Print dialog (Acrobat's File ▸ Print, execution plan M10.5): printer, copies, grayscale;
//! pages to print (all, current, range with labels, the selected pages; odd/even, reverse); page sizing & handling
//! (Size, Poster, Multiple, Booklet); orientation; comments & forms; and a live preview of the
//! sheets. Printing sends the print-ready PDF to the system spooler; "Save as PDF" writes it.

use egui::emath::GuiRounding;
use egui::{Color32, Pos2, Rect, Stroke, pos2, vec2};
use pdfcraft_engine::print::{self, Binding, BookletSubset, Content, Layout, Orientation, PAPERS, PageOrder, SizeMode, Subset, spool};

use crate::theme::{self, Tokens};
use crate::{PdfCraftApp, widgets};

/// Space between the print dialog's settings column and the preview pane.
const COLUMN_GAP: f32 = 14.0;
/// The paper inside the preview pane keeps this much room on each side.
const PREVIEW_INSET: f32 = 12.0;

/// Arif Pdf Editör (arif.13): JoPDF-like wide dialog — large preview on the left, settings column
/// on the right. The sizes follow the window, down to a small one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PrintLayout {
    /// Settings column width (shown on the right).
    pub left_w: f32,
    pub preview_w: f32,
    pub preview_h: f32,
}

impl PrintLayout {
    pub(crate) fn for_screen(screen: egui::Vec2) -> Self {
        let avail_w = (screen.x - 80.0).max(0.0);
        // Settings stay readable; preview takes the rest (JoPDF left-preview proportions).
        let left_w = if avail_w >= 1100.0 { 380.0 } else { 340.0 };
        let total = avail_w.clamp(left_w + COLUMN_GAP + 360.0, left_w + COLUMN_GAP + 720.0);
        PrintLayout { left_w, preview_w: total - left_w - COLUMN_GAP, preview_h: (screen.y - 220.0).clamp(380.0, 620.0) }
    }
    /// The width of the whole dialog content.
    pub(crate) fn width(&self) -> f32 {
        self.left_w + COLUMN_GAP + self.preview_w
    }
    /// The room for a sheet inside the pane.
    pub(crate) fn sheet_box(&self) -> (f32, f32) {
        (self.preview_w - 2.0 * PREVIEW_INSET, self.preview_h - 2.0 * PREVIEW_INSET)
    }
}

/// Arif Pdf Editör: `PAPERS` index of A4, the paper the dialog starts with.
pub(crate) const A4_PAPER: usize = 4;
/// Label printing reads the printed area of at most this many pages (each is a quick render on
/// the UI thread); further pages print whole.
pub(crate) const LABEL_CROP_PAGES: usize = 50;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which {
    All,
    Current,
    /// Visible viewport (treated as the current page until a crop path is wired).
    CurrentView,
    Range,
    /// The pages picked in the Pages panel or the organize grid when the dialog opened.
    Selected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handling {
    Size,
    Poster,
    Multiple,
    Booklet,
    /// Shipping label: cut to the printed area, turned upright, one per sheet.
    Label,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PrintDraft {
    pub printers: Vec<spool::Printer>,
    /// The unit the sheet size is shown in (Preferences).
    pub units: crate::units::Unit,
    /// The printers are still being listed in the background (see [`PrinterSource`]).
    pub printers_loading: bool,
    /// The user chose the printer, so a list arriving late doesn't move it to the default.
    pub printer_picked: bool,
    /// `None` = Save as PDF.
    pub printer: Option<String>,
    pub copies: u32,
    pub collate: bool,
    pub grayscale: bool,
    pub duplex: spool::Duplex,
    pub which: Which,
    pub range: String,
    /// The pages `Which::Selected` prints (0-based, in page order); empty when none were picked.
    pub selected: Vec<usize>,
    pub subset: Subset,
    pub reverse: bool,
    pub handling: Handling,
    pub size: SizeMode,
    pub custom_scale: f64,
    pub per_sheet: usize,
    pub order: PageOrder,
    pub border: bool,
    pub auto_rotate: bool,
    /// JoPDF-like "Otomatik Ortala": keep the page centred on the sheet (Fit / layout already centres).
    pub auto_center: bool,
    pub booklet_subset: BookletSubset,
    pub binding: Binding,
    pub poster_scale: f64,
    pub overlap: f64,
    pub cut_marks: bool,
    pub orientation: Orientation,
    pub content: Content,
    /// Print as image (Acrobat's Advanced Print Setup) at `dpi`.
    pub as_image: bool,
    pub dpi: f64,
    pub paper: usize,
    /// The sheet shown in the preview (0-based).
    pub sheet: usize,
    pub current_page: usize,
    /// The printer driver's own options and the printer they were read for (Properties…, CUPS).
    pub driver_options: Option<(String, Vec<spool::PrinterOption>)>,
    /// Driver options set away from the printer's defaults, for `driver_options`' printer.
    pub driver_choices: std::collections::BTreeMap<String, String>,
    pub show_driver_options: bool,
    /// Why the printer's preferences window didn't open (Windows).
    pub driver_error: Option<String>,
    /// Label: turn a further 180°.
    pub label_flip: bool,
    /// Label: each page's printed area and text direction, read when Label is chosen
    /// ([`PdfCraftApp::read_label_crops`]); empty until then.
    pub crops: Vec<Option<print::Crop>>,
    /// The pages `crops` were read for, so a changed page selection reads them again.
    pub crop_pages: Option<Vec<usize>>,
}

impl Default for PrintDraft {
    fn default() -> Self {
        PrintDraft {
            printers: Vec::new(),
            units: Default::default(),
            printers_loading: false,
            printer_picked: false,
            printer: None,
            copies: 1,
            collate: true,
            grayscale: false,
            duplex: spool::Duplex::Off,
            which: Which::All,
            range: String::new(),
            selected: Vec::new(),
            subset: Subset::All,
            reverse: false,
            handling: Handling::Size,
            size: SizeMode::Fit,
            custom_scale: 100.0,
            per_sheet: 2,
            order: PageOrder::Horizontal,
            border: false,
            auto_rotate: true,
            auto_center: true,
            booklet_subset: BookletSubset::BothSides,
            binding: Binding::Left,
            poster_scale: 200.0,
            overlap: 18.0,
            cut_marks: true,
            orientation: Orientation::Auto,
            content: Content::DocumentAndMarkups,
            as_image: false,
            dpi: 300.0,
            paper: A4_PAPER,
            sheet: 0,
            current_page: 0,
            driver_options: None,
            driver_choices: Default::default(),
            show_driver_options: false,
            driver_error: None,
            label_flip: false,
            crops: Vec::new(),
            crop_pages: None,
        }
    }
}

impl PrintDraft {
    /// The engine settings for this draft (page count and labels from the document).
    pub fn settings(&self, count: usize, labels: &[String]) -> Result<print::Settings, String> {
        if self.handling == Handling::Multiple && self.order == PageOrder::CutStack && self.duplex != spool::Duplex::Off {
            return Err("Cut and stack needs Two-sided: Off. Print single-sided sheets.".into());
        }
        let range = match self.which {
            Which::All | Which::Selected | Which::Current | Which::CurrentView => None,
            Which::Range => Some(self.range.clone()),
        };
        // Selected and Current are positions, not a typed range: a range would read numbers as
        // page labels first.
        let pages = match self.which {
            Which::Selected => print::select_listed(count, &self.selected, self.subset, self.reverse),
            Which::Current | Which::CurrentView => print::select_listed(count, &[self.current_page], self.subset, self.reverse),
            Which::All | Which::Range => print::select_pages(count, range.as_deref(), labels, self.subset, self.reverse),
        }
        // What the person got wrong about what to print is said in their language: the engine hands
        // the wording over whole and the page numbers apart from it.
        .map_err(|e| e.said(&crate::i18n::words))?;
        let layout = match self.handling {
            Handling::Size => Layout::Size(match self.size {
                SizeMode::Custom(_) => SizeMode::Custom(self.custom_scale),
                m => m,
            }),
            Handling::Multiple => match Layout::multiple(self.per_sheet) {
                Layout::Multiple { cols, rows, per_sheet, .. } => {
                    Layout::Multiple { cols, rows, per_sheet, order: self.order, border: self.border, auto_rotate: self.auto_rotate }
                }
                other => other,
            },
            Handling::Booklet => Layout::Booklet { subset: self.booklet_subset, binding: self.binding },
            Handling::Poster => Layout::Poster { scale: self.poster_scale, overlap: self.overlap, cut_marks: self.cut_marks },
            Handling::Label => Layout::Label { flip: self.label_flip },
        };
        Ok(print::Settings {
            pages,
            paper: PAPERS[self.paper.min(PAPERS.len() - 1)].1,
            orientation: self.orientation,
            layout,
            content: self.content,
            raster: self.as_image.then_some(print::Raster { dpi: self.dpi, grayscale: self.grayscale }),
            crops: self.crops.clone(),
        })
    }

    pub fn job(&self, title: &str) -> spool::Job {
        spool::Job {
            printer: self.printer.clone(),
            copies: self.copies.max(1),
            collate: self.collate,
            duplex: Some(self.duplex),
            scaling: spool::Scaling::Exact,
            grayscale: self.grayscale,
            title: title.to_string(),
            // Only for the printer the options were read from.
            options: match (&self.printer, &self.driver_options) {
                (Some(p), Some((read_for, _))) if p == read_for => self.driver_choices.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
                _ => Vec::new(),
            }
            .into_iter()
            // The sheet the PDF was laid out on, not the queue's default paper.
            .chain(print::cups_media(self.paper).map(|m| ("media".to_string(), m.to_string())))
            .collect(),
        }
    }
}

/// Lists the printers off the UI thread (the desktop app sets [`PdfCraftApp::printer_source`]):
/// on Windows each driver's capabilities are read for its two-sided support, and CUPS' driverless discovery can wait on
/// the network. Without a source the dialog reads them as it opens, so tests stay deterministic.
pub type PrinterSource = std::sync::Arc<dyn Fn() -> Vec<spool::Printer> + Send + Sync>;

/// Arif Pdf Editör: the spooler's queue names and the default, without reading any driver's
/// capabilities (two-sided support stays unknown until the full list arrives). Local queues and
/// the user's connections, like craft-print's own list; sorted, each once.
#[cfg(windows)]
fn quick_printers() -> Vec<spool::Printer> {
    use winsafe::co::PRINTER_ENUM;
    let Ok(infos) = winsafe::EnumPrinters4(PRINTER_ENUM::LOCAL | PRINTER_ENUM::CONNECTIONS) else { return Vec::new() };
    let default = winsafe::GetDefaultPrinter().ok().filter(|n| !n.is_empty());
    let mut names: Vec<String> = infos.iter().filter_map(|i| i.pPrinterName()).filter(|n| !n.trim().is_empty()).collect();
    names.sort();
    names.dedup();
    names.into_iter().map(|name| spool::Printer { default: default.as_deref() == Some(name.as_str()), name, two_sided: None }).collect()
}

/// The system spooler's printers, as the desktop app's [`PrinterSource`].
pub fn system_printers() -> PrinterSource {
    std::sync::Arc::new(spool::printers)
}

/// The Print dialog's work in the background: the printer list being read, and the jobs being
/// spooled, oldest first.
#[derive(Default)]
pub(crate) struct PrintTasks {
    #[cfg(not(target_arch = "wasm32"))]
    printers: Option<std::sync::mpsc::Receiver<Vec<spool::Printer>>>,
    #[cfg(not(target_arch = "wasm32"))]
    jobs: Vec<PrintJob>,
    /// The progress card is showing a print job (it is shared with other long jobs).
    #[cfg(not(target_arch = "wasm32"))]
    showing_progress: bool,
    /// The browser print being drawn, a sheet a frame (`web_print`).
    #[cfg(target_arch = "wasm32")]
    web_job: Option<crate::web_print::WebPrintJob>,
    /// What went wrong after the browser print page was handed over, with the print-ready PDF
    /// to download instead.
    #[cfg(target_arch = "wasm32")]
    web_failed: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    /// The last browser print's PDF and name, for that fallback.
    #[cfg(target_arch = "wasm32")]
    web_last: Option<(String, std::sync::Arc<Vec<u8>>)>,
}

/// The printers to offer when no [`PrinterSource`] is set: the spooler's, or in a browser its
/// print dialog.
fn listed_printers() -> Vec<spool::Printer> {
    #[cfg(target_arch = "wasm32")]
    {
        vec![spool::Printer { name: crate::web_print::BROWSER_PRINTER.to_string(), default: true, two_sided: None }]
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        spool::printers()
    }
}

/// The first list is still being read and the user hasn't chosen yet (#160): no printer is
/// selected only because none is known yet, so the dialog says it is looking rather than
/// offering Save as PDF, and its button waits. Printing then would have saved a PDF.
fn still_looking(d: &PrintDraft) -> bool {
    d.printers_loading && d.printer.is_none() && !d.printer_picked
}

/// A printer's name as the dialog shows it.
fn printer_label(name: &str) -> String {
    if name == crate::web_print::BROWSER_PRINTER { crate::web_print::browser_printer_label().to_string() } else { name.to_string() }
}

/// A job being spooled: its printer, the sheets sent so far of how many, and its worker's news.
#[cfg(not(target_arch = "wasm32"))]
struct PrintJob {
    printer: String,
    sheets: Option<(usize, usize)>,
    news: std::sync::mpsc::Receiver<JobNews>,
}

#[cfg(not(target_arch = "wasm32"))]
enum JobNews {
    /// `(sent, sheets)`: the spooler has this many of the job's sheets.
    Sent(usize, usize),
    /// The spooler's answer.
    Done(Result<String, String>),
}

/// The printer the dialog preselects: the system default, else the first.
fn default_printer(printers: &[spool::Printer]) -> Option<String> {
    printers.iter().find(|p| p.default).or(printers.first()).map(|p| p.name.clone())
}

impl PdfCraftApp {
    pub fn open_print(&mut self) {
        let Some((i, _)) = self.active_ids() else { return };
        let (printers, printers_loading) = match self.printer_source.clone() {
            // The list from the last time the dialog opened, until the fresh one arrives.
            #[cfg(not(target_arch = "wasm32"))]
            Some(source) => {
                self.list_printers_in_background(source);
                (self.print_draft.printers.clone(), true)
            }
            _ => (listed_printers(), false),
        };
        let default = default_printer(&printers);
        let current = self.views[i].current;
        let selected: Vec<usize> = self.views[i].selected.iter().copied().collect();
        let keep = std::mem::take(&mut self.print_draft);
        // Picked pages are what Print is for (Acrobat's "Selected pages"); without any, a choice
        // left over from another document falls back to the whole document.
        let which = match (selected.is_empty(), keep.which) {
            (false, _) => Which::Selected,
            (true, Which::Selected) => Which::All,
            (true, other) => other,
        };
        self.print_draft = PrintDraft {
            crops: Vec::new(),
            crop_pages: None,
            printers,
            printers_loading,
            printer_picked: false,
            units: self.units,
            printer: default,
            current_page: current,
            sheet: 0,
            selected,
            which,
            ..keep
        };
        self.dialog = Some(crate::Dialog::Print);
    }

    /// Label printing: find each printed page's printed area (a quick low-resolution render, as
    /// Fit Visible does) and which way its text runs. Only the pages being printed are read, at
    /// most [`LABEL_CROP_PAGES`] of them (the rest print whole); read again when they change.
    pub(crate) fn read_label_crops(&mut self, id: pdfcraft_engine::DocId) {
        let Some(doc) = self.session.get(id) else { return };
        let labels: Vec<String> = doc.info.pages.iter().map(|p| p.label.clone()).collect();
        let mut pages = self.print_draft.settings(doc.info.pages.len(), &labels).map(|s| s.pages).unwrap_or_default();
        pages.sort_unstable();
        pages.dedup();
        pages.truncate(LABEL_CROP_PAGES);
        if self.print_draft.crop_pages.as_ref() == Some(&pages) {
            return;
        }
        let up = self.session.print_text_up(id, &pages);
        let config = pdfcraft_render::RenderConfig { password: doc.password.as_deref().map(std::sync::Arc::from), ..Default::default() };
        let mut r = pdfcraft_render::PageRenderer::new(doc.bytes.clone(), config);
        self.print_draft.crops = doc
            .info
            .pages
            .iter()
            .enumerate()
            .map(|(page, p)| {
                let (w, h) = (f64::from(p.width), f64::from(p.height));
                if pages.binary_search(&page).is_err() || w <= 0.0 || h <= 0.0 {
                    return None;
                }
                let out = r.render(pdfcraft_render::RenderRequest {
                    page,
                    kind: pdfcraft_render::RequestKind::Pixels,
                    tile: None,
                    scale: (1000.0 / w.max(h) as f32).clamp(0.1, 4.0),
                    tag: 0,
                    preview: false,
                });
                let [u0, v0, u1, v1] = crate::zoom_snap::ink_bounds(out.width as usize, out.height as usize, &out.rgba)?;
                // Image rows run down from the top; display space runs up. Half a millimetre of slack.
                let pad = 1.5;
                let rect = [f64::from(u0) * w - pad, (1.0 - f64::from(v1)) * h - pad, f64::from(u1) * w + pad, (1.0 - f64::from(v0)) * h + pad];
                Some(print::Crop { rect, text_up: up.get(page).copied().unwrap_or(false) })
            })
            .collect();
        self.print_draft.crop_pages = Some(pages);
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn list_printers_in_background(&mut self, source: PrinterSource) {
        // A list already being read is as fresh as a new one would be.
        if self.print_tasks.printers.is_some() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            // Arif Pdf Editör: on Windows the full list waits on every driver's PrintCapabilities
            // (a WSD/IPP printer can take 10 s or more), and until then the dialog had no printer to
            // offer but "Save as PDF". Send the queue names (EnumPrinters + default, instant) first.
            #[cfg(windows)]
            {
                let quick = quick_printers();
                if !quick.is_empty() {
                    let _ = tx.send(quick);
                    if let Some(ctx) = &ctx {
                        ctx.request_repaint();
                    }
                }
            }
            // The receiver may be gone (the app quit): nothing to report to then.
            let _ = tx.send(source());
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
        self.print_tasks.printers = Some(rx);
    }

    /// Pick up a printer list or a spooled job that finished (each frame).
    pub(crate) fn poll_print(&mut self) {
        #[cfg(target_arch = "wasm32")]
        self.poll_web_print();
        #[cfg(not(target_arch = "wasm32"))]
        {
            // Arif Pdf Editör: the background reader may answer twice (a quick list of names first,
            // then the full one with two-sided support); take each answer and stop at disconnect.
            while let Some(rx) = &self.print_tasks.printers {
                let (printers, done) = match rx.try_recv() {
                    Ok(p) => (Some(p), false),
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    // A source that died lists nothing new; the dialog keeps what it has.
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => (None, true),
                };
                if done {
                    self.print_tasks.printers = None;
                    self.print_draft.printers_loading = false;
                    break;
                }
                if let Some(printers) = printers {
                    let d = &mut self.print_draft;
                    // A later, failed full read (empty) never hides the names already shown.
                    if printers.is_empty() && !d.printers.is_empty() && !d.printers_loading {
                        continue;
                    }
                    if !d.printer_picked && (d.printer.is_none() || !printers.iter().any(|p| Some(&p.name) == d.printer.as_ref())) {
                        d.printer = default_printer(&printers);
                    }
                    d.printers = printers;
                    d.printers_loading = false;
                }
            }
            let mut finished = Vec::new();
            self.print_tasks.jobs.retain_mut(|job| {
                loop {
                    match job.news.try_recv() {
                        Ok(JobNews::Sent(sent, sheets)) => job.sheets = Some((sent, sheets)),
                        Ok(JobNews::Done(r)) => {
                            finished.push((job.printer.clone(), r));
                            break false;
                        }
                        Err(std::sync::mpsc::TryRecvError::Empty) => break true,
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                            finished.push((job.printer.clone(), Err("the print job stopped unexpectedly".into())));
                            break false;
                        }
                    }
                }
            });
            self.show_print_progress();
            for (printer, result) in finished {
                self.report_print_job(&printer, result);
            }
        }
    }

    /// The oldest job being spooled on the progress card, counting its sheets as they reach the
    /// spooler, while no other long job holds the card.
    #[cfg(not(target_arch = "wasm32"))]
    fn show_print_progress(&mut self) {
        let Some(job) = self.print_tasks.jobs.first() else {
            if std::mem::take(&mut self.print_tasks.showing_progress) {
                self.progress_notice = None;
            }
            return;
        };
        if self.progress_notice.is_some() && !self.print_tasks.showing_progress {
            return;
        }
        let printer = job.printer.as_str();
        let (label, fraction) = match job.sheets {
            Some((sent, sheets)) if sheets > 0 => {
                // The sheet on its way: one past those sent, until the last is.
                let sheet = sent.saturating_add(1).min(sheets).to_string();
                let label = crate::i18n::fmt(
                    tl!("Sending sheet {s} of {n} to {printer}…"),
                    &[("s", &sheet), ("n", &sheets.to_string()), ("printer", printer)],
                );
                (label, sent as f32 / sheets as f32)
            }
            _ => (crate::i18n::fmt(tl!("Sending to {printer}…"), &[("printer", printer)]), 0.0),
        };
        self.progress_notice = Some(crate::widgets::ProgressNotice { label, fraction, cancellable: false });
        self.print_tasks.showing_progress = true;
        // The bar's sweep keeps moving between sheets.
        if let Some(ctx) = &self.ctx {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    fn report_print_job(&mut self, printer: &str, result: Result<String, String>) {
        match result {
            Ok(msg) if msg.is_empty() => self.notify(crate::i18n::fmt(tl!("Sent to {printer}"), &[("printer", printer)])),
            Ok(msg) => self.notify(crate::i18n::fmt(tl!("Sent to {printer}: {msg}"), &[("printer", printer), ("msg", &msg)])),
            Err(e) => self.notify_error(e),
        }
    }

    /// Print (or save) with the dialog's settings. Returns `true` on success. Save as PDF without
    /// a preset path returns `true` once the save picker is showing; the file is written on a
    /// later frame, when the user has chosen where.
    pub fn print_now(&mut self) -> bool {
        // arif.13 TEST BUILD: never send a real spool job while ARIFPDF_DRY_PRINT is set.
        if std::env::var_os("ARIFPDF_DRY_PRINT").is_some() {
            self.notify_tr("Test build: print job was not sent (dry run).");
            return false;
        }
        // What's typed in a form field is part of what's printed (#166).
        if !self.commit_form_typing() {
            return false;
        }
        let Some((_, id)) = self.active_ids() else { return false };
        let Some(doc) = self.session.get(id) else { return false };
        let labels: Vec<String> = doc.info.pages.iter().map(|p| p.label.clone()).collect();
        let settings = match self.print_draft.settings(doc.info.pages.len(), &labels) {
            Ok(s) => s,
            Err(e) => {
                self.notify_tr(&e);
                return false;
            }
        };
        let name = doc.name.clone();
        let bytes = match self.session.print_pdf(id, &settings) {
            Ok(b) => b,
            Err(e) => {
                self.notify_error(e);
                return false;
            }
        };
        match self.print_draft.printer.clone() {
            #[cfg(target_arch = "wasm32")]
            Some(printer) if printer == crate::web_print::BROWSER_PRINTER => {
                let sizes: Vec<(f64, f64)> = doc.info.pages.iter().map(|p| (f64::from(p.width), f64::from(p.height))).collect();
                match print::layout(&sizes, &settings) {
                    Ok(sheets) => self.print_in_browser(name, bytes, sheets.iter().map(|s| s.size).collect()),
                    Err(e) => {
                        self.notify_error(e);
                        false
                    }
                }
            }
            Some(printer) => self.send_to_printer(printer, bytes, self.print_draft.job(&name)),
            None => self.save_print_pdf(&name, bytes),
        }
    }

    /// Spool a job off the UI thread: on Windows every sheet is rendered at the printer's
    /// resolution first, which can take minutes for a long document. The dialog closes at once,
    /// the progress card counts the sheets as they reach the spooler, and the spooler's answer
    /// arrives as a notice.
    #[cfg(not(target_arch = "wasm32"))]
    fn send_to_printer(&mut self, printer: String, bytes: Vec<u8>, job: spool::Job) -> bool {
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let repaint = || {
                if let Some(ctx) = &ctx {
                    ctx.request_repaint();
                }
            };
            // The receiver may be gone (the app quit): nothing to report to then.
            let mut accepted = 0usize;
            let result = spool::submit_with_progress(&bytes, &job, &mut |sent, sheets| {
                accepted = accepted.max(sent);
                let _ = tx.send(JobNews::Sent(sent, sheets));
                repaint();
            });
            let result = result.map_err(|e| e.to_string());
            // Arif Pdf Editör: a driver that refuses the Direct2D/XPS path before taking any
            // sheet (that job is cancelled, nothing was printed) is printed again through GDI.
            #[cfg(windows)]
            let result = match result {
                Err(e) if accepted == 0 => {
                    log::warn!("print: Direct2D/XPS path failed ({e}); retrying through GDI");
                    arif_gdi_print::submit(&bytes, &job, &mut |sent, sheets| {
                        let _ = tx.send(JobNews::Sent(sent, sheets));
                        repaint();
                    })
                    .map_err(|g| format!("{e}; GDI: {g}"))
                }
                other => other,
            };
            let _ = tx.send(JobNews::Done(result));
            repaint();
        });
        self.print_tasks.jobs.push(PrintJob { printer, sheets: None, news: rx });
        self.show_print_progress();
        true
    }

    /// A browser has no spooler: `submit` answers at once.
    #[cfg(target_arch = "wasm32")]
    fn send_to_printer(&mut self, printer: String, bytes: Vec<u8>, job: spool::Job) -> bool {
        let result = spool::submit(&bytes, &job).map_err(|e| e.to_string());
        let ok = result.is_ok();
        self.report_print_job(&printer, result);
        ok
    }

    /// Print through the browser's print dialog: start drawing `sheets` (the pages of the
    /// print-ready `bytes`), a sheet a frame in [`PdfCraftApp::poll_print`]. A print still being
    /// prepared is replaced. The dialog closes at once.
    #[cfg(target_arch = "wasm32")]
    fn print_in_browser(&mut self, name: String, bytes: Vec<u8>, sheets: Vec<(f64, f64)>) -> bool {
        match crate::web_print::WebPrintJob::new(bytes, sheets, name) {
            Ok(job) => {
                let n = job.total().to_string();
                self.print_tasks.web_job = Some(job);
                self.notify_fmt("Preparing {n} sheets for printing…", &[("n", &n)]);
                if let Some(ctx) = &self.ctx {
                    ctx.request_repaint();
                }
                true
            }
            Err(e) => {
                self.notify_error(e);
                false
            }
        }
    }

    /// Draw the next sheet of the browser print, and hand the finished page to the browser.
    #[cfg(target_arch = "wasm32")]
    fn poll_web_print(&mut self) {
        let failed: Vec<String> = self.print_tasks.web_failed.lock().map(|mut q| std::mem::take(&mut *q)).unwrap_or_default();
        if let Some(e) = failed.into_iter().next() {
            self.web_print_failed(&e);
        }
        let Some(job) = self.print_tasks.web_job.as_mut() else { return };
        if !job.finished() {
            if let Err(e) = job.step(&mut crate::web_print::blob_url) {
                self.print_tasks.web_job = None;
                self.notify_error(e);
                return;
            }
            let (done, total) = (job.done().to_string(), job.total().to_string());
            self.notify_fmt("Preparing sheet {done} of {total} for printing…", &[("done", &done), ("total", &total)]);
            if let Some(ctx) = &self.ctx {
                ctx.request_repaint();
            }
            return;
        }
        let Some(mut job) = self.print_tasks.web_job.take() else { return };
        let html = job.html();
        let urls = job.take_urls();
        self.print_tasks.web_last = Some((job.name.clone(), job.pdf.clone()));
        match crate::web_print::print_in_iframe(&html, urls, self.print_tasks.web_failed.clone(), self.ctx.clone()) {
            Ok(()) => self.notify_tr("Opening the browser's print dialog…"),
            Err(e) => self.web_print_failed(&e),
        }
    }

    /// The browser wouldn't print (a sandboxed frame, a picture it couldn't decode): offer the
    /// print-ready PDF as a download instead, so the work isn't lost.
    #[cfg(target_arch = "wasm32")]
    fn web_print_failed(&mut self, error: &str) {
        let Some((name, pdf)) = self.print_tasks.web_last.take() else {
            self.notify_error(error);
            return;
        };
        let stem = name.trim_end_matches(".pdf").trim_end_matches(".PDF");
        let file = format!("{stem} (print).pdf");
        match crate::editing::download(&file, &pdf) {
            Ok(()) => self.notify_fmt("Couldn't print from the browser ({e}); downloaded {name} to print instead", &[("e", error), ("name", &file)]),
            Err(_) => self.notify_error(error),
        }
    }

    /// Print ▸ Save as PDF on the desktop: write to `save_override` (tests and automation), or
    /// ask where and write on a later frame. Returns `true` once written, or once the save picker
    /// is showing.
    #[cfg(not(target_arch = "wasm32"))]
    fn save_print_pdf(&mut self, name: &str, bytes: Vec<u8>) -> bool {
        let write = move |app: &mut Self, path: std::path::PathBuf| match crate::editing::write_atomically(&path.to_string_lossy(), &bytes) {
            Ok(()) => {
                app.notify_fmt("Saved the print-ready PDF to {path}", &[("path", &path.display().to_string())]);
                true
            }
            Err(e) => {
                app.notify_fmt("Could not save: {e}", &[("e", &e.to_string())]);
                false
            }
        };
        match self.save_override.clone() {
            Some(p) => write(self, p.into()),
            None => {
                let stem = name.trim_end_matches(".pdf").trim_end_matches(".PDF");
                let dialog = rfd::AsyncFileDialog::new().add_filter("PDF", &["pdf"]).set_file_name(format!("{stem} (print).pdf"));
                self.ask_one(crate::pickers::Ask::Save(dialog), None, move |app, path| {
                    write(app, path);
                })
            }
        }
    }

    /// Print ▸ Save as PDF in a browser: download it, as Save does (#170). There is no folder to
    /// choose and no file system to write to, so `save_override` doesn't apply.
    #[cfg(target_arch = "wasm32")]
    fn save_print_pdf(&mut self, name: &str, bytes: Vec<u8>) -> bool {
        let stem = name.trim_end_matches(".pdf").trim_end_matches(".PDF");
        let file = format!("{stem} (print).pdf");
        match crate::editing::download(&file, &bytes) {
            Ok(()) => {
                self.notify_fmt("Downloaded {name}", &[("name", &file)]);
                true
            }
            Err(e) => {
                self.notify_fmt("Couldn't download {name}: {e}", &[("name", &file), ("e", &e)]);
                false
            }
        }
    }
}

/// The longest side, in pixels, of one page's print-preview raster (also kept within the GPU's
/// `max_texture_side`): an Actual-size print of a large page would otherwise ask for a raster
/// bigger than a texture can be.
pub(crate) const PREVIEW_MAX_SIDE: f32 = 4096.0;
/// Arif Pdf Editör: points of preview per point of sheet. The pane shows an A4 sheet (in the sheet's
/// own orientation) as large as fits; a smaller paper (A5, a label) is drawn proportionally
/// smaller beside that reference, a larger one (A3, Letter's width…) is fitted so it stays whole.
pub(crate) fn preview_k(sheet: (f64, f64), room: (f32, f32)) -> f32 {
    let (sw, sh) = (sheet.0 as f32, sheet.1 as f32);
    let fit = |w: f32, h: f32| (room.0 / w).min(room.1 / h);
    let a4 = PAPERS[A4_PAPER].1;
    let (aw, ah) = if sw > sh { (a4.1 as f32, a4.0 as f32) } else { (a4.0 as f32, a4.1 as f32) };
    fit(sw, sh).min(fit(aw, ah))
}

/// The most bytes of print-preview rasters kept at once. The current sheet comes first, then the
/// next and the previous one; a neighbour that doesn't fit isn't rendered ahead.
pub(crate) const PREVIEW_BYTES: f64 = 128.0 * 1024.0 * 1024.0;

/// Pages on the current sheet and its neighbours, each with the device pixels per point that
/// fill the preview pane (so the picture is not a stretched thumbnail), nearest sheet first and
/// within [`PREVIEW_MAX_SIDE`] (and `max_side`, the GPU's texture limit) and [`PREVIEW_BYTES`].
pub(crate) fn preview_rasters(d: &PrintDraft, sizes: &[(f64, f64)], labels: &[String], ppp: f32, max_side: f32, layout: &PrintLayout) -> Vec<(usize, f32)> {
    let Ok(settings) = d.settings(sizes.len(), labels) else { return Vec::new() };
    let Ok(sheets) = print::layout(sizes, &settings) else { return Vec::new() };
    if sheets.is_empty() {
        return Vec::new();
    }
    // Arif Pdf Editör: twice the screen's device density (at least 2 px per point): the raster is
    // exactly two texels per screen pixel, so the linear filter averages 2x2 texels (no stretching,
    // no skipped detail) and small text stays crisp on every display scale.
    let ppp = if ppp.is_finite() { ppp.max(1.0) * 2.0 } else { 2.0 };
    let side = if max_side.is_finite() && max_side >= 1.0 { max_side.min(PREVIEW_MAX_SIDE) } else { PREVIEW_MAX_SIDE };
    let i = d.sheet.min(sheets.len() - 1);
    // Nearest first: the sheet on screen, then the next one, then the previous one.
    let order = [Some(i), i.checked_add(1).filter(|n| *n < sheets.len()), i.checked_sub(1)];
    let mut out: Vec<(usize, f32, f64)> = Vec::new();
    let mut bytes = 0.0f64;
    for sheet in order.into_iter().flatten().filter_map(|s| sheets.get(s)) {
        let (sw, sh) = (sheet.size.0 as f32, sheet.size.1 as f32);
        if !(sw.is_finite() && sh.is_finite()) || sw < 1.0 || sh < 1.0 {
            continue;
        }
        let k = preview_k((sw as f64, sh as f64), layout.sheet_box());
        for pl in &sheet.placed {
            let [a, b, _, _, _, _] = pl.matrix.0;
            let placed = (a.hypot(b) as f32) * k * ppp;
            let Some(&(pw, ph)) = sizes.get(pl.page) else { continue };
            let long_pt = pw.max(ph) as f32;
            if !placed.is_finite() || placed < 0.05 || !long_pt.is_finite() || long_pt < 1.0 {
                continue;
            }
            let scale = placed.min(64.0).min(side / long_pt);
            let cost = (pw * f64::from(scale)).ceil() * (ph * f64::from(scale)).ceil() * 4.0;
            if let Some(slot) = out.iter_mut().find(|(page, _, _)| *page == pl.page) {
                if scale > slot.1 && bytes - slot.2 + cost <= PREVIEW_BYTES {
                    bytes += cost - slot.2;
                    (slot.1, slot.2) = (scale, cost);
                }
            } else if out.len() < 48 && (out.is_empty() || bytes + cost <= PREVIEW_BYTES) {
                bytes += cost;
                out.push((pl.page, scale, cost));
            }
        }
    }
    out.into_iter().map(|(page, scale, _)| (page, scale)).collect()
}

/// A thin, low-contrast rule with softly faded ends, from `x0` to `x1` at `y`: it separates the
/// dialog's rows and groups without adding weight (theme `divider` colour, so light and dark match).
fn rule_at(ui: &egui::Ui, t: &Tokens, x0: f32, x1: f32, y: f32) {
    if !(x1 > x0 + 4.0) {
        return;
    }
    let y = y.round();
    let fade = ((x1 - x0) * 0.12).clamp(2.0, 24.0);
    let c = t.divider;
    let xs = [(x0, Color32::TRANSPARENT), (x0 + fade, c), (x1 - fade, c), (x1, Color32::TRANSPARENT)];
    let mut mesh = egui::Mesh::default();
    for (x, col) in xs {
        for yy in [y, y + 1.0] {
            mesh.vertices.push(egui::epaint::Vertex { pos: pos2(x, yy), uv: egui::epaint::WHITE_UV, color: col });
        }
    }
    for i in 0..3u32 {
        let b = i * 2;
        mesh.indices.extend_from_slice(&[b, b + 1, b + 3, b, b + 3, b + 2]);
    }
    ui.painter().add(egui::Shape::mesh(mesh));
}

/// A rule across the available width with comfortable space above and below.
fn rule(ui: &mut egui::Ui, t: &Tokens) {
    ui.add_space(5.0);
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), egui::Sense::hover());
    rule_at(ui, t, r.left(), r.right(), r.top());
    ui.add_space(3.0);
}

/// Paint one sheet (paper, placed pages, borders, cut lines) into `paper`, at `k` points of
/// screen per point of sheet. `texture(page)` gives each page's raster, if it has arrived.
fn paint_sheet(
    painter: &egui::Painter,
    paper: Rect,
    k: f32,
    sheet: &print::Sheet,
    sizes: &[(f64, f64)],
    grayscale: bool,
    t: &Tokens,
    texture: &mut dyn FnMut(usize) -> Option<egui::TextureId>,
) {
    painter.rect_filled(paper, 0.0, Color32::WHITE);
    painter.rect_stroke(paper, 0.0, Stroke::new(1.0, t.border), egui::StrokeKind::Outside);
    let to_screen = |x: f64, y: f64| -> Pos2 { pos2(paper.left() + x as f32 * k, paper.bottom() - y as f32 * k) };
    let painter = painter.with_clip_rect(paper);
    for pl in &sheet.placed {
        let (dw, dh) = sizes[pl.page];
        let [x0, y0, x1, y1] = pl.clip;
        let corners = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)].map(|(x, y)| {
            let (sx, sy) = pl.matrix.apply(x, y);
            to_screen(sx, sy)
        });
        // UVs of the visible part (texture y runs down from the page top).
        let uv = |x: f64, y: f64| pos2((x / dw) as f32, (1.0 - y / dh) as f32);
        let uvs = [uv(x0, y0), uv(x1, y0), uv(x1, y1), uv(x0, y1)];
        match texture(pl.page) {
            Some(tex) => {
                let mut mesh = egui::Mesh::with_texture(tex);
                for (p, u) in corners.iter().zip(uvs) {
                    mesh.vertices.push(egui::epaint::Vertex {
                        pos: *p,
                        uv: u,
                        color: if grayscale { Color32::from_gray(235) } else { Color32::WHITE },
                    });
                }
                mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
                painter.add(egui::Shape::mesh(mesh));
            }
            None => {
                painter.add(egui::Shape::convex_polygon(corners.to_vec(), Color32::from_gray(245), Stroke::new(0.5, Color32::from_gray(180))));
                let c = corners.iter().fold(vec2(0.0, 0.0), |a, p| a + p.to_vec2()) / 4.0;
                painter.text(c.to_pos2(), egui::Align2::CENTER_CENTER, (pl.page + 1).to_string(), theme::regular(11.0), Color32::from_gray(120));
            }
        }
    }
    for b in &sheet.borders {
        painter.rect_stroke(Rect::from_two_pos(to_screen(b[0], b[1]), to_screen(b[2], b[3])), 0.0, Stroke::new(0.6, Color32::BLACK), egui::StrokeKind::Middle);
    }
    for l in &sheet.lines {
        painter.line_segment([to_screen(l[0], l[1]), to_screen(l[2], l[3])], Stroke::new(0.6, Color32::BLACK));
    }
}

/// Properties… on CUPS: the driver's options for this print, grouped as the driver groups them.
/// Only choices away from the printer's defaults are kept, and sent with the job.
fn driver_options_panel(ui: &mut egui::Ui, d: &mut PrintDraft, t: &Tokens) {
    let PrintDraft { driver_options: Some((_, options)), driver_choices: choices, show_driver_options: show, .. } = d else { return };
    egui::Frame::new().fill(t.hover).corner_radius(egui::CornerRadius::same(6)).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(tl!("Printer properties")).font(theme::semibold(13.0)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(tl!("Done")).clicked() {
                    *show = false;
                }
                if ui.add_enabled(!choices.is_empty(), egui::Button::new(tl!("Reset to printer defaults"))).clicked() {
                    choices.clear();
                }
            });
        });
        if options.is_empty() {
            ui.label(egui::RichText::new(tl!("This printer offers no settings of its own.")).small().color(t.text_muted));
            return;
        }
        egui::ScrollArea::vertical().id_salt("driver-options").max_height(240.0).show(ui, |ui| {
            for (n, chunk) in options.chunk_by(|a, b| a.group == b.group).enumerate() {
                let group = chunk.first().map_or("", |o| o.group.as_str());
                if !group.is_empty() {
                    ui.add_space(4.0);
                    // Option and group names are the driver's own text.
                    ui.label(egui::RichText::new(group).small().color(t.text_muted));
                }
                egui::Grid::new(("driver-group", n)).num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                    for o in chunk {
                        ui.label(&o.label);
                        let current = choices.get(&o.key).cloned().unwrap_or_else(|| o.default.clone());
                        let label_of = |c: &str| o.choices.iter().find(|(k, _)| k == c).map_or_else(|| c.to_string(), |(_, l)| l.clone());
                        let mut pick = current.clone();
                        egui::ComboBox::from_id_salt(("driver-option", &o.key)).selected_text(label_of(&current)).width(200.0).show_ui(ui, |ui| {
                            for (k, l) in &o.choices {
                                ui.selectable_value(&mut pick, k.clone(), l.as_str());
                            }
                        });
                        if pick != current {
                            if pick == o.default {
                                choices.remove(&o.key);
                            } else {
                                choices.insert(o.key.clone(), pick);
                            }
                        }
                        ui.end_row();
                    }
                });
            }
        });
    });
}

fn combo<T: PartialEq + Copy>(ui: &mut egui::Ui, id: &str, value: &mut T, choices: &[(T, &str)], width: f32) {
    let shown = choices.iter().find(|c| c.0 == *value).map_or("", |c| c.1);
    egui::ComboBox::from_id_salt(id).selected_text(shown).width(width).show_ui(ui, |ui| {
        for (v, label) in choices {
            ui.selectable_value(value, *v, *label);
        }
    });
}

/// Draw the dialog. `thumb` gives a page's thumbnail texture when there is one. Returns
/// (print, cancel).
pub(crate) fn body(
    ui: &mut egui::Ui,
    d: &mut PrintDraft,
    t: &Tokens,
    sizes: &[(f64, f64)],
    labels: &[String],
    layout: &PrintLayout,
    thumb: &mut dyn FnMut(usize) -> Option<egui::TextureId>,
) -> (bool, bool) {
    ui.set_width(layout.width());
    ui.label(egui::RichText::new(tl!("Print")).font(theme::semibold(18.0)));
    ui.add_space(8.0);
    let settings = d.settings(sizes.len(), labels);
    let sheets = settings.as_ref().ok().and_then(|s| print::layout(sizes, s).ok()).unwrap_or_default();
    d.sheet = d.sheet.min(sheets.len().saturating_sub(1));
    // Accent-edged combo / toggle look for the JoPDF-like print chrome.
    widgets::print_controls_scope(ui, |ui| {
    ui.horizontal_top(|ui| {
        // arif.13 JoPDF-like: preview LEFT, settings RIGHT.
        ui.vertical(|ui| {
            ui.set_width(layout.preview_w);
            let (area, _) = ui.allocate_exact_size(vec2(layout.preview_w, layout.preview_h), egui::Sense::hover());
            ui.painter().rect_filled(area, 6.0, t.hover);
            match (&settings, sheets.get(d.sheet)) {
                (Err(e), _) => {
                    ui.put(area.shrink(16.0), egui::Label::new(egui::RichText::new(tl!(e)).color(t.text_muted)).wrap());
                }
                (Ok(_), Some(sheet)) => {
                    let k = preview_k(sheet.size, layout.sheet_box());
                    let ppp_now = ui.ctx().pixels_per_point();
                    // On the device pixel grid, so the sheet's edge and its texels line up.
                    let paper = Rect::from_center_size(area.center(), vec2(sheet.size.0 as f32 * k, sheet.size.1 as f32 * k)).round_to_pixels(ppp_now);
                    ui.painter().add(egui::epaint::Shadow { offset: [0, 2], blur: 10, spread: 0, color: t.page_shadow }.as_shape(paper, egui::CornerRadius::ZERO));
                    paint_sheet(ui.painter(), paper, k, sheet, sizes, d.grayscale, t, &mut |p| thumb(p));
                }
                _ => {}
            }
            ui.horizontal(|ui| {
                let n = sheets.len();
                if ui.add_enabled(d.sheet > 0, egui::Button::new("⏮")).on_hover_text(tl!("First sheet")).clicked() {
                    d.sheet = 0;
                }
                if ui.add_enabled(d.sheet > 0, egui::Button::new("‹")).on_hover_text(tl!("Previous sheet")).clicked() {
                    d.sheet -= 1;
                }
                ui.label(if n == 0 {
                    tl!("No sheets").to_string()
                } else {
                    format!("{} / {}", d.sheet + 1, n)
                });
                if ui.add_enabled(d.sheet + 1 < n, egui::Button::new("›")).on_hover_text(tl!("Next sheet")).clicked() {
                    d.sheet += 1;
                }
                if ui.add_enabled(d.sheet + 1 < n, egui::Button::new("⏭")).on_hover_text(tl!("Last sheet")).clicked() {
                    d.sheet = n.saturating_sub(1);
                }
            });
            if let Some(s) = sheets.get(d.sheet) {
                ui.label(egui::RichText::new(d.units.size(s.size.0, s.size.1)).small().color(t.text_muted));
                if d.handling == Handling::Size
                    && let Some(p) = s.placed.first()
                {
                    let scale = p.matrix.0[0].hypot(p.matrix.0[1]) * 100.0;
                    ui.label(crate::i18n::fmt(tl!("PDF scale: {scale}%"), &[("scale", &format!("{scale:.2}"))]));
                    ui.label(egui::RichText::new(tl!("Printer paper and scaling may differ. Check driver settings.")).small().color(t.text_muted));
                    if (scale - 100.0).abs() > 0.01 {
                        ui.label(egui::RichText::new(tl!("Drawing scale changes. Use Actual size for full-scale plans.")).small().color(t.text_muted));
                    }
                }
            }
        });
        ui.add_space(COLUMN_GAP);
        // Settings.
        ui.vertical(|ui| {
            ui.set_width(layout.left_w);
            egui::ScrollArea::vertical().id_salt("print-settings").max_height(layout.preview_h + 96.0).auto_shrink([false, true]).show(ui, |ui| {
            ui.set_width(layout.left_w - 14.0);
            let (col_x0, col_x1) = (ui.cursor().left(), ui.cursor().left() + layout.left_w - 14.0);
            egui::Grid::new("print-top").num_columns(2).spacing([10.0, 14.0]).show(ui, |ui| {
                ui.label(tl!("Printer:"));
                let before = d.printer.clone();
                // The browser's print dialog sets copies, two-sided printing and colour itself.
                let browser = d.printer.as_deref() == Some(crate::web_print::BROWSER_PRINTER);
                ui.horizontal(|ui| {
                    let shown = match d.printer.as_deref() {
                        Some(p) => printer_label(p),
                        None if still_looking(d) => tl!("Looking for printers…").to_string(),
                        None => tl!("Save as PDF").to_string(),
                    };
                    egui::ComboBox::from_id_salt("printer").selected_text(shown).width(260.0).show_ui(ui, |ui| {
                        if d.printers_loading {
                            ui.add_enabled(false, egui::Label::new(tl!("Looking for printers…")));
                        }
                        for p in &d.printers {
                            let name = printer_label(&p.name);
                            let label = if p.default && p.name != crate::web_print::BROWSER_PRINTER {
                                crate::i18n::fmt(tl!("{name} (default)"), &[("name", &name)])
                            } else {
                                name
                            };
                            ui.selectable_value(&mut d.printer, Some(p.name.clone()), label);
                        }
                        // Chosen even when it was already the selection, so the list arriving
                        // later doesn't replace it with the default printer.
                        if ui.selectable_value(&mut d.printer, None, tl!("Save as PDF")).clicked() {
                            d.printer_picked = true;
                        }
                    });
                    let tip = if spool::HAS_PRINTER_PREFERENCES {
                        tl!("Opens the printer driver's preferences. They are saved for every print from this computer; the copies, two-sided, grayscale and paper chosen here still apply.")
                    } else {
                        tl!("The printer driver's own settings for this print: paper tray, paper type, finishing and more.")
                    };
                    let properties = ui
                        .add_enabled(d.printer.is_some() && !browser, egui::Button::new("⚙").min_size(egui::vec2(28.0, 24.0)))
                        .on_hover_text(tip);
                    if properties.clicked()
                        && let Some(p) = d.printer.clone()
                    {
                        if spool::HAS_PRINTER_PREFERENCES {
                            d.driver_error = spool::open_printer_preferences(&p).err().map(|e| e.to_string());
                        } else {
                            if d.driver_options.as_ref().is_none_or(|(read_for, _)| *read_for != p) {
                                d.driver_options = Some((p.clone(), spool::printer_options(&p)));
                                d.driver_choices.clear();
                            }
                            d.show_driver_options = !d.show_driver_options;
                        }
                    }
                });
                if d.printer != before {
                    d.printer_picked = true;
                    d.show_driver_options = false;
                    d.driver_error = None;
                }
                ui.end_row();
                // The spooler answered with no printer at all (#160): say so, rather than
                // leaving Save as PDF looking like the only thing PdfCraft can do.
                if !d.printers_loading && d.printers.is_empty() {
                    ui.label("");
                    ui.label(
                        egui::RichText::new(tl!("No printers were found. Check that the printer is installed and the print spooler is running, then open Print again."))
                            .small()
                            .color(t.text_muted),
                    );
                    ui.end_row();
                }
                rule_at(ui, t, col_x0, col_x1, ui.cursor().top() - 7.0);
                ui.label(tl!("Number of copies:"));
                ui.add_enabled_ui(!browser, |ui| {
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut d.copies).range(1..=999));
                        ui.add(widgets::checkbox(&mut d.collate, tl!("Collate")));
                    });
                });
                ui.end_row();
                rule_at(ui, t, col_x0, col_x1, ui.cursor().top() - 7.0);
                ui.label(tl!("Paper size:"));
                let paper_name = PAPERS.get(d.paper).map_or("US Letter", |p| p.0);
                let before = d.paper;
                egui::ComboBox::from_id_salt("paper").selected_text(tl!(paper_name)).width(260.0).show_ui(ui, |ui| {
                    for (i, (name, _)) in PAPERS.iter().enumerate() {
                        ui.selectable_value(&mut d.paper, i, tl!(name));
                    }
                });
                // Label stock gets the label layout (cut, turned upright, fitted).
                if d.paper != before && d.paper == print::LABEL_PAPER {
                    d.handling = Handling::Label;
                    d.sheet = 0;
                }
                ui.end_row();
            });
            // Windows says which drivers can't print two-sided as they're asked; they keep their
            // own setting, even when Two-sided is Off.
            if let Some(p) = d.printers.iter().find(|p| d.printer.as_ref() == Some(&p.name))
                && p.two_sided == Some(false)
            {
                ui.label(egui::RichText::new(tl!("This printer's driver doesn't take two-sided settings from other programs, so its own setting applies. Change it in Properties…")).small().color(t.text_muted));
            }
            if d.printer.as_deref() == Some(crate::web_print::BROWSER_PRINTER) {
                ui.label(egui::RichText::new(tl!("Copies, two-sided printing and colour are chosen in the browser's print dialog.")).small().color(t.text_muted));
            }
            if let Some(e) = &d.driver_error {
                ui.label(egui::RichText::new(crate::i18n::fmt(tl!("Couldn't open the printer's preferences: {e}"), &[("e", e)])).small().color(t.text_muted));
            }
            if d.show_driver_options {
                driver_options_panel(ui, d, t);
            }
            if let Some(&source) = sizes.get(d.current_page) {
                rule(ui, t);
                ui.horizontal_wrapped(|ui| {
                    ui.label(crate::i18n::fmt(tl!("PDF page: {size}"), &[("size", &d.units.size(source.0, source.1))]));
                    let matched = print::matching_paper(source);
                    if ui.add_enabled(matched.is_some(), egui::Button::new(tl!("Match page at 100%"))).clicked()
                        && let Some(paper) = matched
                    {
                        d.paper = paper;
                        d.handling = Handling::Size;
                        d.size = SizeMode::Actual;
                        d.orientation = Orientation::Auto;
                        d.sheet = 0;
                    }
                });
            }
            rule(ui, t);
            widgets::section_title(ui, tl!("Pages to Print"));
            ui.horizontal_wrapped(|ui| {
                widgets::radio_value(ui, &mut d.which, Which::All, tl!("All"));
                widgets::radio_value(ui, &mut d.which, Which::Current, tl!("Current page"));
                widgets::radio_value(ui, &mut d.which, Which::CurrentView, tl!("Current view"));
                widgets::radio_value(ui, &mut d.which, Which::Range, tl!("Custom"));
            });
            ui.horizontal(|ui| {
                let r = ui.add_enabled(
                    d.which == Which::Range,
                    crate::widgets::line(&mut d.range).hint_text(format!("1-{}", sizes.len())).desired_width(110.0),
                );
                if r.gained_focus() {
                    d.which = Which::Range;
                }
                ui.label(egui::RichText::new(format!("/ {}", sizes.len())).small().color(t.text_muted));
            });
            // Offered only when pages were picked before the dialog opened.
            if !d.selected.is_empty() {
                let label = format!("{} ({})", tl!("Selected pages"), d.selected.len());
                widgets::radio_value(ui, &mut d.which, Which::Selected, label);
            }
            ui.horizontal(|ui| {
                ui.label(tl!("More options:"));
                combo(
                    ui,
                    "subset",
                    &mut d.subset,
                    &[(Subset::All, tl!("All pages in range")), (Subset::Odd, tl!("Odd pages only")), (Subset::Even, tl!("Even pages only"))],
                    150.0,
                );
                ui.add(widgets::checkbox(&mut d.reverse, tl!("Reverse pages")));
            });
            rule(ui, t);
            widgets::section_title(ui, tl!("Page Sizing & Handling"));
            ui.horizontal_wrapped(|ui| {
                for (h, label) in [
                    (Handling::Size, tl!("Size")),
                    (Handling::Multiple, tl!("Multiple")),
                    (Handling::Booklet, tl!("Booklet")),
                ] {
                    if widgets::segment_tab(ui, label, d.handling == h).clicked() {
                        d.handling = h;
                        d.sheet = 0;
                    }
                }
                // Poster / Label remain available via Advanced menus; keep state if already set.
                if matches!(d.handling, Handling::Poster | Handling::Label) {
                    ui.label(egui::RichText::new(format!("({})", match d.handling {
                        Handling::Poster => tl!("Poster"),
                        Handling::Label => tl!("Label"),
                        _ => "",
                    })).small().color(t.text_muted));
                }
            });
            // JoPDF-like duplex under sizing.
            ui.horizontal(|ui| {
                let mut duplex_on = d.duplex != spool::Duplex::Off;
                if ui.add(widgets::checkbox(&mut duplex_on, tl!("Print on both sides of paper"))).changed() {
                    d.duplex = if duplex_on { spool::Duplex::LongEdge } else { spool::Duplex::Off };
                }
                ui.add_enabled_ui(duplex_on, |ui| {
                    combo(
                        ui,
                        "duplex-edge",
                        &mut d.duplex,
                        &[
                            (spool::Duplex::LongEdge, tl!("Flip on long edge")),
                            (spool::Duplex::ShortEdge, tl!("Flip on short edge")),
                        ],
                        160.0,
                    );
                });
            });
            ui.add_space(4.0);
            match d.handling {
                Handling::Size => {
                    ui.horizontal(|ui| {
                        widgets::radio_value(ui, &mut d.size, SizeMode::Fit, tl!("Fit"));
                        widgets::radio_value(ui, &mut d.size, SizeMode::Actual, tl!("Actual size"));
                        widgets::radio_value(ui, &mut d.size, SizeMode::Shrink, tl!("Shrink oversized pages"));
                    });
                    ui.horizontal(|ui| {
                        let custom = matches!(d.size, SizeMode::Custom(_));
                        if ui.add(widgets::radio(custom, tl!("Custom scale:"))).clicked() {
                            d.size = SizeMode::Custom(d.custom_scale);
                        }
                        ui.add_enabled(custom, egui::DragValue::new(&mut d.custom_scale).range(1.0..=1000.0).suffix(" %"));
                    });
                }
                Handling::Label => {
                    ui.add(widgets::checkbox(&mut d.label_flip, tl!("Turn upside down (180°)")));
                    ui.label(
                        egui::RichText::new(tl!("Cuts each page to its printed area, turns it upright and fits it to the sheet without stretching. One label per sheet."))
                            .small()
                            .color(t.text_muted),
                    );
                }
                Handling::Poster => {
                    ui.horizontal(|ui| {
                        ui.label(tl!("Tile scale:"));
                        ui.add(egui::DragValue::new(&mut d.poster_scale).range(10.0..=1000.0).suffix(" %"));
                        ui.label(tl!("Overlap:"));
                        ui.add(egui::DragValue::new(&mut d.overlap).range(0.0..=144.0).suffix(" pt"));
                        ui.add(widgets::checkbox(&mut d.cut_marks, tl!("Cut marks")));
                    });
                }
                Handling::Multiple => {
                    ui.horizontal(|ui| {
                        ui.label(tl!("Pages per sheet:"));
                        combo(ui, "per-sheet", &mut d.per_sheet, &[(2, "2"), (4, "4"), (6, "6"), (9, "9"), (16, "16")], 60.0);
                        ui.label(tl!("Page order:"));
                        combo(
                            ui,
                            "order",
                            &mut d.order,
                            &[
                                (PageOrder::Horizontal, tl!("Horizontal")),
                                (PageOrder::HorizontalReversed, tl!("Horizontal reversed")),
                                (PageOrder::Vertical, tl!("Vertical")),
                                (PageOrder::VerticalReversed, tl!("Vertical reversed")),
                                (PageOrder::CutStack, tl!("Cut and stack")),
                            ],
                            150.0,
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.add(widgets::checkbox(&mut d.border, tl!("Print page border")));
                        ui.add(widgets::checkbox(&mut d.auto_rotate, tl!("Auto-rotate pages")));
                    });
                    if d.order == PageOrder::CutStack {
                        ui.label(tl!(
                            "Print single-sided. Keep the sheets in order, cut at the marks, then stack the piles left to right, top to bottom."
                        ));
                    }
                }
                Handling::Booklet => {
                    ui.horizontal(|ui| {
                        ui.label(tl!("Booklet subset:"));
                        combo(
                            ui,
                            "booklet",
                            &mut d.booklet_subset,
                            &[
                                (BookletSubset::BothSides, tl!("Both sides")),
                                (BookletSubset::FrontOnly, tl!("Front side only")),
                                (BookletSubset::BackOnly, tl!("Back side only")),
                            ],
                            130.0,
                        );
                        ui.label(tl!("Binding:"));
                        combo(ui, "binding", &mut d.binding, &[(Binding::Left, tl!("Left")), (Binding::Right, tl!("Right"))], 80.0);
                    });
                }
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(tl!("Orientation:"));
                combo(
                    ui,
                    "orientation",
                    &mut d.orientation,
                    &[
                        (Orientation::Auto, tl!("Auto portrait/landscape")),
                        (Orientation::Portrait, tl!("Portrait")),
                        (Orientation::Landscape, tl!("Landscape")),
                    ],
                    180.0,
                );
            });
            ui.horizontal_wrapped(|ui| {
                ui.add(widgets::checkbox(&mut d.auto_center, tl!("Auto-center")));
                ui.add(widgets::checkbox(&mut d.auto_rotate, tl!("Auto-rotate pages")));
                ui.add(widgets::checkbox(&mut d.grayscale, tl!("Print in grayscale")));
            });
            rule(ui, t);
            widgets::section_title(ui, tl!("Advanced"));
            {
                let mut comments = matches!(d.content, Content::DocumentAndMarkups | Content::DocumentAndStamps);
                if ui.add(widgets::checkbox(&mut comments, tl!("Print comments"))).changed() {
                    d.content = if comments { Content::DocumentAndMarkups } else { Content::Document };
                }
            }
            combo(
                ui,
                "content",
                &mut d.content,
                &[
                    (Content::Document, tl!("Document")),
                    (Content::DocumentAndMarkups, tl!("Document and markups")),
                    (Content::DocumentAndStamps, tl!("Document and stamps")),
                    (Content::FormFieldsOnly, tl!("Form fields only")),
                ],
                220.0,
            );
            // Advanced (Acrobat's Advanced Print Setup): print as image. Folded unless it is on.
            rule(ui, t);
            egui::CollapsingHeader::new(egui::RichText::new(tl!("Advanced").to_uppercase()).font(theme::semibold(10.5)).color(t.text_faint))
                .id_salt("print-advanced")
                .default_open(d.as_image)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.add(widgets::checkbox(&mut d.as_image, tl!("Print as image")));
                        ui.add_enabled_ui(d.as_image, |ui| {
                            ui.label(tl!("Resolution:"));
                            let mut dpi = d.dpi.round() as u32;
                            combo(ui, "print-dpi", &mut dpi, &[(150, "150 dpi"), (300, "300 dpi"), (600, "600 dpi")], 100.0);
                            d.dpi = f64::from(dpi);
                        });
                    });
                    ui.label(
                        egui::RichText::new(tl!(
                            "Use it when a printer skips text, shading or transparency. The print is no longer searchable text; with Print in grayscale, the pictures are grey."
                        ))
                        .small()
                        .color(t.text_muted),
                    );
                });
            });
        });
    });
    }); // print_controls_scope
    ui.add_space(8.0);
    rule(ui, t);
    ui.add_space(4.0);
    let (mut go, mut cancel) = (false, false);
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let looking = still_looking(d);
        let label = if d.printer.is_some() || looking { tl!("Print") } else { tl!("Save as PDF") };
        if ui.add_enabled_ui(settings.is_ok() && !sheets.is_empty() && !looking, |ui| widgets::print_primary_button(ui, label)).inner.clicked() {
            go = true;
        }
        if widgets::print_cancel_button(ui, tl!("Cancel")).clicked() {
            cancel = true;
        }
    });
    (go, cancel)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_page_preview_uses_positions_and_selection_guards() {
        for labels in [["i", "ii", "iii", "1", "2"], ["1", "1", "1", "1", "1"], ["1", "2", "3", "4", "5"]] {
            let labels: Vec<String> = labels.into_iter().map(str::to_owned).collect();
            for current_page in 0..5 {
                let draft = PrintDraft { which: Which::Current, current_page, ..Default::default() };
                let rasters = preview_rasters(&draft, &[(200.0, 300.0); 5], &labels, 1.0, 8192.0, &PrintLayout::for_screen(vec2(1440.0, 900.0)));
                assert_eq!(rasters.iter().map(|(page, _)| *page).collect::<Vec<_>>(), vec![current_page]);
            }
        }
        let labels: Vec<String> = ["i", "ii", "iii", "1", "2"].into_iter().map(str::to_owned).collect();
        let mut draft = PrintDraft { which: Which::Selected, selected: vec![0, 3], ..Default::default() };
        assert_eq!(draft.settings(5, &labels).unwrap().pages, vec![0, 3]);
        draft.which = Which::All;
        assert_eq!(draft.settings(5, &labels).unwrap().pages, vec![0, 1, 2, 3, 4]);
        draft.which = Which::Current;
        draft.reverse = true;
        draft.subset = Subset::Odd;
        assert_eq!(draft.settings(5, &labels).unwrap().pages, vec![0]);
        draft.subset = Subset::Even;
        assert!(draft.settings(5, &labels).is_err());
        draft.subset = Subset::All;
        for (count, current) in [(0, 0), (5, 5), (5, usize::MAX)] {
            draft.current_page = current;
            assert!(draft.settings(count, &labels).is_err());
        }
    }

    #[test]
    fn composed_sheets_keep_exact_scaling_and_the_dialogs_duplex_choice() {
        let draft = PrintDraft { duplex: spool::Duplex::LongEdge, ..PrintDraft::default() };
        let job = draft.job("report.pdf");
        assert_eq!(job.scaling, spool::Scaling::Exact);
        assert_eq!(job.duplex, Some(spool::Duplex::LongEdge));
        assert_eq!(PrintDraft::default().job("report.pdf").duplex, Some(spool::Duplex::Off));
    }

    /// A job being spooled holds the progress card, counting its sheets as the spooler reports
    /// them; the card goes when the job ends and its answer arrives as a notice. A card another
    /// long job is showing is left alone.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn the_progress_card_counts_the_sheets_of_a_print_job() {
        let mut app = PdfCraftApp::new();
        app.set_option("language", "en").unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        app.print_tasks.jobs.push(PrintJob { printer: "Office".into(), sheets: None, news: rx });
        app.poll_print();
        assert_eq!(app.progress_notice.as_ref().map(|p| p.label.as_str()), Some("Sending to Office…"));

        tx.send(JobNews::Sent(0, 3)).unwrap();
        tx.send(JobNews::Sent(1, 3)).unwrap();
        app.poll_print();
        let shown = app.progress_notice.clone().unwrap();
        assert_eq!(shown.label, "Sending sheet 2 of 3 to Office…");
        assert!((shown.fraction - 1.0 / 3.0).abs() < 1e-4 && !shown.cancellable, "{shown:?}");

        tx.send(JobNews::Sent(3, 3)).unwrap();
        tx.send(JobNews::Done(Ok(String::new()))).unwrap();
        app.poll_print();
        assert!(app.progress_notice.is_none() && app.print_tasks.jobs.is_empty());
        assert_eq!(app.toast.as_ref().map(|t| t.0.as_str()), Some("Sent to Office"));

        // Another job's card stays; a worker that vanished is reported, not left spinning.
        let other = crate::widgets::ProgressNotice { label: "Optimizing…".into(), fraction: 0.5, cancellable: true };
        app.progress_notice = Some(other.clone());
        let (tx, rx) = std::sync::mpsc::channel();
        app.print_tasks.jobs.push(PrintJob { printer: "Office".into(), sheets: None, news: rx });
        tx.send(JobNews::Sent(1, 2)).unwrap();
        app.poll_print();
        assert_eq!(app.progress_notice, Some(other));
        drop(tx);
        app.poll_print();
        assert!(app.print_tasks.jobs.is_empty() && app.progress_notice.is_some());
        assert_eq!(app.toast.as_ref().map(|t| t.0.as_str()), Some("the print job stopped unexpectedly"));
    }

    /// Issue #1308: Current page is the page on screen, even when a later page is labelled with
    /// its number. A typed range still reads labels first.
    #[test]
    fn current_page_is_the_viewed_page_not_a_label() {
        let labels: Vec<String> = ["i", "ii", "iii", "1", "2"].map(String::from).to_vec();
        for (current, want) in [(0, vec![0]), (1, vec![1]), (3, vec![3])] {
            let d = PrintDraft { which: Which::Current, current_page: current, ..PrintDraft::default() };
            assert_eq!(d.settings(5, &labels).map(|s| s.pages), Ok(want), "viewing page {}", current + 1);
        }
        let typed = PrintDraft { which: Which::Range, range: "1".into(), ..PrintDraft::default() };
        assert_eq!(typed.settings(5, &labels).map(|s| s.pages), Ok(vec![3]));
    }

    #[test]
    fn print_preview_rasters_stay_within_the_texture_limit() {
        // Actual size on a 14,400 pt (200 in) page would ask for a raster far past any texture.
        let d = PrintDraft { size: SizeMode::Actual, ..PrintDraft::default() };
        let sizes = vec![(14_400.0, 14_400.0); 2];
        let labels = vec!["1".into(), "2".into()];
        for max_side in [2048.0, 8192.0, 16384.0, f32::NAN, 0.0] {
            for (page, scale) in preview_rasters(&d, &sizes, &labels, 4.0, max_side, &PrintLayout::for_screen(vec2(1440.0, 900.0))) {
                let long = 14_400.0 * scale;
                let cap = if max_side.is_finite() && max_side >= 1.0 { max_side.min(PREVIEW_MAX_SIDE) } else { PREVIEW_MAX_SIDE };
                assert!(long <= cap + 1.0, "page {page} at {max_side}: {long} px");
            }
        }
    }

    #[test]
    fn print_preview_rasters_keep_the_current_sheet_within_the_byte_budget() {
        // Many large pages: the current sheet always gets its raster; neighbours only while the
        // budget lasts, and the total stays within it.
        let d = PrintDraft { sheet: 5, ..PrintDraft::default() };
        let sizes = vec![(2_000.0, 2_000.0); 12];
        let labels: Vec<String> = (1..=12).map(|n| n.to_string()).collect();
        let rasters = preview_rasters(&d, &sizes, &labels, 4.0, 16384.0, &PrintLayout::for_screen(vec2(1440.0, 900.0)));
        assert_eq!(rasters.first().map(|r| r.0), Some(5), "the sheet on screen comes first: {rasters:?}");
        let total: f64 = rasters.iter().map(|(_, s)| (2_000.0 * f64::from(*s)).ceil().powi(2) * 4.0).sum();
        assert!(total <= PREVIEW_BYTES || rasters.len() == 1, "{total} bytes for {rasters:?}");
    }

    #[test]
    fn print_preview_is_sharper_than_a_thumbnail_on_a_high_dpi_screen() {
        let d = PrintDraft::default();
        let sizes = vec![(612.0, 792.0); 3];
        let labels = vec!["1".into(), "2".into(), "3".into()];
        let rasters = preview_rasters(&d, &sizes, &labels, 2.0, 8192.0, &PrintLayout::for_screen(vec2(1440.0, 900.0)));
        let scale = rasters.iter().find(|(page, _)| *page == 0).map(|(_, s)| *s).unwrap();
        // A 132 pt thumbnail at 2× is about 0.43 px/pt. The preview pane needs roughly twice that.
        assert!(scale > 0.7, "letter page in the preview at 2 px/pt: {scale}");
        assert!(rasters.iter().any(|(page, _)| *page == 1), "the next sheet is ready");
        let mut later = d.clone();
        later.sheet = 2;
        let rasters = preview_rasters(&later, &sizes, &labels, 2.0, 8192.0, &PrintLayout::for_screen(vec2(1440.0, 900.0)));
        assert!(rasters.iter().any(|(page, _)| *page == 2));
    }

    #[test]
    fn preview_shows_a4_as_the_reference_and_other_papers_in_proportion() {
        let layout = PrintLayout::for_screen(vec2(1440.0, 900.0));
        let room = layout.sheet_box();
        let a4 = PAPERS[A4_PAPER].1;
        let k4 = preview_k(a4, room);
        assert!(a4.0 as f32 * k4 <= room.0 + 0.01 && a4.1 as f32 * k4 <= room.1 + 0.01, "A4 fits the pane");
        assert!(((a4.0 as f32 * k4) / (a4.1 as f32 * k4) - 210.0 / 297.0).abs() < 0.002, "A4 aspect");
        // A5 is drawn at the same scale: half the area of A4, not enlarged to fill the pane.
        assert!((preview_k(PAPERS[5].1, room) - k4).abs() < 1e-6);
        // A3 is fitted whole, smaller than A4's scale.
        assert!(preview_k(PAPERS[3].1, room) < k4);
        // Landscape A4 is the same sheet turned: it fills the width.
        assert!(((a4.1 as f32) * preview_k((a4.1, a4.0), room) - room.0).abs() < 0.01);
        assert_eq!(PrintDraft::default().paper, A4_PAPER);
    }

    #[test]
    fn the_wide_dialog_gives_the_preview_more_room_than_arif_4_and_fits_small_windows() {
        let big = PrintLayout::for_screen(vec2(1440.0, 900.0));
        // arif.4's pane was 320 x 380.
        assert!(big.preview_h >= 500.0 && big.preview_w >= 440.0, "{big:?}");
        assert!(big.width() > big.preview_h + 200.0, "wider than tall: {big:?}");
        let small = PrintLayout::for_screen(vec2(820.0, 520.0));
        assert!(small.width() <= 820.0 - 40.0, "fits the smallest window: {small:?}");
        assert!(small.preview_w >= 250.0 && small.preview_h >= 340.0, "{small:?}");
    }
}
