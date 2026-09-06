# Rencana kompatibilitas Bruno Collection

## Keputusan

**Bisa**, tetapi bukan kompatibilitas penuh tanpa pekerjaan lanjutan. Zaku saat
ini menyimpan request native sebagai TOML dan hanya memahami HTTP statis. Bruno
menyimpan koleksi sebagai legacy `.bru` atau OpenCollection YAML (`.yml`), yang
juga dapat berisi GraphQL, environment, auth, script JavaScript, assertion, dan
beberapa protokol lain.

MVP yang direkomendasikan adalah **import satu arah, copy-on-write** untuk HTTP
statis dan **GraphQL-over-HTTP** (query atau mutation):

1. Pilih folder collection Bruno yang memiliki `opencollection.yml` atau
   `bruno.json`.
2. Salin request HTTP statis serta GraphQL query/mutation statis yang didukung
   ke folder collection Zaku baru dalam format TOML.
3. Tampilkan ringkasan request yang diimpor, dilewati, dan field yang tidak
   dapat dimigrasikan.
4. Jangan mengubah file Bruno asal.

Jalur ini berguna bagi pengguna sekarang, aman terhadap kehilangan data, dan
tidak mengubah asumsi scanner Zaku bahwa request adalah file `.toml`.

GraphQL pada MVP diterjemahkan menjadi request HTTP native: endpoint yang sama,
method `POST`, header, dan JSON envelope `{ "query", "variables" }`. Ini
membuat query dan mutation dapat dijalankan oleh transport Zaku yang ada, tetapi
belum menyediakan editor GraphQL khusus. GraphQL `GET` dan subscription tetap
didiagnostik sebagai belum didukung agar body tidak hilang atau semantik berubah.

Setelah importer stabil, direct-open **read-only** dan editor GraphQL native
dapat ditambahkan. Dukungan baca/tulis dua arah dan eksekusi penuh Bruno adalah
fase terpisah, bukan bagian dari MVP.

## Fakta yang menjadi batasan implementasi

- Format native hanya memuat method, URL, params, headers, dan body
  `text`/`json`/`html`/`xml` di `crates/worktree/src/request.rs`.
- Parser dan serializer native hanya TOML; save saat ini selalu menulis TOML.
- Scanner worktree dan project panel menganggap file `.toml` sebagai request.
- Semua params native Zaku ditambahkan sebagai **query string** ketika request
  dikirim. Path parameter Bruno tidak boleh dipetakan diam-diam ke params Zaku.
- Tidak ada model atau editor GraphQL native, maupun runtime untuk environment,
  variable interpolation, auth, script JavaScript, assertion, WebSocket, atau
  gRPC.

## Kontrak MVP

### Didukung

| Bruno input | Hasil import ke Zaku |
| --- | --- |
| HTTP method dan URL statis | Method dan URL yang sama |
| Query parameter aktif/nonaktif | `RequestFileParam` aktif/nonaktif |
| Header aktif/nonaktif | `RequestFileHeader` aktif/nonaktif |
| Body JSON, text, HTML, XML | Body raw dengan tipe yang sesuai |
| GraphQL `POST` query/mutation statis | HTTP `POST` dengan JSON envelope GraphQL |
| GraphQL variables opsional berupa object JSON valid dan statis | Field `variables` pada JSON envelope |
| Header GraphQL aktif/nonaktif | Header native; tambah `Content-Type: application/json` bila belum ada |
| Struktur folder request | Struktur folder salinan yang sama |
| `.bru` dan OpenCollection YAML | Keduanya diimpor; format tidak boleh tercampur dalam satu source collection |

### Tidak didukung pada MVP

- `{{variable}}`, `process.env`, `.env`, environment Bruno, dan secret
  variables.
- Auth Bruno, OAuth flow, client certificates, redirect/timeout settings.
- Path params, form-urlencoded, multipart/form-data, upload file, docs,
  examples, tags, sequence, collection/folder scripts, tests, dan assertions.
- GraphQL `GET`, subscription, schema fetch otomatis/query builder, persisted
  query extension, WebSocket, gRPC, apps, collection runner, dan JavaScript
  execution. Query introspection yang ditulis manual tetap dapat diimpor sebagai
  query `POST` biasa.

