//! Arif Pdf Editör: asks the institution's update server which version is newest.
//!
//! The answer is `ArifPdfEditor.json` (`{"ad","surum","sha256","boyut","url","tarih","kaynak"}`),
//! the same file the installed `Kur.ps1 -Guncelle` reads. The sources are tried in order: the VPN
//! address (a short wait, so being away from it costs little), the public site, the internal
//! mirror. Only these three are ever contacted; offline simply returns an error, which the app
//! keeps to its log.

use std::time::Duration;

use pdfcraft_ui_egui::updates::Release;

/// (the json's address, how long to wait for it).
const SOURCES: [(&str, u64); 3] = [
    ("http://10.129.0.29/program/ArifPdfEditor.json", 3),
    ("https://www.burasigumushane.com/Program/ArifPdfEditor.json", 8),
    ("http://192.168.2.200/program/ArifPdfEditor.json", 4),
];

/// The newest release from the first source that answers with a usable file.
pub fn latest_release() -> Result<Release, String> {
    let mut last = String::from("güncelleme sunucusu yok");
    for (url, secs) in SOURCES {
        match fetch(url, secs) {
            Ok(r) => return Ok(r),
            Err(e) => last = e,
        }
    }
    Err(last)
}

fn fetch(url: &str, secs: u64) -> Result<Release, String> {
    let host = url.split('/').nth(2).unwrap_or(url);
    let mut config = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(secs)));
    if url.starts_with("https:") {
        config = config.tls_config(ureq::tls::TlsConfig::builder().root_certs(os_roots()?).build());
    }
    let agent = config.build().new_agent();
    let mut response = agent
        .get(url)
        .header("User-Agent", concat!("ArifPdfEditor/", env!("CARGO_PKG_VERSION")))
        .header("Cache-Control", "no-cache")
        .call()
        .map_err(|e| format!("{host}: {e}"))?;
    // The answer is untrusted: its size is capped.
    let body = response.body_mut().with_config().limit(64 << 10).read_to_string().map_err(|e| format!("{host}: okunamadı ({e})"))?;
    parse(&body, url).map_err(|e| format!("{host}: {e}"))
}

/// The certificate authorities the operating system trusts.
fn os_roots() -> Result<ureq::tls::RootCerts, String> {
    let found = rustls_native_certs::load_native_certs();
    let certs: Vec<ureq::tls::Certificate<'static>> = found.certs.iter().map(|c| ureq::tls::Certificate::from_der(c.as_ref()).to_owned()).collect();
    if certs.is_empty() {
        return Err("sistemde güvenilen sertifika yok".into());
    }
    Ok(ureq::tls::RootCerts::new_with_certs(&certs))
}

/// `ArifPdfEditor.json` → a release. `from` is the address it was read from.
fn parse(body: &str, from: &str) -> Result<Release, String> {
    let v: serde_json::Value = serde_json::from_str(body).map_err(|_| "geçersiz sürüm dosyası".to_string())?;
    let version = v["surum"].as_str().map(str::trim).unwrap_or_default();
    // A version is short and plain: digits, letters, dots and dashes.
    if version.is_empty() || version.len() > 40 || !version.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+')) {
        return Err("sürüm bilgisi okunamadı".into());
    }
    let sha256 = v["sha256"].as_str().unwrap_or_default().trim().to_ascii_uppercase();
    if sha256.len() != 64 || !sha256.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("sürüm dosyasında geçerli sha256 yok".into());
    }
    let page = from.rsplit_once('/').map_or(from, |(dir, _)| dir).to_string() + "/";
    Ok(Release { version: version.to_string(), url: page, sha256, size: v["boyut"].as_u64().unwrap_or(0) })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "27DF9EFFD73BB6343046ED8B3199A3207FB9CF854C6FA99415CB26D95DDCDD83";

    #[test]
    fn the_program_sites_json_is_read() {
        let body = format!(
            r#"{{"ad":"Arif Pdf Editör","surum":"0.9.0-arif.4","sha256":"{SHA}","boyut":84137138,"url":"/program/downloads/ArifPdfEditor.zip","tarih":"2026-10-25","kaynak":"x"}}"#
        );
        let r = parse(&body, "http://10.129.0.29/program/ArifPdfEditor.json").unwrap();
        assert_eq!(r.version, "0.9.0-arif.4");
        assert_eq!(r.sha256, SHA);
        assert_eq!(r.size, 84_137_138);
        assert_eq!(r.url, "http://10.129.0.29/program/");
    }

    #[test]
    fn a_bad_file_is_refused() {
        assert!(parse("not json", "http://x/p/a.json").is_err());
        assert!(parse(&format!(r#"{{"surum":"1 2","sha256":"{SHA}"}}"#), "http://x/p/a.json").is_err());
        assert!(parse(r#"{"surum":"0.9.0-arif.5","sha256":"abc"}"#, "http://x/p/a.json").is_err());
        assert!(parse(&format!(r#"{{"sha256":"{SHA}"}}"#), "http://x/p/a.json").is_err());
    }

    #[test]
    fn only_the_institutions_servers_are_listed() {
        for (url, _) in SOURCES {
            assert!(url.contains("10.129.0.29") || url.contains("burasigumushane.com") || url.contains("192.168.2.200"), "{url}");
        }
        assert_eq!(SOURCES[0].1, 3);
    }
}
