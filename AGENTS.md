# AIBox — Developer & AI Agent Guidelines

## 🚨 ATURAN WAJIB: Selalu Update Sistem di Device Setiap Menambahkan Fitur

Setiap kali Anda (AI Agent) diminta untuk **menambahkan fitur baru**, **mengubah UI/UX**, **memodifikasi fungsionalitas**, atau **memperbaiki bug** pada AIBox:

1. **Jalankan Instalasi Standar**:
   ```bash
   ./bin/install
   ```
   Perintah ini membangun aplikasi dan paket Arch Linux, lalu menginstalnya (termasuk memperbarui `/usr/bin/aibox` dan launcher `Super + Space`). Jangan jalankan `makepkg` atau `./bin/install` kedua kalinya. Jika instalasi terblokir oleh autentikasi sistem, laporkan bahwa instalasi belum selesai dan sarankan pengguna menjalankan `./bin/install` secara lokal.
2. **Jalankan Unit Test**:
   ```bash
   ./bin/test
   ```
   Pastikan seluruh test suite lolos sebelum menyatakan pekerjaan selesai.
3. **Update Dokumentasi & Changelog**:
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
- **Dilarang memakai emoji atau ikon gear berwarna.** Gunakan ikon **satu warna (monochrome)** yang sederhana dan mengikuti aksen tema Omarchy (`theme.accent`), misalnya komponen QML `SlidersIcon` (garis + knob) atau SVG satu warna. Ikon harus tetap terbaca tanpa warna dan tidak memakai emoji.
- Mengikuti aksen tema aktif Omarchy secara dinamis (`theme.accent`).
- Mengikuti radius window Hyprland (`theme.radius`).
- Menggunakan font `JetBrainsMono Nerd Font` untuk teks teknis dan `Adwaita Sans` untuk UI umum.
- Menjaga metrik tombol seragam (`34px` height, `10px` gap).
- Seluruh keyboard shortcut terdaftar pada bantuan overlay (`?`).
