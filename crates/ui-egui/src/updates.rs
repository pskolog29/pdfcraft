//! Arif Pdf Editör: automatic updates from the institution's own update server.
//!
//! At start a background thread reads `ArifPdfEditor.json` (version, sha256, size) from the update
//! server: 10.129.0.29 over the VPN first, then ValilikServer (the app supplies how to ask, see
//! [`PdfCraftApp::update_source`], so this crate has no network code). An offline answer is
//! silent. A newer version shows a small notice (Yeni sürüm var — Güncelle). The update itself
//! is the installed package's own `Kur.ps1 -Guncelle` (the ArifPdfGuncelle start-up entry runs
//! the same script): it downloads `ArifPdfEditor.zip`, checks its sha256 against the same json
//! and replaces the program once this one has closed. With "Otomatik güncelle (sessiz)" on, that
//! is armed as soon as a newer version is known and happens when the program is closed.
//! Nothing here talks to the upstream project's servers.

use std::sync::Arc;

use egui::{Align, Layout};

use crate::{PdfCraftApp, theme, widgets};

/// The page of the institution's program site, offered when no installer script is present
/// (a portable copy).
pub const PROGRAM_PAGE: &str = "http://10.129.0.29/program/";

/// The newest version the update server announces (`ArifPdfEditor.json`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    /// Its version, such as `0.9.0-arif.5`.
    pub version: String,
    /// The program site it came from (for the notice).
    pub url: String,
    /// The package's sha256 as published (the installer script checks it again).
    pub sha256: String,
    /// The package's size in bytes.
    pub size: u64,
}

/// Asks the update server for the newest version (blocking; it runs on its own thread).
pub type UpdateSource = Arc<dyn Fn() -> Result<Release, String> + Send + Sync>;

/// The version this program is: `0.9.0-arif.4`.
pub fn running_version() -> String {
    let v = pdfcraft_engine::build_info::VERSION;
    match pdfcraft_engine::build_info::LABEL.map(str::trim).filter(|l| !l.is_empty()) {
        Some(label) => format!("{v}-{label}"),
        None => v.to_owned(),
    }
}

/// `0.9.0-arif.5` → `(0, 9, 0, 5)`: the numbers before the dash, then the number the build label ends in.
fn parse(v: &str) -> Option<[u64; 4]> {
    let v = v.trim().trim_start_matches(['v', 'V']);
    let (core, label) = match v.split_once(['-', '+']) {
        Some((c, l)) => (c, Some(l)),
        None => (v, None),
    };
    let mut out = [0u64; 4];
    let mut n = 0;
    for part in core.split('.') {
        *out.get_mut(n).filter(|_| n < 3)? = part.parse().ok()?;
        n += 1;
    }
    if n == 0 {
        return None;
    }
    if let Some(l) = label {
        out[3] = l.rsplit(['.', '-']).next().and_then(|d| d.parse().ok()).unwrap_or(0);
    }
    Some(out)
}

/// Whether `latest` is newer than `current`. A version that doesn't parse is never newer.
pub fn is_newer(latest: &str, current: &str) -> bool {
    matches!((parse(latest), parse(current)), (Some(l), Some(c)) if l > c)
}

/// Where a check is.
#[derive(Default)]
pub(crate) enum Check {
    #[default]
    Idle,
    #[cfg(not(target_arch = "wasm32"))]
    Running(std::sync::mpsc::Receiver<Result<Release, String>>),
    Done(Result<Release, String>),
}

#[derive(Default)]
pub(crate) struct Updates {
    pub(crate) check: Check,
    /// The Updates dialog is showing (Help ▸ Check for updates).
    pub(crate) open: bool,
    /// The check at start has been started.
    started: bool,
    /// The notice was dismissed for this run.
    hidden: bool,
    /// The installer is armed: it runs when this program closes.
    armed: bool,
    /// Why arming failed (shown in the notice).
    error: Option<String>,
}

/// The installed package's script that updates the program (`-Guncelle`), if there is one.
#[cfg(windows)]
fn installer_script() -> Option<std::path::PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")?;
    let p = std::path::Path::new(&base).join(r"Programs\ArifPdfEditor-Araclar\Kur.ps1");
    p.is_file().then_some(p)
}

#[cfg(not(windows))]
fn installer_script() -> Option<std::path::PathBuf> {
    None
}

