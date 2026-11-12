# AIBox

![AIBox Screenshot](screenshot.png)

**AIBox** adalah aplikasi desktop native untuk **Omarchy** (C++17 & Qt Quick 6) yang mengelola environment OS Linux terisolasi (berbasis **Incus**, **Btrfs**, **KVM**, dan **GPU Passthrough**) khusus untuk pengembangan projek AI, Desktop App, dan Mobile App skala besar tanpa mengotori atau merusak sistem operasi utama Anda.

---

## Fitur Utama

- **Tampilan Native Omarchy (Live Theme & Language)**: Mengikuti warna aksen (`~/.local/state/omarchy/current/theme/colors.toml`), *window rounding* Hyprland, dan bahasa Omarchy secara *real-time*.
- **Manajemen Environment OS Penuh**:
  - Menampilkan daftar semua environment yang telah dibuat beserta status (`RUNNING` / `STOPPED`), kuota Disk, pemakaian RAM, alokasi CPU, dan alamat IPv4.
  - **Pilih Ukuran & Preset di Awal**: Bebas memilih preset **Desktop & Mobile (GPU + KVM + GUI)**, **Standard AI/Web (Docker)**, atau **Full KVM Virtual Machine**, serta menentukan batas ukuran Disk (`20GiB`–`100GiB`), RAM (`4GiB`–`12GiB`), dan CPU Core.
- **Katalog OS & Ketuk untuk Download (One-Tap OS Cache)**:
  - Tersedia daftar distro populer (**Ubuntu 24.04 LTS**, **Ubuntu 22.04 LTS**, **Debian 12**, **Arch Linux**, **Fedora 41**, **Alpine Linux**, **Kali Linux**, **Rocky Linux 9**).
  - Ketuk **Download OS** untuk menyimpan image OS ke cache lokal sehingga pembuatan environment baru selesai dalam **< 1 detik**.
- **Shared Folder Dua Arah Otomatis (`/workspace`)**:
  - Setiap environment otomatis terhubung ke `~/ai-workspaces/<nama-projek>` di OS utama dan `/workspace` di dalam environment dengan sinkronisasi izin file (ACL + UID shifting).
- **Snapshot Instan 1 Detik (Time Machine) & Wipe Bersih**:
  - Simpan kondisi seluruh OS sebelum AI melakukan perombakan besar, kembalikan (*restore*) kapan saja dalam 1 detik, atau hapus bersih (*wipe*) seluruh OS setelah projek selesai.
- **Headless Virtual Display Runner (`Xvfb`)**:
  - Jalankan *automated GUI testing* oleh AI di layar virtual `1920x1080` tanpa mengganggu layar atau kursor mouse Anda.

---

## Pintasan Keyboard (Keyboard Shortcuts)

| Tombol | Aksi |
| :--- | :--- |
| `Space` / `Enter` | Masuk ke terminal OS yang dipilih (langsung di `/workspace`) |
| `N` | Buat environment OS baru |
| `F` | Buka folder *shared workspace* di File Manager Host |
| `S` | Nyalakan (*Start*) atau matikan (*Stop*) environment |
| `P` | Simpan *Snapshot* Btrfs instan (1 detik) |
| `C` | Salin perintah eksekusi AI (`incus exec <nama> --cwd /workspace -- bash`) |
| `Delete` | Hapus total (*Wipe*) environment yang dipilih |
| `1` / `2` | Pindah antara tab **Environments** dan **OS Images & Downloads** |
| `↑` / `↓` | Pilih environment di daftar |
| `R` | Muat ulang (*Refresh*) status environment & cache OS |
| `?` | Tampilkan / sembunyikan panduan pintasan keyboard |
| `Q` | Keluar dari aplikasi |

---

## Cara Setup & Instalasi Bersih (di Mesin Baru / Setup Ulang)

Seluruh kode sumber dan skrip konfigurasi tersimpan rapi di repositori Git ini (`~/Work/aibox`).

### 1. Setup Sistem Incus (Jalankan Sekali Saja)
Menginstal Incus, mengaktifkan storage Btrfs + kuota disk, membuka jalur firewall UFW, dan membuat profil akselerasi GPU/KVM:
```bash
cd ~/Work/aibox
./bin/setup-system
```

### 2. Build & Install Aplikasi Desktop (`aibox`)
Mengompilasi aplikasi C++/Qt6 menjadi satu binary dan menginstalnya sebagai paket resmi Arch Linux (`pacman`) beserta ikon dan shortcut di **Omarchy App Launcher (`Super + Space`)**:
```bash
cd ~/Work/aibox
./bin/install
```

### 3. Menjalankan Unit Test
```bash
cd ~/Work/aibox
./bin/test
```

---

## Cara Uninstall dengan Bersih (100% Tanpa Sisa Sampah)

Karena aplikasi ini diinstal melalui paket sistem (`pacman` / `makepkg`), Anda bisa menghapusnya kapan saja tanpa meninggalkan file sampah di sistem:

* **Hapus Aplikasi GUI AIBox & Shortcut App Launcher Saja** (environment dan data projek tetap aman):
  ```bash
  cd ~/Work/aibox
  ./bin/uninstall
  # atau langsung lewat pacman:
  sudo pacman -Rns aibox
  ```

* **Hapus Total AIBox + Incus + Seluruh Container OS**:
  ```bash
  cd ~/Work/aibox
  ./bin/uninstall --full
  ```
