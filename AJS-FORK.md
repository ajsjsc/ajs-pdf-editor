# AJS PDF Editor – bản sửa đổi của PdfCraft

AJS PDF Editor là bản sửa đổi của [PdfCraft](https://github.com/storytold/pdfcraft), do ArtCraft team và những người đóng góp cho PdfCraft phát triển, phát hành theo giấy phép MIT OR Apache-2.0. AJS (AutoLISP Just Simple, https://lisp.vn) chỉnh sửa và phân phối bản này kèm các plugin AutoCAD. ArtCraft team không làm ra, không tài trợ và không xác nhận bản này.

## Các thay đổi so với upstream

| Thay đổi | Ở đâu |
|---|---|
| **Tắt tự cập nhật:** không kiểm tra GitHub khi khởi động; Help ▸ "Updates from AJS…" mở trang phát hành https://github.com/ajsjsc/ajs-pdf-editor/releases/latest (bản cài qua bộ cài AJS: AppCode `APE`, miễn phí) | `apps/pdfcraft/src/main.rs` (`set_startup_update_check(false)`), `crates/ui-egui/src/platform/updates.rs`, `crates/ui-egui/src/commands.rs` |
| **Đổi tên** "PdfCraft" → "AJS PDF Editor" trong mọi chuỗi hiển thị, thông tin exe, tiêu đề cửa sổ, thư mục dữ liệu | `tools/ajs/rebrand.pl` (chỉ sửa nội dung chuỗi trong mã Rust và bảng dịch `*.tsv`; giữ tên kiểu, định danh nội bộ và câu ghi nguồn gốc) |
| **Gỡ nhãn hiệu ArtCraft:** xoá `docs/brand/`, thay logo trong About và Home bằng logo AJS | `crates/ui-egui/src/widgets.rs` (`ajs_mark`), `dialogs.rs`, `home.rs` |
| **Icon AJS** (logo "A" của bộ cài AJS) | `assets/app-icon/pdfcraft.ico`, `hicolor/*`, `pdfcraft-1024.png`, `assets/ajs/` |
| **Link** Help, About, Home trỏ về AJS; giữ link mã nguồn PdfCraft gốc | `crates/engine/src/links.rs`, `crates/engine/src/commands.rs`, `crates/ui-egui/src/menus.rs` |
| **Ghi nguồn gốc** trong About, `NOTICE`, thông tin bản quyền của exe | `dialogs.rs`, `NOTICE`, `apps/*/build.rs` |
| **Bản dịch:** từ PdfCraft 0.21 bảng dịch nằm ở repo [storytold/craft-i18n](https://github.com/storytold/craft-i18n) (commit ghi trong `craft-i18n.rev`). Khi đóng gói, `rebrand.pl --catalogs` đổi tên trong bản sao các bảng dịch và nối thêm chuỗi tiếng Việt riêng của AJS | `tools/ajs/rebrand.pl`, `tools/ajs/i18n/vi.tsv`, `tools/ajs/package.ps1` (`-CraftI18n`) |
| **Tạo shortcut lên desktop:** Help ▸ "Create desktop shortcut" tạo `AJS PDF Editor.lnk` trên màn hình nền (Windows), trỏ tới exe đang chạy; cùng tên tệp với shortcut D2P tạo khi cài/cập nhật nên không bị trùng | `crates/platform/src/desktop_shortcut.rs`, `crates/engine/src/commands.rs`, `crates/ui-egui/src/commands.rs`, `menus.rs`, `platform/updates.rs` |

Tên exe phát hành: `AJSPdfEditor.exe` (giao diện) và `AJSPdfEditor-cli.exe` (dòng lệnh, dùng cho D2P ghép/tách/bookmark). Tên crate và tên tệp build trong `target\` vẫn là `pdfcraft`, để việc đồng bộ upstream ít xung đột.

## Nhánh

- `ajs/main`: bản AJS; mọi thay đổi của AJS nằm ở đây.
- Remote `upstream` = `https://github.com/storytold/pdfcraft.git`; chỉ đồng bộ theo **tag phát hành** (`v0.18.0`, …).

## Cập nhật lên bản PdfCraft mới (admin quyết định thời điểm)

AJS PDF Editor không tự kiểm tra hay tự cập nhật. Khi admin muốn đưa bản mới tới người dùng:

```powershell
cd C:\Dev\ajs-pdf-editor
tools\ajs\sync-upstream.ps1 -Tag v0.22.0          # merge tag, đổi tên lại, kiểm tra các thay đổi AJS
tools\ajs\package.ps1 -CraftFonts C:\Dev\craft-fonts -CraftI18n C:\Dev\craft-i18n
```

1. `sync-upstream.ps1` dừng nếu có xung đột và liệt kê các tệp bị xung đột. Khi gỡ xung đột phải giữ đủ các thay đổi trong bảng trên, rồi chạy lại script.
2. `craft-fonts` phải checkout đúng revision ghi trong `.github/workflows/release.yml` (`craft-fonts-ref`) của tag mới; `craft-i18n` (clone `https://github.com/storytold/craft-i18n`) phải checkout đúng commit trong `craft-i18n.rev` (script tự kiểm tra).
3. Kiểm tra bản build: mở PDF do D2P in ra, ghi chú, ký, lưu; chạy script ghép/tách/bookmark của D2P bằng `AJSPdfEditor-cli.exe run --script … --report`.
4. Đưa zip lên kênh phát hành của AJS (AJS Plugin Updater / `lics.autolisp.vn`).

## Build

- Rust stable (MSVC) ≥ `rust-version` trong `Cargo.toml`, Windows SDK (rc.exe), mạng (lần đầu tải crate và model OCR).
- `tools\ajs\package.ps1` build giống bản chính thức: CRT tĩnh, bắt buộc nhúng icon/VERSIONINFO, nhúng `craft-fonts`, nhãn build `ajs` (hiện trong `--version` và About: `0.18.0 (ajs)`).
- Kết quả: `dist-ajs\AJSPdfEditor-<ver>-windows-x64-portable.zip` + `.sha256`.

## Giấy phép

- Mã: MIT OR Apache-2.0 (`LICENSE-MIT`, `LICENSE-APACHE`), giữ nguyên `NOTICE` gốc và thêm phần ghi chú của AJS ở đầu.
- Font đi kèm: SIL OFL 1.1 (`OFL-*.txt`); model OCR: CC-BY-SA-4.0 (`models\ATTRIBUTION.txt`); icon tài liệu: ISC AND (MIT OR Apache-2.0) (`LICENSE-document.txt`).
- Tên, wordmark và logo ArtCraft là nhãn hiệu của ArtCraft Team, đã gỡ khỏi bản này theo `docs/brand/LICENSE-brand.txt` của upstream.
