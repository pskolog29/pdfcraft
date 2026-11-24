use super::edit::{actions_menu, edit_panel, stamp_palette};
use super::{
    Align2, Availability, Color32, CornerRadius, LeftPanel, PdfCraftApp, Rect, Sense, Stroke, Tokens, ToolGroup, catalog, icons, theme, vec2, widgets,
};

/// One slate for every tool icon, so the list reads as one corporate palette.
fn icon_color(t: &Tokens) -> Color32 {
    if t.kind == theme::ThemeKind::Dark { Color32::from_rgb(148, 163, 184) } else { Color32::from_rgb(51, 65, 85) }
}

pub fn left_panel(app: &mut PdfCraftApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    // Organize closed another way (the grid's Close, or a page opened from it) closes the tool too,
    // as its panel's Close does, back to the workspace's own panel.
    if app.left == LeftPanel::Tool("organize") && app.active.and_then(|i| app.views.get(i)).is_some_and(|v| !v.organize) {
        app.left = crate::organize_panel::workspace_panel(app.mode);
    }
    egui::Frame::NONE.fill(t.panel).inner_margin(egui::Margin { left: 14, right: 14, top: 12, bottom: 10 }).stroke(Stroke::new(1.0, t.divider)).show(
        ui,
        |ui| match app.left {
            // Organize pages has its own panel whenever the grid shows (with a document open).
            _ if app.active.is_some() && crate::organize_panel::showing(app) => crate::organize_panel::panel(app, ui, &t),
            LeftPanel::AllTools => all_tools(app, ui, &t),
            LeftPanel::Tool(id) => match catalog::group(id) {
                Some(g) => tool_detail(app, ui, &t, g),
                None => app.left = LeftPanel::AllTools,
            },
            LeftPanel::CalculationOrder => crate::calc_order_ui::panel(app, ui, &t),
        },
    );
}

/// A panel's title row: a leading button (`lead`: its icon and tooltip) when given, the title
/// and Close. Returns whether the leading button and Close were clicked.
pub(crate) fn panel_header(ui: &mut egui::Ui, t: &Tokens, title: &str, lead: Option<(&str, &str)>) -> (bool, bool) {
    let mut go_back = false;
    let mut close = false;
    ui.horizontal(|ui| {
        if let Some((icon, tip)) = lead
            && icons::button(ui, icon, 26.0, false, tip).clicked()
        {
            go_back = true;
        }
        // Reserve space for Close before laying out the translated title. Long tool names
        // must never expand the fixed-width sidebar or push the Close control off-screen.
        let title_width = (ui.available_width() - 26.0 - ui.spacing().item_spacing.x).max(1.0);
        ui.add_sized([title_width, 26.0], egui::Label::new(egui::RichText::new(tl!(title)).font(theme::semibold(15.5)).color(t.text)).truncate())
            .on_hover_text(tl!(title));
        if icons::button(ui, "x", 26.0, false, tl!("Close panel")).clicked() {
            close = true;
        }
    });
    ui.add_space(4.0);
    ui.separator();
    ui.add_space(6.0);
    (go_back, close)
}

/// Width kept free at the right of a scrolling list for the floating scrollbar.
const SCROLLBAR_GUTTER: f32 = 18.0;

/// The scrollbar floats over the right edge: keeps the rows of the scroll area `ui` is in (and
/// their rounded hover wash) clear of it.
pub(super) fn clear_of_scrollbar(ui: &mut egui::Ui) {
    ui.set_max_width((ui.available_width() - SCROLLBAR_GUTTER).max(0.0));
}

/// The All tools sidebar: Expand (the full view) and Close, then each catalog category as an
/// accordion. Sections start open so every tool stays reachable.
fn all_tools(app: &mut PdfCraftApp, ui: &mut egui::Ui, t: &Tokens) {
    // Labels distinct from the Combine grid's Expand and Collapse, which can show beside them.
    let (expand, close) = panel_header(ui, t, "All tools", Some(("maximize-2", tl!("Expand all tools"))));
    if expand {
        app.expand_all_tools();
    }
    if close {
        app.left_open = false;
    }
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        clear_of_scrollbar(ui);
        ui.add_space(2.0);
        for cat in catalog::TOOL_CATEGORIES {
            if cat.groups.iter().all(|id| super::hidden_tool_group(id)) {
                continue;
            }
            section(app, ui, t, cat);
        }
    });
}