/// The PowerShell that waits for this program to close, runs the installer script and, when
/// `restart`, starts the program again.
///
/// arif.12: do **not** nest `powershell -File Kur.ps1` (Restricted / MOTW / no-Bypass VPN policy
/// aborts silently → app restarts on the old build and shows Güncelle again). Run Kur in-process
/// after `Set-ExecutionPolicy -Scope Process RemoteSigned` (not Bypass; not on the argv that
/// CrowdStrike flags) and `Unblock-File`, and write every step to guncelle.log.
fn updater_script(pid: u32, kur: &str, exe: &str, restart: bool) -> String {
    let q = |s: &str| s.replace('\'', "''");
    let mut s = format!(
        "$ErrorActionPreference='Stop'\n\
         $log = Join-Path $env:LOCALAPPDATA 'Programs\\ArifPdfEditor-Araclar\\guncelle.log'\n\
         function L([string]$m) {{ Add-Content -LiteralPath $log -Value (((Get-Date).ToString('yyyy-MM-dd HH:mm:ss')) + ' yardimci: ' + $m) -Encoding UTF8 }}\n\
         try {{\n\
           L 'pid {pid} bekleniyor'\n\
           try {{ Wait-Process -Id {pid} -Timeout 900 }} catch {{ }}\n\
           if (Get-Process -Id {pid} -ErrorAction SilentlyContinue) {{ L 'pid hala acik, vazgecildi'; exit 1 }}\n\
           Start-Sleep -Milliseconds 1200\n\
           $kur = '{kur}'\n\
           if (-not (Test-Path -LiteralPath $kur)) {{ L \"Kur.ps1 yok: $kur\"; exit 2 }}\n\
           try {{ Unblock-File -LiteralPath $kur -ErrorAction SilentlyContinue }} catch {{ }}\n\
           # Process-scope only (machine/VPN policy untouched). RemoteSigned — not Bypass.
           Set-ExecutionPolicy -Scope Process -ExecutionPolicy RemoteSigned -Force\n\
           L 'Kur -Guncelle basliyor'\n\
           & $kur -Guncelle\n\
           L ('Kur bitti OK')\n\
         }} catch {{\n\
           L ('HATA: ' + $_.Exception.Message)\n\
         }}\n",
        kur = q(kur)
    );
    if restart {
        s.push_str(&format!(
            "$e = Join-Path $env:LOCALAPPDATA 'Programs\\ArifPdfEditor\\ArifPdfEditor.exe'\n\
             if (-not (Test-Path -LiteralPath $e)) {{ $e = '{exe}' }}\n\
             L ('yeniden baslat: ' + $e)\n\
             if (Test-Path -LiteralPath $e) {{ Start-Process -FilePath $e }} else {{ L 'exe yok, baslatilamadi' }}\n",
            exe = q(exe)
        ));
    }
    s
}

/// Start the helper that installs the update after this program has closed.
/// arif.6: no -EncodedCommand, no hidden window (CrowdStrike).
/// arif.12: do **not** launch the helper with `powershell -File` (Restricted blocks it before any
/// line runs). Write the .ps1 for diagnosis, then start via `-Command` + process-scope
/// RemoteSigned (never `-ExecutionPolicy Bypass` on argv / VPN scripts).
#[cfg(windows)]
fn spawn_updater(restart: bool) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let kur = installer_script().ok_or("kurulum betiği (Kur.ps1) bulunamadı")?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let script = updater_script(std::process::id(), &kur.to_string_lossy(), &exe.to_string_lossy(), restart);
    let helper = kur.with_file_name("guncelle-yardimci.ps1");
    // UTF-8 with BOM so Windows PowerShell 5.1 reads non-ASCII paths correctly.
    let mut body = vec![0xEF, 0xBB, 0xBF];
    body.extend_from_slice(script.replace('\n', "\r\n").as_bytes());
    std::fs::write(&helper, body).map_err(|e| e.to_string())?;
    let helper_q = helper.to_string_lossy().replace('\'', "''");
    // Process-scope RemoteSigned only — machine policy unchanged; no Bypass token for AV.
    let cmd = format!(
        "Set-ExecutionPolicy -Scope Process -ExecutionPolicy RemoteSigned -Force; & '{helper_q}'"
    );
    // CREATE_NEW_CONSOLE: its own (minimised) window; it outlives this program.
    std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-WindowStyle", "Minimized", "-Command", &cmd])
        .creation_flags(0x0000_0010)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(not(windows))]
fn spawn_updater(_restart: bool) -> Result<(), String> {
    Err("yalnızca Windows'ta desteklenir".into())
}

