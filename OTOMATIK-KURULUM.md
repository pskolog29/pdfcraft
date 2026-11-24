# 🚀 Arif PDF Editor - Otomatik Kurulum

## ✨ Tek Tıkla Kurulum!

Karışık komutlar yok, her şey otomatik!

---

## 📥 Adım 1: Script'i İndirin

### Yöntem A: GitHub'dan (Önerilen)

1. Şu linke gidin:  
   **https://github.com/pskolog29/pdfcraft/blob/arif-29-rebase/arif-pdf-kur.bat**

2. Sağ üstte **"Download raw file"** (↓) butonuna tıklayın

3. `arif-pdf-kur.bat` dosyası inecek

### Yöntem B: Direkt Link

**Direkt indirme linki**:
```
https://raw.githubusercontent.com/pskolog29/pdfcraft/arif-29-rebase/arif-pdf-kur.bat
```

Tarayıcıda açın, **Sağ tık** → **Farklı Kaydet** → `arif-pdf-kur.bat`

---

## 🎯 Adım 2: Çalıştırın

1. İndirilen `arif-pdf-kur.bat` dosyasına **ÇİFT TIKLAYIN**

2. Script otomatik olarak:
   - ✅ Rust kontrolü yapar (yoksa kurar)
   - ✅ Kaynak kodu indirir
   - ✅ EXE'yi derler (10-15 dakika)
   - ✅ Masaüstüne kısayol koyar
   - ✅ Test dosyası ile hazır eder

3. **Bitince**: Masaüstünde "Arif PDF Editor" kısayolu hazır!

---

## ⏱️ Ne Kadar Sürer?

- **İlk derleme**: 15-20 dakika
- **Tekrar derlemeler**: 2-3 dakika (çok daha hızlı)

**CPU %100 kullanacak** - Normal! Bekleyin, kapanmasın.

---

## 🧪 İlk Test

Script bitince:

1. Masaüstündeki **"Arif PDF Editor"** kısayoluna tıklayın
2. `test-edit-image.pdf` dosyasını açın (kurulum klasöründe)
3. **Düzenle** moduna geçin
4. Mavi dikdörtgene tıklayın
5. ✅ **Resim seçilmeli, DONMA OLMAMALI**

---

## 📂 Kurulum Yeri

```
C:\Users\ArifOfis\ArifPdfEditor\ArifPdf\
```

İçinde:
- `pdfcraft.exe` (Ana program)
- `test-edit-image.pdf` (Test dosyası)
- `NASIL-KULLANILIR.txt` (Talimatlar)

---

## 🔧 Script Ne Yapıyor?

1. **Rust Kontrolü**:
   - Rust varsa atlar
   - Yoksa otomatik kurar

2. **Kaynak İndirme**:
   - Git varsa: `git clone`
   - Git yoksa: ZIP indirir

3. **Derleme**:
   - `cargo build --release`
   - Native Windows EXE oluşturur

4. **Hazırlık**:
   - EXE'yi kopyalar
   - Test dosyası ekler
   - Masaüstü kısayolu oluşturur

---

## ⚠️ Olası Sorunlar

### "Bu uygulama çalıştırılamıyor"

Windows Defender veya SmartScreen:
1. **"Daha fazla bilgi"** tıklayın
2. **"Yine de çalıştır"** seçin

Script güvenli, .bat dosyası sadece Rust kurup derleme yapıyor.

### Internet bağlantısı gerekli

Script şunları indirir:
- Rust installer (~20 MB)
- Kaynak kodu (~19 MB)
- Cargo bağımlılıkları (~500 MB)

**Toplam**: ~550 MB indirme

### Derleme çok uzun sürüyor

Normal! İlk derleme 15-20 dakika sürer.
CPU %100 kullanır - bekleyin, iptal etmeyin.

---

## 🎊 Avantajları

✅ **Karışık komut yok** - Sadece çift tıkla  
✅ **Native Windows EXE** - Tam donanımlı  
✅ **Otomatik güncelleme** - Script'i tekrar çalıştırın  
✅ **GDI Print dahil** - Tüm özellikler çalışır  
✅ **Masaüstü kısayolu** - Hemen kullanıma hazır  

---

## 📊 Sonuç

**Versiyon**: 0.21.1-arif.29  
**Base**: PdfCraft v0.21.1  
**Türkçe UI**: ✅ Tam  
**Edit Donma**: ✅ Çözüldü  
**Native Build**: ✅ Evet  

---

## 🆘 Yardım

Script'te sorun olursa:
1. Hata mesajının **son 20 satırını** kaydedin
2. Bana gönderin
3. Birlikte bakalım

---

## 🔗 Linkler

**Script İndirme**:  
https://raw.githubusercontent.com/pskolog29/pdfcraft/arif-29-rebase/arif-pdf-kur.bat

**GitHub Repo**:  
https://github.com/pskolog29/pdfcraft/tree/arif-29-rebase

**Manuel Kurulum** (isterseniz):  
Repo'daki `WINDOWS-BUILD.md` dosyasına bakın