/// One category heading. A click folds or unfolds its tools. The heading's accessible name is the
/// category label, without the chevron.
fn section(app: &mut PdfCraftApp, ui: &mut egui::Ui, t: &Tokens, cat: &catalog::ToolCategory) {
    let id = ui.scope_id().with(("tool_section", cat.label));
    let mut open = ui.data_mut(|d| d.get_temp::<bool>(id)).unwrap_or(true);
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 28.0), Sense::click());
    let label = tl!(cat.label);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
    if resp.hovered() {
        ui.painter().rect_filled(rect, 6.0, t.hover);
    }
    let mark = if open { "chevron-down" } else { "chevron-right" };
    icons::paint(ui, Rect::from_center_size(rect.left_center() + vec2(10.0, 0.0), vec2(14.0, 14.0)), mark, 14.0, t.text_muted);
    ui.painter().text(rect.left_center() + vec2(22.0, 0.0), egui::Align2::LEFT_CENTER, label, theme::semibold(12.0), t.text);
    if resp.clicked() {
        open = !open;
        ui.data_mut(|d| d.insert_temp(id, open));
    }
    if !open {
        return;
    }
    for gid in cat.groups {
        if super::hidden_tool_group(gid) {
            continue;
        }
        let Some(g) = catalog::group(gid) else { continue };
        let (row, pin_clicked) = tool_row(ui, t, g, app.is_tool_pinned(g.id));
        if pin_clicked {
            app.toggle_pin_tool(g.id);
        } else if row.clicked() {
            app.select_tool(g.id);
        }
    }
}

/// Draw a one-line row label without painting into the status chip or beyond the sidebar.
/// Accessibility retains the complete name through the row's WidgetInfo.
pub(super) fn row_label(ui: &mut egui::Ui, text: &str, font: egui::FontId, color: Color32, left_center: egui::Pos2, max_width: f32) -> bool {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap = egui::text::TextWrapping { max_width: max_width.max(1.0), max_rows: 1, break_anywhere: true, ..Default::default() };
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    let elided = galley.elided;
    ui.painter().galley(left_center - vec2(0.0, galley.size().y / 2.0), galley, color);
    elided
}

/// Draw one tool-group row with its icon, label, badge and availability hint.
fn tool_row(ui: &mut egui::Ui, t: &Tokens, g: &ToolGroup, is_pinned: bool) -> (egui::Response, bool) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, tl!(g.label)));
    let press = theme::Press::track(ui, &resp);
    press.wash(ui, rect, 6, false);
    let body = rect.translate(press.offset());
    icons::paint(ui, Rect::from_min_size(body.min + vec2(6.0, 7.0), vec2(20.0, 20.0)), g.icon, 19.0, icon_color(t));

    let pin_rect = Rect::from_center_size(body.right_center() - vec2(14.0, 0.0), vec2(22.0, 22.0));
    let pin_resp = ui.interact(pin_rect, ui.scope_id().with(("pin_toggle_row", g.id)), Sense::click());
    let pin_hovered = pin_resp.hovered();
    let mut pin_clicked = false;

    let pin_space = 26.0;
    let badge_or_hint_reserved = match (g.badge, g.availability) {
        (Some(b), _) => ui.fonts_mut(|f| f.layout_no_wrap(tl!(b).to_owned(), theme::semibold(9.5), Color32::WHITE).size().x) + 18.0,
        (None, Availability::Planned(m)) if resp.hovered() => {
            let hint = format!("{} · {m}", tl!("Planned"));
            ui.fonts_mut(|f| f.layout_no_wrap(hint, theme::medium(10.5), t.text_faint).size().x) + 10.0
        }
        _ => 0.0,
    };
    let right_reserved = pin_space + badge_or_hint_reserved + 8.0;
    let clipped =
        row_label(ui, tl!(g.label), theme::regular(13.5), t.text, body.left_center() + vec2(36.0, 0.0), rect.width() - 36.0 - right_reserved);
    match (g.badge, g.availability) {
        (Some(b), _) => {
            let font = theme::semibold(9.5);
            let badge = tl!(b);
            let w = ui.fonts_mut(|f| f.layout_no_wrap(badge.to_string(), font.clone(), Color32::WHITE).size().x);
            let r = Rect::from_center_size(body.right_center() - vec2(pin_space + w / 2.0 + 4.0, 0.0), vec2(w + 10.0, 16.0));
            ui.painter().rect_filled(r, CornerRadius::same(4), t.badge_new);
            ui.painter().text(r.center(), Align2::CENTER_CENTER, badge, font, Color32::WHITE);
        }
        // Planned tools: a quiet milestone hint instead of a chip, so the list stays calm.
        (None, Availability::Planned(m)) if resp.hovered() => {
            ui.painter().text(
                body.right_center() - vec2(pin_space + 4.0, 0.0),
                Align2::RIGHT_CENTER,
                format!("{} · {m}", tl!("Planned")),
                theme::medium(10.5),
                t.text_faint,
            );
        }
        _ => {}
    }

    let row_hovered = ui.rect_contains_pointer(rect) || resp.hovered();
    if is_pinned || row_hovered || pin_hovered {
        if pin_hovered {
            ui.painter().rect_filled(pin_rect, CornerRadius::same(4), t.pressed);
        }
        let pin_tint = if is_pinned {
            t.accent_text
        } else if pin_hovered {
            t.text
        } else {
            t.text_faint
        };
        icons::paint(ui, pin_rect.shrink(3.0), "pin", 15.0, pin_tint);
        let tip = if is_pinned { tl!("Unpin from Home") } else { tl!("Pin to Home") };
        pin_resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, tip));
        if theme::hand(pin_resp).on_hover_text(tip).clicked() {
            pin_clicked = true;
        }
    }

    let tip = match g.availability {
        Availability::Ready => tl!("Available").to_string(),
        Availability::Planned(m) => format!("{} {m} — {}", tl!("Planned for milestone"), tl!("open to see what it will include")),
        Availability::Provider => tl!("Optional: needs an AI provider you configure").into(),
    };
    let tip = if clipped { format!("{} — {tip}", tl!(g.label)) } else { tip };
    (theme::hand(resp).on_hover_text(tip), pin_clicked)
}

