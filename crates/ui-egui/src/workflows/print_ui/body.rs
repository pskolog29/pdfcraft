use super::draft::{Handling, PrintDraft, Which};
use super::{
    Binding, BookletSubset, Color32, Content, Orientation, PAPERS, PREVIEW_BOX, PREVIEW_H, PREVIEW_W, PageOrder, Pos2, Rect, SizeMode, Stroke,
    Subset, Tokens, pos2, print, printer_label, spool, still_looking, theme, vec2, widgets,
};

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
                        let current = choices.get(&o.key).cloned().unwrap_or_else(|| o.default.clone());
                        let label_of = |c: &str| o.choices.iter().find(|(k, _)| k == c).map_or_else(|| c.to_string(), |(_, l)| l.clone());
                        let mut pick = current.clone();
                        let option_label = ui.label(&o.label);
                        craft_ui::ComboBox::from_id_salt(("driver-option", &o.key))
                            .selected_text(label_of(&current))
                            .width(200.0)
                            .show_ui(ui, |ui| {
                                for (k, l) in &o.choices {
                                    ui.selectable_value(&mut pick, k.clone(), l.as_str());
                                }
                            })
                            .response
                            .labelled_by(option_label.id);
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

/// Draw a combo box whose accessible name does not depend on the selected value.
fn combo<T: PartialEq + Copy>(ui: &mut egui::Ui, id: &str, name: &str, value: &mut T, choices: &[(T, &str)], width: f32) {
    let shown = choices.iter().find(|c| c.0 == *value).map_or("", |c| c.1);
    craft_ui::ComboBox::from_id_salt(id)
        .selected_text(shown)
        .width(width)
        .show_ui(ui, |ui| {
            for (v, label) in choices {
                ui.selectable_value(value, *v, *label);
            }
        })
        .response
        .accessible_name(name);
}

/// A section name with a soft rule under it, in the dialog's own case (not small capitals).
fn soft_heading(ui: &mut egui::Ui, t: &Tokens, text: &str) {
    ui.add_space(8.0);
    let resp = ui.label(egui::RichText::new(text).font(theme::semibold(13.0)).color(t.text));
    let y = resp.rect.bottom() + 3.0;
    ui.painter().hline(resp.rect.left()..=ui.max_rect().right(), y, Stroke::new(1.0, Color32::from_rgb(0xD0, 0xD4, 0xDB)));
    ui.add_space(8.0);
}

