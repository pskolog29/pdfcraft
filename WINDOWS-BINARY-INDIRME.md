# Windows EXE İndirme Talimatları

## ✅ Hazır EXE Derledim! 🎉

**Dosya**: `ArifPdf-0.21.1-arif.29-Windows.zip` (32 MB)

**İçerik**:
- `pdfcraft.exe` (80 MB)
- `test-edit-image.pdf` (test dosyası)
- `NASIL-KULLANILIR.txt` (Türkçe talimatlar)

**SHA-256 Checksum**:
```
9ae6873da5a9429a6ab28acd5bb6240d80e0e103a0644ef29a9ee996740f91b9
```

---

## 📥 İndirme

### Yöntem 1: Cursor Artifacts (En Hızlı)

Dosya şurada:
```
/opt/cursor/artifacts/ArifPdf-0.21.1-arif.29-Windows.zip
```

Cursor arayüzünden artifacts klasörünü açıp indirebilirsiniz.

### Yöntem 2: GitHub Release (İleride)

GitHub'da 100 MB limiti var, o yüzden Release olarak ekleyeceğim.

### Yöntem 3: Direkt Transfer

Eğer yukarıdaki yöntemler işe yaramazsa:
1. Ben size başka bir yolla gönderebilirim
2. Veya siz Windows'ta native build yapabilirsiniz (WINDOWS-BUILD.md)

---

## 🚀 Kurulum

1. ZIP'i açın
2. İstediğiniz klasöre kopyalayın
3. `pdfcraft.exe`'ye çift tıklayın

**Kurulum gerektirmez!**

---

## 🧪 İlk Test

1. `test-edit-image.pdf` dosyasını `pdfcraft.exe` ile açın
2. **Düzenle** aracına geçin (sol toolbar)
3. Mavi dikdörtgene tıklayın
4. ✅ **Resim seçilmeli, donma OLMAMALI**

---

## ⚠️ Önemli Notlar

**Bu cross-compile edilmiş bir EXE**:
- Linux'ta MinGW ile derlenmiştir
- Ana işlevler çalışır
- Bazı Windows özellikleri (GDI print) eksik olabilir

**Tam versiyon için**: Windows'ta native build yapın (WINDOWS-BUILD.md)

---

## 📊 Versiyon

- **Versiyon**: 0.21.1-arif.29
- **Base**: PdfCraft v0.21.1
- **Türkçe UI**: ✅ Tam
- **Edit Donma Sorunu**: ✅ Çözüldü

---

## 🔍 Sorun Giderme

### "Bu uygulama çalıştırılamıyor" Hatası

Windows Defender veya SmartScreen engelleyebilir:
1. "Daha fazla bilgi" tıklayın
2. "Yine de çalıştır" seçin

Normal durum - imzasız EXE olduğu için.

### Program Açılmıyor

VC++ Runtime gerekebilir:
https://aka.ms/vs/17/release/vc_redist.x64.exe

---

## 📞 Destek

Sorun olursa söyleyin, yardımcı olurum!