/// Draw the detail view for one tool group, dispatching its commands and tool-specific panels.
fn tool_detail(app: &mut PdfCraftApp, ui: &mut egui::Ui, t: &Tokens, g: &'static ToolGroup) {
    if g.id == "edit" {
        edit_panel(app, ui, t, g);
        return;
    }
    let (back, close) = panel_header(ui, t, g.label, Some(("chevron-left", tl!("Back to all tools"))));
    if back {
        app.left = LeftPanel::AllTools;
        app.enter_mode(crate::Mode::AllTools);
    }
    if close {
        app.left_open = false;
    }
    if let Availability::Planned(m) = g.availability {
        egui::Frame::NONE.fill(t.accent_soft).corner_radius(CornerRadius::same(8)).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                egui::RichText::new(format!("{} {m}. {}", tl!("Coming in milestone"), tl!("Items marked Ready work today.")))
                    .color(t.text)
                    .font(theme::regular(12.0)),
            );
        });
    }
    if g.id == "measure" {
        crate::measure_ui::panel(app, ui, t);
        return;
    }
    // Add a stamp: the palette.
    if g.id == "stamp" {
        stamp_palette(app, ui, t);
        return;
    }
    // Prepare a form: Preview fills the form as a reader would; Edit goes back.
    if g.id == "form" {
        ui.horizontal(|ui| {
            let label = if app.form_preview { "Edit fields" } else { "Preview" };
            if widgets::pill_button(ui, tl!(label), app.form_preview).on_hover_text(tl!("Try the form as people filling it in will see it")).clicked()
            {
                app.form_preview = !app.form_preview;
                if app.form_preview {
                    app.quick_tool = crate::QuickTool::Select;
                }
            }
        });
        ui.add_space(6.0);
    }
    let mut run = None;
    // Redact a PDF has Acrobat's footer: Clear all / Redact all.
    let footer = g.id == "redact";
    let marks = if footer { app.active_ids().and_then(|(_, id)| app.session.get(id)).map_or(0, |d| d.redaction_marks()) } else { 0 };
    let list_h = if footer { (ui.available_height() - 52.0).max(80.0) } else { ui.available_height() };
    egui::ScrollArea::vertical().auto_shrink([false, false]).max_height(list_h).show(ui, |ui| {
        clear_of_scrollbar(ui);
        ui.add_space(2.0);
        for s in g.sections {
            widgets::section_title(ui, tl!(s.title));
            for item in s.items {
                if g.id == "fill_sign" && (item.command.starts_with("sign.fill.signature") || item.command.starts_with("sign.fill.initials")) {
                    continue;
                }
                let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::click());
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, tl!(item.label)));
                let ready = item.availability == Availability::Ready;
                let press = theme::Press::track(ui, &resp);
                press.wash(ui, rect, 6, false);
                let body = rect.translate(press.offset());
                let fg = if ready { t.text } else { t.text_muted };
                let badge = Rect::from_min_size(body.min + vec2(6.0, 6.0), vec2(20.0, 22.0));
                if !export_badge(ui, badge, item.command) {
                    icons::paint(
                        ui,
                        Rect::from_min_size(body.min + vec2(6.0, 8.0), vec2(18.0, 18.0)),
                        item.icon,
                        17.0,
                        if ready { icon_color(t) } else { t.text_faint },
                    );
                }
                let (chip, fill, cfg) = match item.availability {
                    Availability::Ready => (tl!("Ready"), Color32::from_rgb(0xDD, 0xF3, 0xE4), Color32::from_rgb(0x1E, 0x7B, 0x43)),
                    Availability::Planned(m) => (m, t.pressed, t.text_muted),
                    Availability::Provider => ("AI", t.pressed, t.text_muted),
                };
                let font = theme::semibold(9.5);
                let chip_label = tl!(chip).to_string();
                let w = crate::widgets::text_width(ui, &chip_label, &font);
                let r = Rect::from_center_size(body.right_center() - vec2(w / 2.0 + 10.0, 0.0), vec2(w + 10.0, 16.0));
                row_label(ui, tl!(item.label), theme::regular(13.0), fg, body.left_center() + vec2(34.0, 0.0), r.left() - body.left() - 42.0);
                ui.painter().rect_filled(r, CornerRadius::same(4), fill);
                ui.painter().text(r.center(), Align2::CENTER_CENTER, chip_label, font, cfg);
                let resp = theme::hand(resp).on_hover_text(format!("{} — {}", tl!(item.label), item.command));
                // An entry with further commands opens a menu; its caret goes before the chip.
                if item.actions.is_empty() {
                    if resp.clicked() {
                        run = Some(item.command);
                    }
                } else if let Some(cmd) = actions_menu(app, ui, &resp, item, r.left_center() - vec2(10.0, 0.0), fg) {
                    run = Some(cmd);
                }
            }
        }
        if g.id == "fill_sign" {
            widgets::section_title(ui, tl!("Sign yourself"));
            if let Some(cmd) = crate::fill_sign::signature_entries(ui, app, t) {
                run = Some(cmd);
            }
        }
    });
    if footer {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if marks > 0 {
                ui.label(
                    egui::RichText::new(if marks == 1 {
                        crate::i18n::fmt(tl!("1 mark"), &[])
                    } else {
                        crate::i18n::fmt(tl!("{n} marks"), &[("n", &marks.to_string())])
                    })
                    .color(t.text_muted),
                );
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add_enabled_ui(marks > 0, |ui| widgets::pill_button(ui, tl!("Redact all"), true)).inner.clicked() {
                    run = Some("redact.apply");
                }
                if ui.add_enabled_ui(marks > 0, |ui| widgets::pill_button(ui, tl!("Clear all"), false)).inner.clicked() {
                    run = Some("redact.clear");
                }
            });
        });
    }
    if let Some(cmd) = run {
        app.run_command(cmd);
    }
}

