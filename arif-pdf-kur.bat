@echo off
chcp 65001 >nul
setlocal EnableDelayedExpansion

:: ============================================================
::  Arif PDF Editor 0.21.1-arif.29 - Otomatik Kurulum
:: ============================================================

color 0A
title Arif PDF Editor - Otomatik Kurulum

echo.
echo ═══════════════════════════════════════════════════════════
echo   Arif PDF Editor 0.21.1-arif.29
echo   Otomatik Kurulum Scripti
echo ═══════════════════════════════════════════════════════════
echo.
echo Bu script:
echo   1. Rust'ı kontrol eder / kurar
echo   2. Kaynak kodu indirir
echo   3. EXE'yi derler
echo   4. Test dosyası ile hazır eder
echo.
echo Toplam süre: 15-20 dakika
echo.
pause

:: ============================================================
:: Adım 1: Rust Kontrolü
:: ============================================================

echo.
echo [1/5] Rust kontrolü yapılıyor...
echo.

cargo --version >nul 2>&1
if %ERRORLEVEL% EQU 0 (
    echo ✓ Rust zaten kurulu!
    cargo --version
    goto :SKIP_RUST_INSTALL
)

echo.
echo ✗ Rust kurulu değil. Şimdi kurulacak...
echo.

:: Rust indirme linki
set RUSTUP_URL=https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe

echo Rust installer indiriliyor...
powershell -Command "& {Invoke-WebRequest -Uri '%RUSTUP_URL%' -OutFile '%TEMP%\rustup-init.exe'}"

if not exist "%TEMP%\rustup-init.exe" (
    echo.
    echo ✗ HATA: Rust installer indirilemedi!
    echo.
    echo Manuel indirme:
    echo 1. https://rustup.rs/ adresine gidin
    echo 2. rustup-init.exe indirin
    echo 3. Bu scripti tekrar çalıştırın
    echo.
    pause
    exit /b 1
)

echo.
echo Rust kuruluyor... (2-3 dakika)
echo.
echo Not: Kurulum sırasında:
echo   1 yazıp ENTER basın (default installation)
echo.

"%TEMP%\rustup-init.exe" -y --default-toolchain stable

if %ERRORLEVEL% NEQ 0 (
    echo.
    echo ✗ HATA: Rust kurulumu başarısız!
    echo.
    pause
    exit /b 1
)

:: PATH'i güncelle
call "%USERPROFILE%\.cargo\env.bat" 2>nul

echo.
echo ✓ Rust başarıyla kuruldu!
echo.

:SKIP_RUST_INSTALL

:: ============================================================
:: Adım 2: Kaynak Kodu İndirme
:: ============================================================

echo.
echo [2/5] Kaynak kodu indiriliyor...
echo.

set INSTALL_DIR=%USERPROFILE%\ArifPdfEditor
set SOURCE_DIR=%INSTALL_DIR%\pdfcraft-arif29

:: Eski kurulum varsa sor
if exist "%SOURCE_DIR%" (
    echo.
    echo Eski kurulum bulundu: %SOURCE_DIR%
    echo.
    set /p REMOVE_OLD="Silinsin mi? (E/H): "
    if /i "!REMOVE_OLD!"=="E" (
        echo Eski kurulum siliniyor...
        rd /s /q "%SOURCE_DIR%" 2>nul
    ) else (
        echo Mevcut kurulum kullanılacak.
        goto :SKIP_DOWNLOAD
    )
)

mkdir "%INSTALL_DIR%" 2>nul
cd /d "%INSTALL_DIR%"

:: Git var mı kontrol et
git --version >nul 2>&1
if %ERRORLEVEL% EQU 0 (
    echo Git ile indiriliyor...
    git clone --depth 1 --branch arif-29-rebase https://github.com/pskolog29/pdfcraft.git pdfcraft-arif29
    if %ERRORLEVEL% NEQ 0 (
        echo ✗ Git clone başarısız, ZIP indirme deneniyor...
        goto :DOWNLOAD_ZIP
    )
    goto :SKIP_DOWNLOAD
)

:DOWNLOAD_ZIP
echo Git bulunamadı, ZIP olarak indiriliyor...
echo.

set ZIP_URL=https://github.com/pskolog29/pdfcraft/archive/refs/heads/arif-29-rebase.zip
set ZIP_FILE=%TEMP%\pdfcraft-arif29.zip

powershell -Command "& {Invoke-WebRequest -Uri '%ZIP_URL%' -OutFile '%ZIP_FILE%'}"

if not exist "%ZIP_FILE%" (
    echo ✗ HATA: Kaynak kodu indirilemedi!
    pause
    exit /b 1
)

echo ZIP açılıyor...
powershell -Command "& {Expand-Archive -Path '%ZIP_FILE%' -DestinationPath '%INSTALL_DIR%' -Force}"
ren "%INSTALL_DIR%\pdfcraft-arif-29-rebase" "pdfcraft-arif29"

