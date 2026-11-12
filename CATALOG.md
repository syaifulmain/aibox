# CATALOG.md — Katalog Arsitektur, File, & Riwayat Perubahan AIBox

Dokumen ini mencatat seluruh perubahan sistem, arsitektur komponen, daftar file projek, serta riwayat pembaruan (**Changelog**) dari **AIBox** agar mudah dilacak, dipelihara, atau dipasang ulang di masa mendatang.

---

## 1. Katalog Perubahan pada Sistem Utama (Host OS — Omarchy)

Seluruh perubahan pada sistem utama diotomatisasi di dalam skrip [bin/setup-system](file:///home/saep/Work/aibox/bin/setup-system) dan dapat dibersihkan kembali menggunakan [bin/uninstall](file:///home/saep/Work/aibox/bin/uninstall).

| Komponen Sistem | Lokasi / Nama | Fungsi & Tujuan |
| :--- | :--- | :--- |
| **Paket Sistem** | `incus`, `dnsmasq`, `xorg-xhost`, `acl`, `qemu-system-x86`, `edk2-ovmf` | Mesin *system container* & *virtual machine*, DHCP/DNS lokal, jembatan layar X11/Wayland, dan manajemen izin ACL. |
| **Subordinate UID/GID** | `/etc/subuid` & `/etc/subgid` (`root:1000000:1000000000`) | Memetakan akun `root` di dalam container menjadi user biasa (*unprivileged*) di Host agar AI tidak dapat merusak OS utama. |
| **Grup User** | `incus-admin`, `kvm`, `render`, `video` (untuk user `saep`) | Memberikan akses penuh ke Incus, akselerasi KVM (`/dev/kvm`), dan GPU Intel Iris Xe (`/dev/dri`) tanpa `sudo`. |
| **Systemd Drop-in ACL** | `/etc/systemd/system/incus.service.d/acl.conf` | Memberikan izin baca/tulis pada `/var/lib/incus/unix.socket` secara otomatis saat service menyala. |
| **Btrfs Storage Pool** | `default` (`/var/lib/incus/storage-pools/default`) | Penyimpanan container di atas *subvolume* Btrfs dengan fitur **Btrfs Quota** (batas ukuran disk akurat) & *snapshot* instan (< 1 detik). |
| **Jaringan Bridge** | `incusbr0` (`10.143.44.1/24`, NAT aktif, IPv6 nonaktif) | Jaringan privat virtual yang memberikan alamat IPv4 dan koneksi internet ke setiap environment. |
| **Aturan Firewall UFW** | `ufw allow in on incusbr0` & `ufw route allow in/out on incusbr0` | Mengizinkan lalu lintas DHCP, DNS, dan internet dari container melewati firewall UFW Host. |
| **Profil Incus GPU/KVM** | Profil `ai-gui` (`security.nesting=true`, device `gpu` & `kvm`) | Mengaktifkan Docker-in-Docker, akselerasi GPU Desktop App, dan akselerasi hardware Android Emulator di dalam container. |
| **Folder Kerja Bersama** | `/home/saep/ai-workspaces/<nama>` ⟷ `/workspace` | Folder *shared* dua arah antara Host dan container dengan *default ACL* (`setfacl`) & *UID shifting* (`shift=true`). |
| **State & Log Background** | `/home/saep/.cache/aibox/` (`task.pid`, `task.meta`, `task.log`, `task.sh`) | Menyimpan status proses pembuatan/pengunduhan yang berjalan di latar belakang (`setsid`) agar tetap berjalan meskipun jendela GUI ditutup. |

---

## 2. Katalog File Projek (`~/Work/aibox`)

Projek ini dibangun mengikuti standar aplikasi native **Omarchy** (C++17 + Qt Quick 6 + `qmake6` + `makepkg`).

| File / Path | Kategori | Deskripsi |
| :--- | :--- | :--- |
| [aibox.pro](file:///home/saep/Work/aibox/aibox.pro) | Build Config | Konfigurasi proyek `qmake6` untuk mengompilasi binary utama `aibox`. |
| [src/main.cpp](file:///home/saep/Work/aibox/src/main.cpp) | Source (C++) | Entry point aplikasi GUI (`aibox`) sekaligus mode CLI tanpa layar (`aibox list`, `aibox check`). |
| [src/backend.h](file:///home/saep/Work/aibox/src/backend.h) | Source (C++) | Deklarasi class `Backend` (`QObject`), properti QML, sinyal progress, dan *invokable methods*. |
| [src/backend.cpp](file:///home/saep/Work/aibox/src/backend.cpp) | Source (C++) | Implementasi manajemen Incus, katalog OS, *real-time progress parser* (`script` PTY), pembatalan proses (`cancelTask`), dan *background task re-attachment*. |
| [src/theme.h](file:///home/saep/Work/aibox/src/theme.h) & [src/theme.cpp](file:///home/saep/Work/aibox/src/theme.cpp) | Source (C++) | Pemantau tema aktif Omarchy (`~/.local/state/omarchy/current/theme/colors.toml`) dan sudut jendela (`decoration:rounding`) Hyprland secara *live*. |
| [src/translator.h](file:///home/saep/Work/aibox/src/translator.h) & [src/translator.cpp](file:///home/saep/Work/aibox/src/translator.cpp) | Source (C++) | Sistem terjemahan dinamis yang memantau `~/.config/omarchy/language` secara *real-time*. |
| [src/Main.qml](file:///home/saep/Work/aibox/src/Main.qml) | Source (QML) | Antarmuka pengguna utama, komponen `FlatInput`, `ActionButton`, panel *inspector*, katalog OS, modal pembuatan, *progress bar*, dan pintasan keyboard. |
| [src/resources.qrc](file:///home/saep/Work/aibox/src/resources.qrc) | Resource | Mengemas `Main.qml` dan file bahasa ke dalam satu file binary `/usr/bin/aibox`. |
| [src/i18n/id.json](file:///home/saep/Work/aibox/src/i18n/id.json), [en.json](file:///home/saep/Work/aibox/src/i18n/en.json), [da.json](file:///home/saep/Work/aibox/src/i18n/da.json) | Lokalisasi | Katalog terjemahan bahasa Indonesia, Inggris, dan Denmark. |
| [tests/aibox_tests.pro](file:///home/saep/Work/aibox/tests/aibox_tests.pro) & [aibox_tests.cpp](file:///home/saep/Work/aibox/tests/aibox_tests.cpp) | Unit Tests | 13 pengujian otomatis Qt Test untuk parser JSON Incus, sanitasi nama, katalog OS, *state background task*, *Theme*, dan *Translator*. |
| [pkgbuild/PKGBUILD](file:///home/saep/Work/aibox/pkgbuild/PKGBUILD) | Packaging | Skrip paket resmi Arch Linux (`pacman`) untuk memasang `/usr/bin/aibox`, ikon, lisensi, dan `.desktop`. |
| [pkgbuild/aibox.desktop](file:///home/saep/Work/aibox/pkgbuild/aibox.desktop) | Launcher | Entri shortcut aplikasi untuk Omarchy App Launcher (`Super + Space`). |
| [pkgbuild/aibox.svg](file:///home/saep/Work/aibox/pkgbuild/aibox.svg) | Asset | Ikon vektor aplikasi AIBox. |
| [bin/setup-system](file:///home/saep/Work/aibox/bin/setup-system) | Script | Skrip otomatisasi instalasi & konfigurasi awal Incus, Btrfs, UFW, dan profil GPU/KVM. |
| [bin/build](file:///home/saep/Work/aibox/bin/build) | Script | Mengompilasi kode sumber menjadi binary `build/aibox`. |
| [bin/test](file:///home/saep/Work/aibox/bin/test) | Script | Mengompilasi dan menjalankan seluruh *unit test* dalam mode *offscreen*. |
| [bin/install](file:///home/saep/Work/aibox/bin/install) | Script | Membangun dan menginstal paket Arch Linux via `makepkg -fsi`. |
| [bin/uninstall](file:///home/saep/Work/aibox/bin/uninstall) | Script | Menghapus bersih paket `aibox` (serta opsi `--full` untuk menghapus Incus & seluruh container). |

---

## 3. Riwayat Perubahan (Changelog)

### Iterasi 3 — Penyempurnaan UI Profesional, Live Progress, Pembatalan, & Background Persistence (Commit `4500970`)
* **Desain Ulang Input (`FlatInput`)**: Menghapus `Material.TextField` (yang memiliki *floating label* berlekuk) dan menggantinya dengan komponen input datar minimalis ber-border `1px`.
* **Perbaikan Tipografi**: Menghapus penggunaan `font.bold` berlebihan; menerapkan hierarki font **`Adwaita Sans`** (`Normal` & `Medium`) untuk antarmuka serta **`JetBrainsMono Nerd Font`** untuk informasi teknis (kuota disk, RAM, IP, nama container, dan *shortcut*).
* **Desain Selector & Tombol Baru**: Mengganti kotak pilihan berwarna blok mencolok menjadi kartu permukaan gelap bernuansa aksen (`#192136`) dengan indikator titik (*dot*) dan tata letak dua baris agar teks tidak pernah terpotong.
* **Real-Time Progress & Log Streaming**: Membungkus perintah `incus image copy` dan `incus launch` dengan PTY (`script -q -e -c`) sehingga persentase unduhan (`0%–100%`), tahapan (*Step 1/4* s.d. *Step 4/4*), dan baris log terminal tampil secara *real-time*.
* **Fitur Pembatalan (`Cancel`) & Kunci Pembuatan Ganda**:
  * Menambahkan tombol **Cancel** yang menghentikan seluruh grup proses (`kill -TERM/-KILL`) dan membersihkan container/folder setengah jadi.
  * Mengunci seluruh tombol pembuatan environment baru (`disabled: backend.busy`) selama ada proses yang sedang berjalan.
* **Detached Background Task & Re-attach Otomatis**:
  * Menjalankan tugas pembuatan/pengunduhan di sesi latar belakang mandiri (`setsid`) dengan pencatatan status di `~/.cache/aibox/`.
  * Jika jendela GUI ditutup saat proses pembuatan berlangsung, proses tetap berjalan aman hingga selesai dan mengirimkan notifikasi desktop (`notify-send`).
  * Saat jendela GUI dibuka kembali, aplikasi otomatis menyambung kembali (*re-attach*) ke *progress bar* yang sedang berjalan atau menampilkan laporan hasil akhirnya.

### Iterasi 2 — Pembuatan Aplikasi GUI Native Omarchy `AIBox` (Commit `80e4505`)
* Membangun aplikasi desktop native C++17 & Qt Quick 6 di `~/Work/aibox`.
* Menyediakan daftar environment aktif, tombol masuk terminal (`Space`), buka *shared folder* (`F`), *snapshot* Btrfs instan (`P`), *headless GUI runner* (`Xvfb`), dan *wipe* (`Delete`).
* Menyediakan katalog 8 distro Linux populer dengan fitur ketuk untuk *download* ke *cache* lokal.
* Mengemas aplikasi dengan `PKGBUILD` dan memasangnya ke App Launcher Omarchy (`Super + Space`).

### Iterasi 1 — Konfigurasi Infrastruktur Dasar Incus & CLI `ai-box`
* Menginstal dan mengonfigurasi Incus di atas Btrfs dengan kuota disk aktif.
* Mengonfigurasi jembatan jaringan `incusbr0` dan aturan firewall UFW.
* Membuat profil `ai-gui` untuk *passthrough* GPU Intel Iris Xe (`/dev/dri`) dan KVM (`/dev/kvm`) bagi kebutuhan Desktop App & Android Emulator.
* Menyediakan skrip CLI interaktif di `~/.local/bin/ai-box`.
