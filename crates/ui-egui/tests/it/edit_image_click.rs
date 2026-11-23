//! Test for arif.29: clicking an image in Edit mode must not freeze.
//! Regression test for the freeze bug in arif.27 that did not exist in upstream v0.21.1.

use egui::Pos2;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use pdfcraft_ui_egui::PdfCraftApp;

/// Creates a PDF with an embedded image: a simple Form XObject with colored rectangle.
fn pdf_with_image() -> Vec<u8> {
    let stream = |body: &str, dict: &str| format!("<< {dict} /Length {} >>\nstream\n{body}\nendstream", body.len());
    let objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".into(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 400 500] /Contents 4 0 R /Resources << /XObject << /Image 5 0 R >> >> >>".into(),
        stream("q 1 0 0 1 100 200 cm /Image Do Q", ""),
        stream("0.2 0.6 0.9 rg 0 0 150 100 re f", "/Type /XObject /Subtype /Form /BBox [0 0 150 100]"),
    ];
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (i, object) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes());
    for offset in offsets {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1).as_bytes());
    out
}

/// Opens a PDF with an image, switches to Edit mode, clicks the image center multiple times,
/// runs several frames without hanging. The freeze bug made this loop infinitely.
#[test]
fn clicking_image_in_edit_mode_does_not_freeze() {
    let mut h = Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_eframe(move |_cc| {
        let mut app = PdfCraftApp::new();
        app.set_option("language", "en").unwrap();
        app.open_bytes("image.pdf", None, pdf_with_image()).unwrap();
        app.set_option("zoom", "100").unwrap();
        app
    });
    h.run_steps(2);

    // Enter Edit mode (Edit Text tool, which shares the object selection machinery)
    assert!(h.state_mut().execute("edit.edit_text"));
    h.run_steps(2);

    // Find the image position on screen: the image is at x=100..250, y=200..300 in PDF coordinates
    let image_center = {
        let app = h.state();
        let view = &app.views[0];
        let r = view.page_screen_rect(0).unwrap();
        // Image center in PDF: (175, 250), page size 400x500
        r.min + egui::vec2(175.0, 250.0) * (r.width() / 400.0)
    };

    // Click the image center multiple times
    for _ in 0..3 {
        h.hover_at(image_center);
        h.run_steps(2);
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton {
                pos: image_center,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
        }
        h.run_steps(4);
    }

    // If we got here without hanging, the bug is fixed.
    // Check that we can still interact with the UI:
    h.get_by_label("1 object selected");

    // Verify that the app is still responsive by checking objects
    let app = h.state();
    let doc = app.session.get(app.views[0].id).unwrap();
    let objects = doc.editable_objects(0).unwrap();
    assert_eq!(objects.len(), 1, "Should have one image object");
}

/// Similar test but with text and an image together, to ensure text interaction doesn't interfere.
#[test]
fn clicking_image_with_text_on_page_works() {
    let mut h = Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_eframe(move |_cc| {
        let mut app = PdfCraftApp::new();
        app.set_option("language", "en").unwrap();
        // PDF with text and image
        let stream = |body: &str, dict: &str| format!("<< {dict} /Length {} >>\nstream\n{body}\nendstream", body.len());
        let objects = vec![
            "<< /Type /Catalog /Pages 2 0 R >>".into(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 400 500] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> /XObject << /Image 6 0 R >> >> >>".into(),
            stream("BT /F1 12 Tf 50 450 Td (Sample text) Tj ET q 1 0 0 1 100 200 cm /Image Do Q", ""),
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into(),
            stream("0.9 0.3 0.1 rg 0 0 120 80 re f", "/Type /XObject /Subtype /Form /BBox [0 0 120 80]"),
        ];
        let mut out = b"%PDF-1.7\n".to_vec();
        let mut offsets = Vec::new();
        for (i, object) in objects.iter().enumerate() {
            offsets.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", i + 1).as_bytes());
        }
        let xref = out.len();
        out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes());
        for offset in offsets {
            out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1).as_bytes());
        app.open_bytes("mixed.pdf", None, out).unwrap();
        app.set_option("zoom", "100").unwrap();
        app
    });
    h.run_steps(2);

    assert!(h.state_mut().execute("edit.edit_text"));
    h.run_steps(2);

    // Click image (at center ~160, 240)
    let image_pos = {
        let app = h.state();
        let r = app.views[0].page_screen_rect(0).unwrap();
        r.min + egui::vec2(160.0, 240.0) * (r.width() / 400.0)
    };

    h.hover_at(image_pos);
    h.run_steps(2);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos: image_pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.run_steps(4);

    // Should select the image without freezing
    h.get_by_label("1 object selected");

    // Now click text (at ~100, 50 from top)
    let text_pos = {
        let app = h.state();
        let r = app.views[0].page_screen_rect(0).unwrap();
        r.min + egui::vec2(100.0, 50.0) * (r.width() / 400.0)
    };

    h.hover_at(text_pos);
    h.run_steps(2);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos: text_pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.run_steps(4);

    // Should open text editor
    assert!(h.state().views[0].line_editor.is_some(), "Text should be editable");
}