del "%ZIP_FILE%" 2>nul

:SKIP_DOWNLOAD

if not exist "%SOURCE_DIR%\Cargo.toml" (
    echo ✗ HATA: Kaynak dosyaları bulunamadı!
    pause
    exit /b 1
)

echo ✓ Kaynak kodu hazır: %SOURCE_DIR%
echo.

:: ============================================================
:: Adım 3: Derleme
:: ============================================================

echo.
echo [3/5] EXE derleniyor...
echo.
echo Bu 10-15 dakika sürecek, lütfen bekleyin...
echo CPU kullanımı yüksek olacak (normal).
echo.

cd /d "%SOURCE_DIR%"

cargo build --release

if %ERRORLEVEL% NEQ 0 (
    echo.
    echo ✗ HATA: Derleme başarısız!
    echo.
    echo Hata logunu kontrol edin yukarıda.
    echo.
    pause
    exit /b 1
)

if not exist "target\release\pdfcraft.exe" (
    echo ✗ HATA: EXE oluşturulamadı!
    pause
    exit /b 1
)

echo.
echo ✓ Derleme başarılı!
echo.

:: ============================================================
:: Adım 4: Kurulum Klasörü Hazırlama
:: ============================================================

echo.
echo [4/5] Kurulum klasörü hazırlanıyor...
echo.

set APP_DIR=%INSTALL_DIR%\ArifPdf
mkdir "%APP_DIR%" 2>nul

copy /y "target\release\pdfcraft.exe" "%APP_DIR%\" >nul
copy /y "test-edit-image.pdf" "%APP_DIR%\" 2>nul

:: Kullanım talimatları
(
echo ═══════════════════════════════════════════════════════════
echo   Arif PDF Editor 0.21.1-arif.29
echo ═══════════════════════════════════════════════════════════
echo.
echo KULLANIM:
echo   pdfcraft.exe dosyasına çift tıklayın.
echo.
echo İLK TEST:
echo   1. test-edit-image.pdf dosyasını pdfcraft.exe ile açın
echo   2. Sol tarafta "Düzenle" aracına tıklayın
echo   3. Mavi dikdörtgene tıklayın
echo   4. ✓ Resim seçilmeli, donma OLMAMALI
echo.
echo VERSİYON:
echo   Yardım → Hakkında → 0.21.1-arif.29
echo.
echo DESTEK:
echo   GitHub: https://github.com/pskolog29/pdfcraft
echo.
) > "%APP_DIR%\NASIL-KULLANILIR.txt"

echo ✓ Dosyalar kopyalandı: %APP_DIR%
echo.

:: ============================================================
:: Adım 5: Masaüstü Kısayolu
:: ============================================================

echo.
echo [5/5] Masaüstü kısayolu oluşturuluyor...
echo.

set DESKTOP=%USERPROFILE%\Desktop
set SHORTCUT=%DESKTOP%\Arif PDF Editor.lnk

powershell -Command "$WS = New-Object -ComObject WScript.Shell; $SC = $WS.CreateShortcut('%SHORTCUT%'); $SC.TargetPath = '%APP_DIR%\pdfcraft.exe'; $SC.WorkingDirectory = '%APP_DIR%'; $SC.Description = 'Arif PDF Editor 0.21.1-arif.29'; $SC.Save()"

if exist "%SHORTCUT%" (
    echo ✓ Kısayol oluşturuldu: Masaüstü\Arif PDF Editor
) else (
    echo ✗ Kısayol oluşturulamadı (opsiyonel)
)

echo.

:: ============================================================
:: Tamamlandı
:: ============================================================

color 0B
cls
echo.
echo ═══════════════════════════════════════════════════════════
echo   ✓ KURULUM TAMAMLANDI!
echo ═══════════════════════════════════════════════════════════
echo.
echo Program kuruldu: %APP_DIR%
echo.
echo NASIL ÇALIŞTIRIRIM?
echo   1. Masaüstündeki "Arif PDF Editor" kısayoluna tıklayın
echo   2. VEYA: %APP_DIR%\pdfcraft.exe
echo.
echo İLK TEST:
echo   test-edit-image.pdf ile test edin (aynı klasörde)
echo   • Düzenle moduna geçin
echo   • Mavi dikdörtgene tıklayın
echo   • ✓ Donma OLMAMALI
echo.
echo VERSİYON: 0.21.1-arif.29
echo BASE: PdfCraft v0.21.1
echo TARİH: 2026-11-24
echo.
echo ═══════════════════════════════════════════════════════════
echo.

set /p OPEN_NOW="Şimdi açmak ister misiniz? (E/H): "
if /i "%OPEN_NOW%"=="E" (
    start "" "%APP_DIR%\pdfcraft.exe"
)

echo.
echo Keyifli kullanımlar!
echo.
pause