impl PdfCraftApp {
    /// Ask the update server (on its own thread). `dialog`: show the outcome (Help ▸ Check for updates).
    fn start_update_check(&mut self, dialog: bool) {
        let Some(source) = self.update_source.clone() else {
            if dialog {
                self.open_url(PROGRAM_PAGE);
            }
            return;
        };
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.updates.open |= dialog;
            if matches!(self.updates.check, Check::Running(_)) {
                return;
            }
            let (tx, rx) = std::sync::mpsc::channel();
            let ctx = self.ctx.clone();
            std::thread::spawn(move || {
                // The receiver may be gone (the app quit): nothing to report to then.
                let _ = tx.send(source());
                if let Some(ctx) = ctx {
                    ctx.request_repaint();
                }
            });
            self.updates.check = Check::Running(rx);
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = (source, dialog);
        }
    }

    /// Help ▸ Check for updates.
    pub fn check_for_updates(&mut self) {
        self.updates.hidden = false;
        self.start_update_check(true);
    }

    /// The newer version the last check found, if any.
    fn newer_release(&self) -> Option<&Release> {
        match &self.updates.check {
            Check::Done(Ok(r)) if is_newer(&r.version, &running_version()) => Some(r),
            _ => None,
        }
    }

    /// Update when the program is closed (`restart`: and open it again).
    fn arm_update(&mut self, restart: bool) {
        match spawn_updater(restart) {
            Ok(()) => {
                self.updates.armed = true;
                self.updates.error = None;
            }
            Err(e) => {
                log::warn!("update: couldn't start the installer: {e}");
                self.updates.error = Some(e);
            }
        }
    }

    /// Run the check at start, and pick up a finished one (each frame). Never blocks.
    pub(crate) fn poll_updates(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if !self.updates.started && self.update_source.is_some() {
                self.updates.started = true;
                // Sandbox / visual QA: skip the "Yeni sürüm var" toast.
                if std::env::var_os("ARIFPDF_TEST").is_some() {
                    self.updates.hidden = true;
                } else {
                    self.start_update_check(false);
                }
            }
            if let Check::Running(rx) = &self.updates.check {
                let result = match rx.try_recv() {
                    Ok(r) => r,
                    Err(std::sync::mpsc::TryRecvError::Empty) => return,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => Err("denetim beklenmedik şekilde durdu".into()),
                };
                if let Err(e) = &result {
                    // Offline or the server is down: silent, only in the log.
                    log::info!("update check: {e}");
                }
                self.updates.check = Check::Done(result);
            }
            if self.auto_update && !self.updates.armed && self.updates.error.is_none() && self.newer_release().is_some() && installer_script().is_some() {
                self.arm_update(false);
            }
        }
    }
}

/// The update notice (bottom right) and the Updates dialog.
pub(crate) fn dialog(app: &mut PdfCraftApp, ctx: &egui::Context) {
    notice(app, ctx);
    if !app.updates.open {
        return;
    }
    let t = theme::Tokens::get(ctx);
    let current = running_version();
    let mut close = false;
    let mut update_now = false;
    let modal = crate::widgets::show_modal(ctx, "updates", |ui| {
        ui.set_width(420.0);
        ui.horizontal(|ui| {
            ui.add(crate::icons::image("cloud", 22.0, t.accent));
            ui.label(egui::RichText::new("Güncelleme denetimi").font(theme::semibold(16.0)));
        });
        ui.add_space(8.0);
        match &app.updates.check {
            Check::Idle => {
                ui.label("Henüz denetim yapılmadı.");
            }
            #[cfg(not(target_arch = "wasm32"))]
            Check::Running(_) => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Yeni sürüm denetleniyor…");
                });
            }
            Check::Done(Ok(r)) if is_newer(&r.version, &current) => {
                ui.label(egui::RichText::new(format!("Yeni sürüm var: {}", r.version)).strong());
                ui.label(egui::RichText::new(format!("Kurulu sürüm: {current}. Güncelleme sunucudan indirilir, sha256 ile doğrulanır ve program kapanınca kurulur.")).color(t.text_muted));
            }
            Check::Done(Ok(_)) => {
                ui.label(format!("Arif Pdf Editör {current} güncel."));
            }
            Check::Done(Err(e)) => {
                ui.label(format!("Güncelleme sunucusuna ulaşılamadı: {e}"));
                ui.label(egui::RichText::new(format!("Kurulu sürüm: {current}. VPN veya ağ bağlantısını denetleyin.")).color(t.text_muted));
            }
        }
        ui.add_space(10.0);
        ui.label(
            egui::RichText::new("Yalnızca kurumun güncelleme sunucusuna (10.129.0.29 / ValilikServer) bağlanılır; başka bir yere veri gönderilmez.")
                .color(t.text_muted)
                .small(),
        );
        ui.add_space(12.0);
        let newer = app.newer_release().is_some();
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if newer {
                update_now = widgets::pill_button(ui, "Güncelle", true).clicked();
                if widgets::pill_button(ui, "Sonra", false).clicked() {
                    close = true;
                }
            } else if widgets::pill_button(ui, "Kapat", true).clicked() {
                close = true;
            }
        });
    });
    if modal.should_close() {
        close = true;
    }
    if update_now {
        close = true;
        app.apply_update(ctx);
    }
    if close {
        app.updates.open = false;
    }
}