Jika salah satu field tidak didukung ditemukan, importer harus memberi
diagnostik per file dan **tidak** mengklaim request tersebut setara. Field yang
didukung tetap boleh diimpor hanya bila semantiknya tidak berubah; selain itu
request harus dilewati.

## Guardrail wajib

1. Jangan pernah menjalankan script JavaScript Bruno atau memanggil CLI `bru`.
2. Jangan membaca, menyalin, menampilkan, atau mencatat nilai dari `.env` dan
   environment secret.
3. Jangan menulis kembali ke source collection Bruno pada fase import maupun
   direct-open read-only.
4. Jangan mengklasifikasikan setiap file `.yml` di workspace sebagai request;
   hanya file di bawah root Bruno yang tervalidasi yang boleh diproses.
5. Parser `.bru` harus berupa lexer/parser blok, bukan regex yang rapuh.
6. Jangan melakukan fetch schema atau resource jaringan saat import.
7. Semua fixture harus sintetis dan bebas token, password, host internal, atau
   data pelanggan.
8. Terjemahkan GraphQL hanya ketika query dan variables sudah statis; query
   tidak boleh kosong dan variables, bila ada, harus object JSON valid. Jangan
   mengevaluasi `{{variable}}`, `process.env`, atau script untuk membuatnya.

## TODO terurut dan kecil

Setiap item di bawah dirancang sebagai satu PR kecil yang dapat dikerjakan model
AI murah. Kerjakan berurutan; jangan memulai item yang bergantung pada item
sebelumnya sebelum test-nya hijau.

### BRUNO-01 — Tetapkan kontrak dan corpus fixture

- [x] Tambahkan fixture sintetis untuk HTTP sederhana, header/param disabled,
  body JSON/text/XML/HTML, GraphQL query, GraphQL mutation, variables JSON,
  serta satu fixture untuk setiap fitur unsupported.
- [x] Tambahkan tabel mapping Bruno → Zaku dan error/warning code yang stabil.
- [x] Cadangkan diagnostic code GraphQL: `BRUNO_GQL_METHOD_UNSUPPORTED`,
  `BRUNO_GQL_QUERY_EMPTY`, `BRUNO_GQL_VARIABLES_INVALID`, dan
  `BRUNO_GQL_DYNAMIC_VALUE`.
- [x] Gunakan placeholder credential yang eksplisit seperti
  `__REDACTED_TEST_SECRET__`; tambahkan test yang memastikan setiap nilai
  credential fixture memakai placeholder tersebut. Nama field seperti
  `password` atau `token` tetap boleh ada agar parser dapat diuji.

**Selesai jika:** corpus membedakan `importable`, `importable_with_warnings`,
dan `skipped`, tanpa mengubah perilaku request TOML yang sudah ada.

### BRUNO-02 — Buat model import murni dan laporan diagnostik

- [ ] Di crate `worktree`, buat modul khusus Bruno dengan tipe tanpa UI:
  `BrunoFormat`, `BrunoImportReport`, `BrunoImportWarning`,
  `ImportedRequest`, dan penanda bahwa request asal adalah HTTP atau GraphQL.
- [ ] `ImportedRequest` harus menyimpan `RequestFile` native hasil mapping,
  relative source path, dan warning terstruktur.
- [ ] Pisahkan discovery source collection dari parsing isi request agar unit
  test tidak membutuhkan `gpui`, filesystem nyata, atau network.
- [ ] Jangan mengubah `RequestFile`, parser TOML, atau writer TOML pada item
  ini.

**Selesai jika:** test dapat membuat report untuk input tiruan dan error punya
path source yang jelas.

### BRUNO-03 — Deteksi root collection dengan aman

- [ ] Implementasikan discovery berdasarkan manifest `opencollection.yml`
  (modern) atau `bruno.json` (legacy).
- [ ] Tolak source yang memiliki format request `.bru` dan `.yml` tercampur.
- [ ] Terapkan daftar ignore dari manifest bila tersedia, dan selalu abaikan
  `.env`, `node_modules`, `.git`, dan file script sebagai input request.
- [ ] Test nested folder, manifest hilang, YAML non-Bruno, mixed format, dan
  ignored directory.