/// A small original page for an export format. The letters name the file type; the
/// shape is ours, not a vendor logo.
fn export_badge(ui: &egui::Ui, rect: Rect, command: &str) -> bool {
    let Some((letters, color, stack)) = export_mark(command) else { return false };
    let painter = ui.painter();
    if stack {
        let back = rect.translate(vec2(3.0, -2.0));
        painter.rect_filled(back, CornerRadius::same(3), Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 150));
    }
    painter.rect_filled(rect, CornerRadius::same(3), color);
    if !stack {
        let fold = Rect::from_min_size(rect.right_top() - vec2(6.0, 0.0), vec2(6.0, 6.0));
        painter.rect_filled(fold, CornerRadius::same(1), Color32::from_white_alpha(210));
    }
    painter.text(rect.center() + vec2(if stack { 0.0 } else { -1.0 }, 1.0), Align2::CENTER_CENTER, letters, theme::semibold(6.5), Color32::WHITE);
    true
}

fn export_mark(command: &str) -> Option<(&'static str, Color32, bool)> {
    Some(match command {
        "export.docx" => ("DOC", Color32::from_rgb(0x3B, 0x6F, 0xD4), false),
        "export.xlsx" => ("XLS", Color32::from_rgb(0x2E, 0x8B, 0x57), false),
        "export.pptx" => ("PPT", Color32::from_rgb(0xE0, 0x7A, 0x2F), false),
        "export.image" => ("PNG", Color32::from_rgb(0x7C, 0x5C, 0xBF), false),
        "export.all_images" => ("IMG", Color32::from_rgb(0x7C, 0x5C, 0xBF), true),
        "export.html" => ("HTM", Color32::from_rgb(0xD9, 0x77, 0x06), false),
        "export.rtf" => ("RTF", Color32::from_rgb(0x0E, 0x74, 0x90), false),
        "export.text" => ("TXT", Color32::from_rgb(0x64, 0x74, 0x8B), false),
        "export.ps" => ("PS", Color32::from_rgb(0x33, 0x41, 0x55), false),
        _ => return None,
    })
}