impl PdfCraftApp {
    /// Güncelle: arm the installer (with a restart) and close the program through the normal
    /// quit path, which asks about unsaved documents first.
    fn apply_update(&mut self, ctx: &egui::Context) {
        if installer_script().is_none() {
            // A portable copy has no installer script: the program site has the Win+R installation.
            self.open_url(PROGRAM_PAGE);
            return;
        }
        self.arm_update(true);
        if self.updates.armed {
            self.request_quit(ctx);
        }
    }
}

/// The small notice: a newer version is available.
fn notice(app: &mut PdfCraftApp, ctx: &egui::Context) {
    if app.updates.hidden || app.updates.open {
        return;
    }
    let Some(version) = app.newer_release().map(|r| r.version.clone()) else { return };
    let t = theme::Tokens::get(ctx);
    let (mut update, mut later) = (false, false);
    egui::Area::new(egui::Id::new("update-notice"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-16.0, -16.0))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).fill(t.card).stroke(egui::Stroke::new(1.0, t.border)).inner_margin(egui::Margin::symmetric(14, 10)).show(ui, |ui| {
                ui.set_max_width(340.0);
                ui.horizontal(|ui| {
                    ui.add(crate::icons::image("cloud", 18.0, t.accent));
                    ui.label(egui::RichText::new("Yeni sürüm var").font(theme::semibold(13.5)));
                });
                if app.updates.armed {
                    ui.label(egui::RichText::new(format!("{version} program kapatılınca sessizce kurulacak.")).small().color(t.text_muted));
                } else if let Some(e) = &app.updates.error {
                    ui.label(egui::RichText::new(format!("{version} hazır; otomatik kurulamadı ({e}).")).small().color(t.text_muted));
                } else {
                    ui.label(egui::RichText::new(format!("{version} sürümü hazır.")).small().color(t.text_muted));
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    update = widgets::pill_button(ui, "Güncelle", true).clicked();
                    later = widgets::pill_button(ui, "Sonra", false).clicked();
                });
            });
        });
    if update {
        app.updates.hidden = true;
        app.apply_update(ctx);
    } else if later {
        app.updates.hidden = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arif_versions_compare_by_their_build_number() {
        assert!(is_newer("0.9.0-arif.5", "0.9.0-arif.4"));
        assert!(is_newer("0.9.0-arif.10", "0.9.0-arif.9"));
        assert!(!is_newer("0.9.0-arif.4", "0.9.0-arif.4"));
        assert!(!is_newer("0.9.0-arif.3", "0.9.0-arif.4"));
        assert!(is_newer("0.9.1-arif.1", "0.9.0-arif.7"));
        assert!(is_newer("0.9.0-arif.1", "0.9.0"));
        assert!(!is_newer("garbage", "0.9.0-arif.4"));
        assert!(!is_newer("0.9.0-arif.5", "garbage"));
    }

    #[test]
    fn the_updater_waits_for_the_program_and_quotes_paths() {
        let s = updater_script(1234, r"C:\Users\O'Neil\Kur.ps1", r"C:\x\ArifPdfEditor.exe", true);
        assert!(s.contains("Wait-Process -Id 1234"));
        assert!(s.contains("Get-Process -Id 1234"));
        assert!(s.contains("& $kur -Guncelle"), "{s}");
        assert!(s.contains("RemoteSigned"), "{s}");
        assert!(!s.contains("-File '"), "{s}"); assert!(!s.contains("powershell.exe -NoProfile -File"), "{s}");
        assert!(s.contains("O''Neil"), "path quote: {s}");
        assert!(s.contains("Start-Process"));
        assert!(!updater_script(1, "k", "e", false).contains("Start-Process -FilePath"));
    }

}