**Selesai jika:** hasil discovery adalah daftar path request yang eksplisit;
file YAML konfigurasi lain tidak pernah ikut terpilih.

### BRUNO-04 — Parser OpenCollection YAML untuk subset HTTP dan GraphQL

- [ ] Pilih parser YAML Rust yang terpelihara, tambahkan secara eksplisit ke
  workspace manifest/lockfile, dan parse tanpa fetch schema saat runtime.
- [ ] Parse `info` dan `http` hanya untuk request `type: http`.
- [ ] Tambahkan parsing request `type: graphql` dan blok `graphql` berdasarkan
  schema OpenCollection versi yang dipin; gunakan fixture yang dibuat Bruno
  aktual sebagai referensi field endpoint, method, query, dan variables.
- [ ] Map method, URL, query params, headers, disabled flag, dan body
  JSON/text/HTML/XML ke `RequestFile`.
- [ ] Untuk GraphQL `POST` statis, hasilkan `RequestFile` HTTP `POST` dengan
  JSON envelope tervalidasi; variables opsional harus object JSON valid atau
  request dilewati.
- [ ] Emit warning atau skip untuk path params, auth, runtime/scripts,
  settings, variable interpolation, unsupported body type, GraphQL
  subscription, dan protocol selain HTTP/GraphQL.
- [ ] Tambahkan test positif, malformed YAML, unknown field, dan mapping yang
  tidak boleh mengubah semantic request.

**Selesai jika:** semua fixture YAML HTTP/GraphQL yang importable menghasilkan
TOML native yang dapat diparse lagi oleh `parse_request_file` dan body GraphQL
berbentuk JSON valid.

### BRUNO-05 — Parser legacy `.bru` untuk subset HTTP dan GraphQL

- [ ] Implementasikan tokenizer dan parser block-based untuk `meta`, method
  block (`get`, `post`, `put`, `patch`, `delete`, `options`, `head`, `trace`,
  `connect`, atau `http`), `headers`, `params:query`, dan body yang didukung.
- [ ] Tangani quoted key, multiline value, dan prefix `~` pada header/param.
- [ ] Untuk `meta.type: graphql`, parse body `body:graphql` dan optional
  `body:graphql:vars`; terjemahkan hanya query/mutation statis ke JSON envelope
  HTTP pada modul mapping terpisah.
- [ ] Simpan unknown block dan field unsupported sebagai diagnostik, bukan
  mengabaikannya secara senyap.
- [ ] Tambahkan test malformed brace, quoted key, disabled item, multiline
  body, GraphQL variables invalid, dan request non-HTTP/non-GraphQL.

**Selesai jika:** parser tidak panic untuk file `.bru` salah format dan hasil
mapping fixture HTTP dan GraphQL setara dengan YAML fixture yang sama.

### BRUNO-06 — Pemetaan GraphQL dan test semantik

- [ ] Buat fungsi murni yang membentuk JSON envelope GraphQL dari query dan
  variables tanpa melakukan interpolation.
- [ ] Jangan menimpa header `Content-Type` yang eksplisit; warning jika nilainya
  tidak kompatibel dengan JSON.
- [ ] Tambahkan test yang menangkap request transport dan memeriksa method,
  URL, headers, serta body untuk query, mutation, variables, dan named
  operation di dalam query.
- [ ] Pastikan response GraphQL (`data` dan/atau `errors`) tetap tampil utuh
  melalui response panel HTTP yang ada; jangan membuang atau menulis ulang field
  `errors`.

**Selesai jika:** query dan mutation fixture dapat dikirim sebagai HTTP request
yang ekuivalen tanpa runtime GraphQL atau JavaScript tambahan.

### BRUNO-07 — Orkestrasi import copy-on-write

- [ ] Buat service yang menerima source collection directory dan destination
  directory baru, memakai discovery + parser dari item sebelumnya.
- [ ] Tulis hanya request yang berstatus importable ke file `.toml`; pertahankan
  struktur folder dan gunakan nama file yang aman dari collision.
- [ ] Jika destination sudah ada atau write gagal di tengah jalan, jangan
  overwrite source dan jangan meninggalkan import parsial tanpa report yang
  jelas.
