//! Arif Pdf Editör: the update check and the "Yeni sürüm var" notice, with stand-in sources (no network).

use std::sync::Arc;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use pdfcraft_ui_egui::PdfCraftApp;
use pdfcraft_ui_egui::updates::{Release, UpdateSource, is_newer};

const SHA: &str = "27DF9EFFD73BB6343046ED8B3199A3207FB9CF854C6FA99415CB26D95DDCDD83";

fn release(v: &str) -> Release {
    Release { version: v.to_string(), url: "http://10.129.0.29/program/".into(), sha256: SHA.into(), size: 1 }
}

fn source(answer: Result<&str, &str>) -> UpdateSource {
    let answer = answer.map(str::to_string).map_err(str::to_string);
    Arc::new(move || answer.clone().map(|v| release(&v)))
}

fn harness(answer: Result<&str, &str>) -> Harness<'static, PdfCraftApp> {
    Harness::builder().with_size(egui::vec2(1200.0, 800.0)).build_eframe(move |_cc| {
        let mut app = PdfCraftApp::new();
        app.set_option("language", "en").unwrap();
        // Never arm the real installer from a test.
        app.auto_update = false;
        app.update_source = Some(source(answer));
        app
    })
}

/// Run frames until `done` holds, with a 30-second deadline: under load the check's thread can
/// take longer (#1277).
fn wait_for(h: &mut Harness<'static, PdfCraftApp>, what: &str, done: impl Fn(&Harness<'static, PdfCraftApp>) -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        h.run_steps(2);
        if done(h) {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    h.run_steps(2);
}

#[test]
fn versions_compare_by_build_number() {
    assert!(is_newer("0.9.0-arif.5", "0.9.0-arif.4"));
    assert!(is_newer("0.9.0-arif.10", "0.9.0-arif.9"));
    assert!(!is_newer("0.9.0-arif.4", "0.9.0-arif.4"));
    assert!(!is_newer("0.9.0-arif.3", "0.9.0-arif.4"));
    assert!(!is_newer("nightly", "0.9.0-arif.4"), "a tag that isn't a version is never newer");
}

#[test]
fn a_newer_version_shows_the_notice_at_start_without_any_command() {
    let mut h = harness(Ok("99.0.0-arif.1"));
    wait_for(&mut h, "the notice", |h| h.query_by_label_contains("Yeni sürüm var").is_some());
    h.get_by_label("Güncelle");
    h.get_by_label("Sonra").click();
    h.run_steps(3);
    assert!(h.query_by_label_contains("Yeni sürüm var").is_none(), "Sonra hides the notice");
}

#[test]
fn an_offline_start_stays_silent() {
    let mut h = harness(Err("ağ yok"));
    h.run_steps(30);
    std::thread::sleep(std::time::Duration::from_millis(100));
    h.run_steps(5);
    assert!(h.query_by_label_contains("Yeni sürüm var").is_none());
    assert!(h.query_by_label_contains("ulaşılamadı").is_none(), "no dialog opens by itself");
}

#[test]
fn the_start_check_asks_once_and_help_asks_again() {
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = calls.clone();
    let mut h = Harness::builder().with_size(egui::vec2(1200.0, 800.0)).build_eframe(move |_cc| {
        let mut app = PdfCraftApp::new();
        app.set_option("language", "en").unwrap();
        app.auto_update = false;
        let counted = counted.clone();
        app.update_source = Some(Arc::new(move || {
            counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(release("0.0.1-arif.1"))
        }));
        app
    });
    wait_for(&mut h, "the start check", |_| calls.load(std::sync::atomic::Ordering::SeqCst) >= 1);
    h.run_steps(20);
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    h.state_mut().execute("help.check_updates");
    wait_for(&mut h, "the dialog's check", |h| h.query_by_label_contains("güncel").is_some());
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
}

#[test]
fn the_dialog_says_what_it_found() {
    let mut h = harness(Ok("99.0.0-arif.1"));
    h.state_mut().execute("help.check_updates");
    wait_for(&mut h, "the dialog", |h| h.query_by_label_contains("Yeni sürüm var: 99.0.0-arif.1").is_some());
    h.get_by_label("Sonra").click();
    h.run_steps(3);

    let mut h = harness(Err("ağ yok"));
    h.state_mut().execute("help.check_updates");
    wait_for(&mut h, "the dialog", |h| h.query_by_label_contains("Güncelleme sunucusuna ulaşılamadı: ağ yok").is_some());
}