/// One compact choice in the sizing row. The label stays a button the tests can click.
fn segment(ui: &mut egui::Ui, t: &Tokens, label: &str, on: bool) -> egui::Response {
    let color = if on { t.text } else { t.text_muted };
    let galley = ui.fonts_mut(|f| f.layout_no_wrap(label.to_owned(), theme::medium(12.0), color));
    let (rect, resp) = ui.allocate_exact_size(vec2((galley.size().x + 18.0).max(72.0), 28.0), egui::Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
    if on {
        ui.painter().rect_filled(rect, 6.0, t.card);
        ui.painter().rect_stroke(rect, 6.0, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
    }
    ui.painter().galley(rect.center() - galley.size() * 0.5, galley, color);
    resp
}

/// The red print action. Cancel stays the ordinary pill.
fn print_action(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let galley = ui.fonts_mut(|f| f.layout_no_wrap(label.to_owned(), theme::medium(13.0), Color32::WHITE));
    let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 28.0, 32.0), egui::Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
    let fill = if resp.hovered() { Color32::from_rgb(0xC4, 0x2B, 0x2B) } else { Color32::from_rgb(0xD9, 0x32, 0x32) };
    ui.painter().rect_filled(rect, 6.0, fill);
    ui.painter().galley(rect.center() - galley.size() * 0.5, galley, Color32::WHITE);
    resp
}

/// Draw the dialog. `thumb` gives a page's thumbnail texture when there is one. Returns
/// (print, cancel).
pub(crate) fn body(
    ui: &mut egui::Ui,
    d: &mut PrintDraft,
    t: &Tokens,
    file_name: &str,
    sizes: &[(f64, f64)],
    labels: &[String],
    thumb: &mut dyn FnMut(usize) -> Option<egui::TextureId>,
) -> (bool, bool) {
    ui.set_width(860.0);
    ui.label(egui::RichText::new(tl!("Print")).font(theme::semibold(16.0)));
    ui.add_space(4.0);
    let settings = d.settings(sizes.len(), labels);
    let sheets = settings.as_ref().ok().and_then(|s| print::layout(sizes, s).ok()).unwrap_or_default();
    d.sheet = d.sheet.min(sheets.len().saturating_sub(1));
    ui.horizontal_top(|ui| {
        preview_pane(ui, d, t, file_name, sizes, settings.as_ref().err().map(String::as_str), &sheets, thumb);
        ui.add_space(16.0);
        // Settings, on the right.
        ui.vertical(|ui| {
            ui.set_width(500.0);
            egui::Grid::new("print-top").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
                let printer_caption = ui.label(tl!("Printer:"));
                let before = d.printer.clone();
                // The browser's print dialog sets copies, two-sided printing and colour itself.
                let browser = d.printer.as_deref() == Some(crate::web_print::BROWSER_PRINTER);
                ui.horizontal(|ui| {
                    let shown = match d.printer.as_deref() {
                        Some(p) => printer_label(p),
                        None if still_looking(d) => tl!("Looking for printers…").to_string(),
                        None => tl!("Save as PDF").to_string(),
                    };
                    craft_ui::ComboBox::from_id_salt("printer").selected_text(shown).width(210.0).show_ui(ui, |ui| {
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
                    })
                    .response
                    .labelled_by(printer_caption.id);
                    let tip = if spool::HAS_PRINTER_PREFERENCES {
                        tl!("Opens the printer driver's preferences. They are saved for every print from this computer; the copies, two-sided, grayscale and paper chosen here still apply.")
                    } else {
                        tl!("The printer driver's own settings for this print: paper tray, paper type, finishing and more.")
                    };
                    let properties = ui.add_enabled(d.printer.is_some() && !browser, egui::Button::new(tl!("Properties…"))).on_hover_text(tip);
                    if properties.clicked()
                        && let Some(p) = d.printer.clone()
                    {
                        if spool::HAS_PRINTER_PREFERENCES {
                            d.driver_error = spool::open_printer_preferences(&p).err().map(|e| e.to_string());
                        } else {
                            if d.driver_options.as_ref().is_none_or(|(read_for, _)| *read_for != p) && d.driver_options_loading.as_deref() != Some(p.as_str()) {
                                // Read off the GUI thread (#1818): `poll_print` starts it next frame.
                                d.driver_options = None;
                                d.driver_choices.clear();
                                d.driver_options_loading = Some(p.clone());
                                d.read_options_for = Some(p);
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
                let copies_label = ui.label(tl!("Copies:"));
                ui.add_enabled_ui(!browser, |ui| {
                    ui.add(egui::DragValue::new(&mut d.copies).range(1..=999)).labelled_by(copies_label.id);
                });
                ui.end_row();
                ui.label(tl!("Orientation:"));
                combo(
                    ui,
                    "orient",
                    tl!("Orientation:"),
                    &mut d.orientation,
                    &[
                        (Orientation::Auto, tl!("Auto portrait/landscape")),
                        (Orientation::Portrait, tl!("Portrait")),
                        (Orientation::Landscape, tl!("Landscape")),
                    ],
                    210.0,
                );
                ui.end_row();
                // A booklet decides two-sided itself (#2707), so the choice is hidden meanwhile, as
                // in Acrobat's Print dialog; it is kept for the other layouts.
                if d.handling != Handling::Booklet {
                    ui.label(tl!("Two-sided:"));
                    ui.add_enabled_ui(!browser, |ui| {
                        combo(
                            ui,
                            "duplex",
                            tl!("Two-sided:"),
                            &mut d.duplex,
                            &[
                                (None, tl!("Printer default")),
                                (Some(spool::Duplex::Off), tl!("Off")),
                                (Some(spool::Duplex::LongEdge), tl!("Flip on long edge")),
                                (Some(spool::Duplex::ShortEdge), tl!("Flip on short edge")),
                            ],
                            160.0,
                        );
                    });
                    ui.end_row();
                }
                let paper_label = ui.label(tl!("Paper:"));
                let paper_name = PAPERS.get(d.paper).map_or("US Letter", |p| p.0);
                let before = d.paper;
                craft_ui::ComboBox::from_id_salt("paper").selected_text(tl!(paper_name)).width(210.0).show_ui(ui, |ui| {
                    for (i, (name, _)) in PAPERS.iter().enumerate() {
                        ui.selectable_value(&mut d.paper, i, tl!(name));
                    }
                })
                .response
                .labelled_by(paper_label.id);
                // Label stock gets the label layout (cut, turned upright, fitted).
                if d.paper != before && d.paper == print::LABEL_PAPER {
                    d.paper_before_label = Some(before);
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
                if d.driver_options.is_none() && d.driver_options_loading.is_some() {
                    ui.label(egui::RichText::new(tl!("Reading the printer's settings…")).small().color(t.text_muted));
                } else {
                    driver_options_panel(ui, d, t);
                }
            }
            ui.horizontal_wrapped(|ui| {
                ui.add(widgets::checkbox(&mut d.collate, tl!("Collate")));
                let mut center = matches!(d.size, SizeMode::Fit);
                if ui.add(widgets::checkbox(&mut center, tl!("Center on page"))).changed() {
                    d.size = if center { SizeMode::Fit } else { SizeMode::Actual };
                }
                ui.add(widgets::checkbox(&mut d.auto_rotate, tl!("Auto-rotate pages")));
                let browser = d.printer.as_deref() == Some(crate::web_print::BROWSER_PRINTER);
                ui.add_enabled_ui(!browser, |ui| {
                    ui.add(widgets::checkbox(&mut d.grayscale, tl!("Print in grayscale")));
                });
            });
            if let Some(&source) = sizes.get(d.current_page) {
                ui.horizontal_wrapped(|ui| {
                    ui.label(crate::i18n::fmt(tl!("PDF page: {size}"), &[("size", &d.units.size(source.0, source.1))]));
                    let matched = print::matching_paper(source);
                    if ui.add_enabled(matched.is_some(), egui::Button::new(tl!("Match page at 100%"))).clicked()
                        && let Some(paper) = matched
                    {
                        d.paper = paper;
                        // A paper picked outright: nothing for leaving Label to give back.
                        d.paper_before_label = None;
                        d.handling = Handling::Size;
                        d.size = SizeMode::Actual;
                        d.orientation = Orientation::Auto;
                        d.sheet = 0;
                    }
                });
            }
            ui.add_space(6.0);
            soft_heading(ui, t, tl!("Pages to Print"));
            ui.horizontal(|ui| {
                widgets::radio_value(ui, &mut d.which, Which::All, tl!("All"));
                widgets::radio_value(ui, &mut d.which, Which::Current, tl!("Current page"));
                let pages = widgets::radio_value(ui, &mut d.which, Which::Range, tl!("Pages"));
                let r = ui.add_enabled(
                    d.which == Which::Range,
                    crate::widgets::line(&mut d.range).hint_text(format!("1-{}", sizes.len())).desired_width(110.0),
                ).labelled_by(pages.id);
                if r.gained_focus() {
                    d.which = Which::Range;
                }
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
                    tl!("More options:"),
                    &mut d.subset,
                    &[(Subset::All, tl!("All pages in range")), (Subset::Odd, tl!("Odd pages only")), (Subset::Even, tl!("Even pages only"))],
                    150.0,
                );
                ui.add(widgets::checkbox(&mut d.reverse, tl!("Reverse pages")));
            });
            ui.add_space(6.0);
            soft_heading(ui, t, tl!("Page Sizing & Handling"));
            egui::Frame::NONE
                .fill(t.hover)
                .corner_radius(egui::CornerRadius::same(8))
                .inner_margin(egui::Margin::symmetric(4, 4))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        for (h, label) in [
                            (Handling::Size, tl!("Size")),
                            (Handling::Multiple, tl!("Multiple")),
                            (Handling::Booklet, tl!("Booklet")),
                            (Handling::Poster, tl!("Poster")),
                            (Handling::Label, tl!("Label")),
                        ] {
                            if segment(ui, t, label, d.handling == h).clicked() {
                                d.set_handling(h);
                            }
                        }
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
                        ui.add_enabled(custom, egui::DragValue::new(&mut d.custom_scale).range(1.0..=1000.0).suffix(" %"))
                            .accessible_name(tl!("Custom scale"));
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
                        let scale_label = ui.label(tl!("Tile scale:"));
                        ui.add(egui::DragValue::new(&mut d.poster_scale).range(10.0..=1000.0).suffix(" %")).labelled_by(scale_label.id);
                        let overlap_label = ui.label(tl!("Overlap:"));
                        ui.add(egui::DragValue::new(&mut d.overlap).range(0.0..=144.0).suffix(" pt")).labelled_by(overlap_label.id);
                        ui.add(widgets::checkbox(&mut d.cut_marks, tl!("Cut marks")));
                    });
                }
                Handling::Multiple => {
                    ui.horizontal(|ui| {
                        ui.label(tl!("Pages per sheet:"));
                        combo(ui, "per-sheet", tl!("Pages per sheet:"), &mut d.per_sheet, &[(2, "2"), (4, "4"), (6, "6"), (9, "9"), (16, "16")], 60.0);
                        ui.label(tl!("Page order:"));
                        combo(
                            ui,
                            "order",
                            tl!("Page order:"),
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
                    ui.add(widgets::checkbox(&mut d.border, tl!("Print page border")));
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
                            tl!("Booklet subset:"),
                            &mut d.booklet_subset,
                            &[
                                (BookletSubset::BothSides, tl!("Both sides")),
                                (BookletSubset::FrontOnly, tl!("Front side only")),
                                (BookletSubset::BackOnly, tl!("Back side only")),
                            ],
                            130.0,
                        );
                        ui.label(tl!("Binding:"));
                        combo(ui, "binding", tl!("Binding:"), &mut d.binding, &[(Binding::Left, tl!("Left")), (Binding::Right, tl!("Right"))], 80.0);
                    });
                }
            }
            soft_heading(ui, t, tl!("Advanced"));
            combo(
                ui,
                "content",
                tl!("Comments & Forms"),
                &mut d.content,
                &[
                    (Content::Document, tl!("Document")),
                    (Content::DocumentAndMarkups, tl!("Document and markups")),
                    (Content::DocumentAndStamps, tl!("Document and stamps")),
                    (Content::FormFieldsOnly, tl!("Form fields only")),
                ],
                220.0,
            );
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add(widgets::checkbox(&mut d.as_image, tl!("Print as image")));
                ui.add_enabled_ui(d.as_image, |ui| {
                    ui.label(tl!("Resolution:"));
                    let mut dpi = d.dpi.round() as u32;
                    combo(ui, "print-dpi", tl!("Resolution:"), &mut dpi, &[(150, "150 dpi"), (300, "300 dpi"), (600, "600 dpi")], 100.0);
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
    ui.add_space(12.0);
    let (mut go, mut cancel) = (false, false);
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let looking = still_looking(d);
        let label = if d.printer.is_some() || looking { tl!("Print") } else { tl!("Save as PDF") };
        if ui.add_enabled_ui(settings.is_ok() && !sheets.is_empty() && !looking, |ui| print_action(ui, label)).inner.clicked() {
            go = true;
        }
        if widgets::pill_button(ui, tl!("Cancel"), false).clicked() {
            cancel = true;
        }
    });
    (go, cancel)
}

/// The gray preview well on the left: the file name, the current sheet, and the sheet stepper.
fn preview_pane(
    ui: &mut egui::Ui,
    d: &mut PrintDraft,
    t: &Tokens,
    file_name: &str,
    sizes: &[(f64, f64)],
    error: Option<&str>,
    sheets: &[print::Sheet],
    thumb: &mut dyn FnMut(usize) -> Option<egui::TextureId>,
) {
    ui.vertical(|ui| {
        ui.set_width(PREVIEW_W + 20.0);
        egui::Frame::NONE
            .fill(Color32::from_rgb(0xE6, 0xE7, 0xEB))
            .corner_radius(egui::CornerRadius::same(8))
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                ui.set_width(PREVIEW_W);
                if !file_name.is_empty() {
                    ui.vertical_centered(|ui| {
                        egui::Frame::NONE
                            .fill(Color32::WHITE)
                            .corner_radius(egui::CornerRadius::same(10))
                            .inner_margin(egui::Margin::symmetric(12, 4))
                            .show(ui, |ui| {
                                ui.label(egui::RichText::new(file_name).font(theme::medium(12.0)).color(t.text));
                            });
                    });
                    ui.add_space(8.0);
                }
                let (area, _) = ui.allocate_exact_size(vec2(PREVIEW_W, PREVIEW_H), egui::Sense::hover());
                if let Some(e) = error {
                    ui.put(area.shrink(16.0), egui::Label::new(egui::RichText::new(tl!(e)).color(t.text_muted)).wrap());
                } else if let Some(sheet) = sheets.get(d.sheet) {
                    let k = (PREVIEW_BOX.0 / sheet.size.0 as f32).min(PREVIEW_BOX.1 / sheet.size.1 as f32);
                    let paper = Rect::from_center_size(area.center(), vec2(sheet.size.0 as f32 * k, sheet.size.1 as f32 * k));
                    ui.painter().rect_filled(paper, 0.0, Color32::WHITE);
                    ui.painter().rect_stroke(paper, 0.0, Stroke::new(1.0, t.border), egui::StrokeKind::Outside);
                    let to_screen = |x: f64, y: f64| -> Pos2 { pos2(paper.left() + x as f32 * k, paper.bottom() - y as f32 * k) };
                    let painter = ui.painter().with_clip_rect(paper);
                    for pl in &sheet.placed {
                        let Some(&(dw, dh)) = sizes.get(pl.page) else { continue };
                        let [x0, y0, x1, y1] = pl.clip;
                        let corners = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)].map(|(x, y)| {
                            let (sx, sy) = pl.matrix.apply(x, y);
                            to_screen(sx, sy)
                        });
                        let uv = |x: f64, y: f64| pos2((x / dw) as f32, (1.0 - y / dh) as f32);
                        let uvs = [uv(x0, y0), uv(x1, y0), uv(x1, y1), uv(x0, y1)];
                        match thumb(pl.page) {
                            Some(tex) => {
                                let mut mesh = egui::Mesh::with_texture(tex);
                                for (p, u) in corners.iter().zip(uvs) {
                                    mesh.vertices.push(egui::epaint::Vertex {
                                        pos: *p,
                                        uv: u,
                                        color: if d.grayscale { Color32::from_gray(235) } else { Color32::WHITE },
                                    });
                                }
                                mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
                                painter.add(egui::Shape::mesh(mesh));
                            }
                            None => {
                                painter.add(egui::Shape::convex_polygon(
                                    corners.to_vec(),
                                    Color32::from_gray(245),
                                    Stroke::new(0.5, Color32::from_gray(180)),
                                ));
                                let c = corners.iter().fold(vec2(0.0, 0.0), |a, p| a + p.to_vec2()) / 4.0;
                                painter.text(
                                    c.to_pos2(),
                                    egui::Align2::CENTER_CENTER,
                                    (pl.page + 1).to_string(),
                                    theme::regular(11.0),
                                    Color32::from_gray(120),
                                );
                            }
                        }
                    }
                    for b in &sheet.borders {
                        painter.rect_stroke(
                            Rect::from_two_pos(to_screen(b[0], b[1]), to_screen(b[2], b[3])),
                            0.0,
                            Stroke::new(0.6, Color32::BLACK),
                            egui::StrokeKind::Middle,
                        );
                    }
                    for l in &sheet.lines {
                        painter.line_segment([to_screen(l[0], l[1]), to_screen(l[2], l[3])], Stroke::new(0.6, Color32::BLACK));
                    }
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.add_space((PREVIEW_W - 180.0).max(0.0) / 2.0);
                    let n = sheets.len();
                    if ui.add_enabled(d.sheet > 0, egui::Button::new("‹")).on_hover_text(tl!("Previous sheet")).clicked() {
                        d.sheet -= 1;
                    }
                    let full = if n == 0 {
                        tl!("No sheets").to_string()
                    } else {
                        crate::i18n::fmt(tl!("Sheet {s} of {n}"), &[("s", &(d.sheet + 1).to_string()), ("n", &n.to_string())])
                    };
                    ui.label(egui::RichText::new(full).font(theme::medium(12.5)).color(t.text));
                    if ui.add_enabled(d.sheet + 1 < n, egui::Button::new("›")).on_hover_text(tl!("Next sheet")).clicked() {
                        d.sheet += 1;
                    }
                });
                if let Some(s) = sheets.get(d.sheet) {
                    ui.label(egui::RichText::new(d.units.size(s.size.0, s.size.1)).small().color(t.text_muted));
                    if d.handling == Handling::Size
                        && let Some(p) = s.placed.first()
                    {
                        let scale = p.matrix.0[0].hypot(p.matrix.0[1]) * 100.0;
                        ui.label(crate::i18n::fmt(tl!("PDF scale: {scale}%"), &[("scale", &format!("{scale:.2}"))]));
                        ui.label(
                            egui::RichText::new(tl!("Printer paper and scaling may differ. Check driver settings.")).small().color(t.text_muted),
                        );
                        if (scale - 100.0).abs() > 0.01 {
                            ui.label(
                                egui::RichText::new(tl!("Drawing scale changes. Use Actual size for full-scale plans.")).small().color(t.text_muted),
                            );
                        }
                    }
                }
            });
    });
}