- [ ] Tulis manifest/report import lokal tanpa menyertakan isi secret atau
  raw value unsupported.
- [ ] Test temporary filesystem untuk success, collision, malformed request,
  dan rollback/cleanup.

**Selesai jika:** source collection byte-for-byte tidak berubah dan setiap
request output dapat dibuka oleh Zaku sebagai `.toml` native.

### BRUNO-08 — UI action untuk import dan ringkasan hasil

- [ ] Tambahkan action/menu `Import Bruno Collection…` di lokasi UI yang
  sesuai dengan pola existing action dan file picker Zaku.
- [ ] Minta destination baru, bukan source directory sebagai output.
- [ ] Tampilkan jumlah imported/skipped/warnings dan path report tanpa
  mengekspos nilai environment atau secret.
- [ ] Tambahkan test action/UI minimum sesuai pola test `project_panel`.

**Selesai jika:** pengguna dapat mengimpor collection tanpa menyentuh file
Bruno asal, kemudian membuka request hasil import di Zaku.

### BRUNO-09 — Acceptance test end-to-end dan dokumentasi pengguna

- [ ] Tambahkan integration test untuk folder Bruno contoh sintetis → folder
  Zaku TOML → request HTTP dan GraphQL terbuka di editor dan dapat dikirim.
- [ ] Dokumentasikan scope dukungan, daftar fitur yang dilewati, dan cara
  memeriksa report import.
- [ ] Verifikasi formatter serta test crate yang terkena perubahan. Jalankan
  minimal `cargo test -p worktree`; tambah `cargo test -p project_panel` bila
  UI berubah.

**Selesai jika:** dokumentasi tidak menjanjikan full compatibility dan test
menutup jalur sukses serta kegagalan utama.

## Fase setelah MVP (butuh review manusia)

### Direct-open read-only

- [ ] Tambahkan source-format ke request buffer, parser, metadata panel, dan
  scanner.
- [ ] Kenali `.bru`/`.yml` hanya ketika berada di bawah manifest Bruno yang
  tervalidasi.
- [ ] Tampilkan request dengan banner read-only dan nonaktifkan Save.
- [ ] Jangan membuat `.toml` baru di collection Bruno karena format campuran
  tidak valid di Bruno.

### Dukungan eksekusi lanjutan

- [ ] Rancang resolver variable dan environment yang eksplisit tanpa membaca
  `.env` otomatis.
- [ ] Tambahkan path parameter, basic/bearer/API-key auth, timeout, redirect,
  dan form body satu per satu dengan test semantik.
- [ ] Tambahkan model TOML versi baru dan editor GraphQL khusus (endpoint,
  method, query, variables) setelah importer GraphQL stabil; pertahankan
  kompatibilitas baca untuk request TOML versi 1.
- [ ] Jadikan subscription GraphQL sebagai proyek terpisah karena memerlukan
  transport WebSocket dan protokol `graphql-transport-ws`; jangan menyamakan
  dengan query/mutation HTTP.
- [ ] Tentukan sandbox keamanan sebelum mempertimbangkan script/test JavaScript;
  jangan menggunakan runtime host tanpa isolasi.

### Round-trip dua arah

- [ ] Bangun AST/serializer format-aware yang lossless untuk `.bru` dan YAML.
- [ ] Simpan field yang belum didukung tanpa data loss dan validasi hasil dengan
  schema OpenCollection serta Bruno versi yang ditetapkan.
- [ ] Aktifkan Save hanya setelah test round-trip mencakup auth, docs, runtime,
  body, dan unknown extension.

## Referensi implementasi

- Bruno BRU language: <https://docs.usebruno.com/bru-lang/language>
- Bruno BRU tag reference: <https://docs.usebruno.com/bru-lang/tag-reference>
- Bruno GraphQL request: <https://docs.usebruno.com/send-requests/graphql/graphql-api>
- Bruno GraphQL variables: <https://docs.usebruno.com/send-requests/graphql/graphql-variables>
- OpenCollection YAML structure: <https://docs.usebruno.com/opencollection-yaml/structure-reference>
- Bruno migration BRU → YAML: <https://docs.usebruno.com/bru-lang/yaml-migration>
- OpenCollection specification v1.0.0: <https://spec.opencollection.com/>
