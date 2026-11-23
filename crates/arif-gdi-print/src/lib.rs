//! Arif Pdf Editör: GDI print fallback (Windows only; empty elsewhere).
#[cfg(windows)]
mod gdi;
#[cfg(windows)]
pub use gdi::submit;
