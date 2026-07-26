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
- Taslakları iki kliple erken inceleyebilirsin; ancak `eval-harness run` fail-closed çalışır ve her kovada ≥5 geçerli çift olmadan gate raporu üretmez. `--quick N` kullanılıyorsa `N ≥ 5` olmalıdır.
- **KVKK:** kayıttaki herkesten izin al.

## Referans transkript akışı
1. Klipleri `raw/<kova>/` altına koy.
2. Harness her klip için taslak transkript üretir (`<id>.draft.txt`).
3. Sen taslağı düzeltip `<id>.ref.txt` olarak onaylarsın (insan-doğrulamalı referans şartı).

`.ref.txt` dosyasının varlığı, bu akışta insan onayı beyanıdır; makine taslağını kontrol etmeden
yalnızca yeniden adlandırmak kabul edilmez. `run`, model yüklemeden önce her kovada en az beş çift,
boş olmayan referans ve 16 kHz mono s16 WAV biçimini doğrular; eksikte rapor yazmadan durur.

## Jargon listesi ve domain setleri
`jargon.txt` taslağı intralojistik/depo otomasyonu için hazırlandı — **yanlışları sil, kendi ürün/parça adlarını ekle** (müşteri adları, ürün kodları, marka adları çok değerli).

Birden fazla domain ölçüyorsan (ör. hukuk + intralojistik) **her domain kendi set dosyasını alır**:

| Dosya | Set adı |
|---|---|
| `jargon.txt` | `default` — sidecar'ı olmayan tüm klipler |
| `jargon.<ad>.txt` (ör. `jargon.legal.txt`) | `<ad>` — yalnız o seti seçen klipler |

Bir klibin setini `<kova>/<id>.vocab.txt` dosyasıyla seçersin (tek satır set adı; `#` yorum olabilir),
tıpkı `<id>.lang.txt` gibi:

```
eval/jargon/jl01.wav
eval/jargon/jl01.ref.txt
eval/jargon/jl01.vocab.txt   →  içinde tek satır:  legal
```

Bu ayrım **şart**, kolaylık değil: whisper'ın `initial_prompt` bütçesi ~600 karakter (≈224 token), yani
tek birleşik listede prompt'a **yalnızca dosyada önce yazılan domain** girer ve diğer domainin
`*_vocab` koşusu sessizce yanlış sözlükle ölçülür. Set adı hem prompt bias'ını hem terim-recall
skorunu belirler. Tanımsız bir set adı yazarsan `run` **model yüklemeden** durur ve tanımlı setleri
listeler.

Koşu ve rapor: `docs/PHASE0_VALIDATION.md` + `eval-harness` (workspace bin). Çıktılar:
`eval/report.md` + `eval/report.json` → GO / CONDITIONAL / NO-GO kararı insana sunulur.
Raporun multi-speaker/diyarisasyon sanity alanını bir insan doldurur. Raporlanan `wall_secs` ve RTF
batch çalışma süresidir; canlı arayüzde konuşmadan ilk metne kadar geçen TTFT değildir.
