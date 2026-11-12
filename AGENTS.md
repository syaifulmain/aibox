# AIBox — Developer & AI Agent Guidelines

## 🚨 ATURAN WAJIB: Selalu Update Sistem di Device Setiap Menambahkan Fitur

Setiap kali Anda (AI Agent) diminta untuk **menambahkan fitur baru**, **mengubah UI/UX**, **memodifikasi fungsionalitas**, atau **memperbaiki bug** pada AIBox:

1. **Kompilasi & Build**:
   ```bash
   ./bin/build
   ```
2. **Jalankan Unit Test**:
   ```bash
   ./bin/test
   ```
   Pastikan seluruh test suite lolos (13/13 PASS) sebelum melanjutkan.
3. **Build Paket Arch Linux**:
   ```bash
   cd pkgbuild && makepkg -f
   ```
4. **Update Sistem di Device User**:
   Pastikan aplikasi yang terpasang di sistem device user (`/usr/bin/aibox` dan launcher shortcut `Super + Space`) ikut terbarui dengan versi terbaru:
   ```bash
   ./bin/install
   ```
   *(Catatan: Jika sesi non-interaktif membutuhkan konfirmasi sudo password dari user, informasikan bahwa paket terbaru telah selesai di-build di `pkgbuild/` dan berikan instruksi instalasi langsung).*
5. **Update Dokumentasi & Changelog**:
   Perbarui catatan perubahan di [CATALOG.md](file:///home/saep/Work/aibox/CATALOG.md) (bagian Changelog) dan [README.md](file:///home/saep/Work/aibox/README.md) jika ada penambahan fitur, perubahan shortcut, atau opsi baru.

---

## 🏛️ Arsitektur AIBox

- **Framework**: C++17 + Qt Quick 6 (QML) + `qmake6`
- **Isolasi OS**: Incus (system containers & VM), Btrfs quotas, KVM virtualization (`/dev/kvm`), Intel Iris Xe GPU passthrough (`/dev/dri`).
- **File Inti**:
  - `aibox.pro`: Konfigurasi build `qmake6`.
  - `src/main.cpp`: Entry point aplikasi (GUI + subperintah CLI `aibox list`, `aibox check`).
  - `src/backend.{h,cpp}`: Logika C++, parsing JSON Incus, stream progress PTY (`script -q -e -c`), background task detached session (`setsid`), dan pembatalan proses.
  - `src/theme.{h,cpp}`: Live watcher tema Omarchy (`colors.toml`) & window rounding Hyprland.
  - `src/translator.{h,cpp}`: Live multi-language loader (`~/.config/omarchy/language`).
  - `src/Main.qml`: Seluruh antarmuka grafis QML (FlatInput, ActionButton, selector card, keyboard shortcuts).
  - `tests/`: Offscreen Qt Test suite (`./bin/test`).
  - `pkgbuild/`: `PKGBUILD`, `aibox.desktop`, `aibox.svg` untuk manajemen paket native Arch Linux.

---

## 🎨 Konvensi Desain Omarchy
- Mengikuti aksen tema aktif Omarchy secara dinamis (`theme.accent`).
- Mengikuti radius window Hyprland (`theme.radius`).
- Menggunakan font `JetBrainsMono Nerd Font` untuk teks teknis dan `Adwaita Sans` untuk UI umum.
- Menjaga metrik tombol seragam (`34px` height, `10px` gap).
- Seluruh keyboard shortcut terdaftar pada bantuan overlay (`?`).
