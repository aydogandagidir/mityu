# Phase 0 Değerlendirme Seti — Kayıt Talimatı (docs/PHASE0_VALIDATION.md'nin uygulaması)

Amaç: Mityu'nun transkripsiyon kalitesini **senin gerçek ortamında** ölçmek (WER/CER + jargon terim yakalama + gecikme). Halka açık hazır veri seti KULLANILMAZ.

## Ne kaydedeceksin (4 kova)

| Klasör | Ortam | Klip | Süre |
|---|---|---|---|
| `raw/quiet/`  | Sessiz oda/ofis, 1-2 konuşmacı, doğal konuşma | ≥5 | 2-10 dk |
| `raw/field/`  | Gerçek saha: depo/şantiye gürültüsü, konveyör/forklift sesi, yankı | ≥5 | 2-10 dk |
| `raw/multi/`  | 3+ konuşmacı, araya girmeler | ≥5 | 2-10 dk |
| `raw/jargon/` | İntralojistik terimleri yoğun (bkz. `jargon.txt`), **TR ağırlıklı + TR-EN karışık** | ≥5 | 2-10 dk |

- Format serbest (m4a/mp3/wav; telefon olur) — dönüşüm/normalizasyon otomatik yapılır.
- Dosya adı: kısa ve benzersiz (örn. `q1.m4a`, `f3.wav`).
- Erken sinyal için kova başına 2 kliple ön-tur koşulabilir; gate kararı ≥5 ister.
- **KVKK:** kayıttaki herkesten izin al.

## Referans transkript akışı
1. Klipleri `raw/<kova>/` altına koy.
2. Harness her klip için taslak transkript üretir (`<id>.draft.txt`).
3. Sen taslağı düzeltip `<id>.ref.txt` olarak onaylarsın (insan-doğrulamalı referans şartı).

## Jargon listesi
`jargon.txt` taslağı intralojistik/depo otomasyonu için hazırlandı — **yanlışları sil, kendi ürün/parça adlarını ekle** (müşteri adları, ürün kodları, marka adları çok değerli).

Koşu ve rapor: `docs/PHASE0_VALIDATION.md` + `eval-harness` (workspace bin). Çıktılar: `eval/report.md` + `eval/report.json` → GO / CONDITIONAL / NO-GO kararı insana sunulur.
