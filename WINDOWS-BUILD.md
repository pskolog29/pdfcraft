# Windows'ta Arif PDF Editor 0.21.1-arif.29 Derleme

## Ön Gereksinimler

1. **Rust** (minimum 1.95.0 - edition 2024 desteği için)
   - İndirme: https://rustup.rs/
   - Kurulu ise güncelle: `rustup update stable`

2. **Git** (opsiyonel, kaynak indirmek için)
   - İndirme: https://git-scm.com/download/win

3. **Visual Studio Build Tools** (Rust Windows derlemeleri için)
   - Zaten kurulu olmalı (arif.27'yi derleyebildiyseniz)

---

## Adım 1: Kaynak Kodunu İndirin

### Yöntem A: Git ile (Önerilen)

```cmd
cd D:\ArifPdfDev
git clone --branch arif-29-rebase https://github.com/pskolog29/pdfcraft.git pdfcraft-arif29
cd pdfcraft-arif29
```

### Yöntem B: ZIP İndirme

1. https://github.com/pskolog29/pdfcraft/archive/refs/heads/arif-29-rebase.zip
2. `D:\ArifPdfDev\pdfcraft-arif29\` dizinine açın

### Yöntem C: Tarball (Repo'dan)

1. `pdfcraft-arif29-source.tar.gz` dosyasını indirin
2. 7-Zip ile açın: `D:\ArifPdfDev\pdfcraft-arif29\`

---

## Adım 2: Rust Versiyonunu Kontrol Edin

```cmd
rustc --version
cargo --version
```

**Gerekli**: `rustc 1.95.0` veya daha yeni

Güncelleme:
```cmd
rustup update stable
rustup default stable
```

---

## Adım 3: Derleme

### Debug Build (Hızlı derleme, test için):

```cmd
cd D:\ArifPdfDev\pdfcraft-arif29
cargo build
```

Çıktı: `target\debug\pdfcraft.exe`

### Release Build (Optimize, dağıtım için):

```cmd
cargo build --release
```

Çıktı: `target\release\pdfcraft.exe` (~25-30 MB)

**Derleme Süresi**: İlk derleme ~10-15 dakika (tüm bağımlılıklar)

---

## Adım 4: Test Edin

### A. Edit Modunda Resim Tıklama (Ana Test)

1. Test PDF'ini açın:
   ```cmd
   target\release\pdfcraft.exe test-edit-image.pdf
   ```

2. Veya kendi resimli PDF'inizi açın

3. **Düzenle** (Edit) aracına geçin (sol toolbar)

4. **Mavi dikdörtgen resme tıklayın**

5. ✅ **Beklenen Sonuç**: 
   - Resim seçilir (mavi kenarlık)
   - Program DONMAZ
   - Resmi taşıyabilirsiniz
   - Delete tuşuyla silebilirsiniz

6. ❌ **Eski arif.27 Davranışı**:
   - Tıkladıktan sonra program tamamen donar
   - Pencere yanıt vermez
   - Görev Yöneticisi'nden kapatmak gerekir

### B. Türkçe UI Kontrolü

- **Menüler**: Dosya, Düzenle, Sayfalar, Görünüm, Yardım
- **Araçlar**: Yorum ekle, Metni düzenle, vb.
- **Dialoglar**: Tercihler, Hakkında

Hepsi Türkçe olmalı (2.455 string çevrildi)

### C. Versiyon Kontrolü

**Yardım** → **Hakkında**  
Versiyon: **0.21.1-arif.29**

---

## Adım 5: EXE Paketleme (Dağıtım için)

```cmd
copy target\release\pdfcraft.exe D:\ArifPdfDev\ArifPdf-0.21.1-arif.29.exe
```

Veya installer oluştur (varsa eski build script'i):
```cmd
D:\ArifPdfDev\package-arif.bat pdfcraft-arif29
```

---

## Sorun Giderme

### "edition2024 not found" Hatası

**Neden**: Rust 1.83 veya daha eski

**Çözüm**:
```cmd
rustup update stable
rustup default stable
rustc --version
```
En az `1.95.0` görmeli

### "arif-gdi-print" Derleme Hatası (Windows Specific)

**Kontrol**:
```cmd
cargo check -p arif-gdi-print
```

Hata varsa, MSVC build tools kurulu olmalı:
- Visual Studio Installer → "Desktop development with C++"

### Uzun Derleme Süresi

İlk derleme normal (~15 dakika). Sonrakiler çok daha hızlı (~2-3 dakika).

**Hızlandırma** (opsiyonel):
```toml
# .cargo\config.toml ekleyin:
[build]
jobs = 8  # CPU core sayınıza göre
```

---

## Karşılaştırma: v0.21.1-arif.29 vs 0.9.0-arif.27

| Özellik | arif.27 | arif.29 (Bu) |
|---------|---------|--------------|
| Base | PdfCraft v0.9.0 | PdfCraft v0.21.1 |
| Edit Donma | ❌ Var | ✅ Yok |
| Türkçe UI | ✅ Var | ✅ Var (2455 string) |
| GDI Print | ✅ Var | ✅ Var (crate eklendi) |
| OCR | ✅ Var | ✅ Var |
| Edit Mimarisi | Eski monolitik | Yeni modular |

---

## Test Senaryoları

### 1. Minimal Test (5 dakika)

✅ Derle  
✅ `test-edit-image.pdf` aç  
✅ Düzenle moduna geç  
✅ Resme tıkla → donma yok  

### 2. Kapsamlı Test (30 dakika)

✅ Minimal test  
✅ Türkçe UI kontrol (tüm menüler)  
✅ Farklı PDF'ler aç (text, image, karışık)  
✅ Düzenle modunda text düzenleme  
✅ Yorum ekleme  
✅ Yazdırma (GDI print test)  
✅ OCR test (varsa)  

### 3. Regresyon Test (eski arif.27 ile)

✅ aynı PDF'i arif.27 ve arif.29'da aç  
✅ Düzenle → resim tıkla  
✅ arif.27: donar ❌  
✅ arif.29: çalışır ✅  

---

## Hata Raporu

Sorun olursa:

1. **Hata Mesajı**: Tam metni kaydedin
2. **Repro**: Nasıl oluştuğunu yazın
3. **PDF**: Test PDF'i ekleyin (mümkünse)
4. **Versiyon**: `cargo --version`, `pdfcraft.exe --version`

GitHub Issue: https://github.com/pskolog29/pdfcraft/issues

---

## Sonraki Adımlar (Gelecek Versiyonlar için)

### Eksik Özellikler:

1. **Print Workflow Entegrasyonu**
   - `arif-gdi-print` v0.21.1 print mimarisine bağlanmalı
   - Şu an crate var ama workflow'da kullanılmıyor

2. **Branding**
   - Pencere başlığı: "PdfCraft" → "Arif Pdf Editör"
   - Uygulama ikonu güncelleme
   - Hakkında dialogu

3. **Installer**
   - InnoSetup / WiX script güncelleme
   - Versiyon: 0.21.1-arif.29

---

## Yardım

**GitHub**: https://github.com/pskolog29/pdfcraft/tree/arif-29-rebase  
**Commit**: `761f8be83`  
**Tarih**: 2026-11-23
