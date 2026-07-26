# A5 Sprint — Phase 0 Transkripsiyon Doğrulama (yürütme runbook'u)

> **Bu doküman ne DEĞİL:** protokol değil. Protokol = `PHASE0_VALIDATION.md` (ne ölçülür, nasıl),
> toplama kuralları = `PHASE0_EVIDENCE_ACQUISITION.md` (izin, kanıt, 20-kayıt planı).
> **Bu doküman:** o protokolü kapatmak için kim, ne sırayla, hangi komutla ne yapar.
>
> **Kapı statüsü:** A5 = `DEFERRED / NOT EVALUATED`, dört kovada da **0/5** (ADR-0027 yalnızca
> v1.0.4 *yayınını* engellememesi için erteledi — PASS saymadı). Bu sprint o satırı kapatır.

---

## 1. Bugünkü gerçek durum (2026-07-26'da diskten doğrulandı)

**Hazır olan (yeniden yapılmayacak):**

| Varlık | Durum |
|---|---|
| `eval-harness/` Rust bin (6 dosya, ~1.956 satır) | Implemente; workspace member; app'in kendi motorlarını sürer (whisper-rs + ort) |
| Fail-closed gate | `MIN_GATE_CLIPS_PER_BUCKET = 5`, model yüklemeden önce doğrular (`main.rs:33`) |
| `ggml-large-v3.bin` (3,1 GB) + `ggml-large-v3-turbo.bin` (1,6 GB) | İnmiş, `%APPDATA%\com.bluedev.mityu\models` |
| Parakeet `parakeet-tdt-0.6b-v3-int8` | İnmiş |
| `tools/phase0/verify-evidence.ps1` | Hazır |
| `eval/evidence/manifest.template.csv` | Hazır, 20 satır iskelet |
| `eval/jargon.txt` | 72 terim — **ama intralojistik** (bkz. §3.2) |

> ⚠️ **`eval/run_eval.py` YAZILMAYACAK.** `PHASE0_VALIDATION.md` §5'teki Python iskeleti
> **tarihsel referans**; §7 onu `eval-harness` ile değiştirdi. Python yolunu diriltmek regresyondur.

**Eksik olan — tek engel insan işi:**

```
eval/quiet/  0 wav / 0 ref   (hedef 5)
eval/field/  0 wav / 0 ref   (hedef 5)
eval/multi/  0 wav / 0 ref   (hedef 5)
eval/jargon/ 0 wav / 0 ref   (hedef 10 — §2 kararı)
eval/raw/**  tamamen boş
```

Yani bu sprint **bir yazılım sprinti değil, bir kanıt-toplama sprintidir.** Kod tarafında yalnızca
§3'teki iki küçük borç var.

---

## 2. Kilitlenen kararlar (sahip, 2026-07-26)

1. **Eval domain'i = İKİSİ BİRDEN.** `jargon` kovası ikiye bölünür: **5 hukuk + 5 intralojistik =
   10 klip**. Gerekçe: strateji beachhead'i hukuk (`STRATEGY_2026-2030.md`), ama mevcut intralojistik
   varlıklar ve erişim korunur. Toplam kayıt: **25 klip**.
2. **Eşikler = dokümandaki bar, ölçümden ÖNCE kilitli.** GO = medyan WER ≤ %15 (quiet), ≤ %25
   (field), jargon terim-recall ≥ %80. Türkçe için diyakritik-katlanmış **CER ikincil metrik** olarak
   raporlanır. Bu rakamlar ölçüm başlamadan ADR'ye yazılır (§4, Aşama 0).
3. **Katılımcı erişimi belirsiz** → tam plan yazılır, `multi`/`field` için §8 fallback dalları
   şartlı olarak işlenir.

**Varsayımım (blokla­madım, işaretliyorum):** soru `jargon` bölünmesi üzerineydi; `field` kovası 5
klip kalır ama **2'si hukuk-bağlamı** (adliye koridoru / keşif), **3'ü endüstriyel** olacak şekilde
bölünür — böylece saha kanıtı da beachhead'e transfer olur. Farklı istersen §5 tablosunu değiştir.

---

## 3. Sprint öncesi teknik borç (2 madde — bunlar gerçek, keşfedildi)

### 3.1 Harness derleme reçetesi (day-0 riski)

**Gözlemlendi (2026-07-26):** `cargo check -p eval-harness` düz Git Bash PATH'iyle **başarısız** —
vendored OpenSSL, perl `Configure` adımında patlıyor (MSYS perl, Strawberry perl'i gölgeliyor).
Aşağıdaki reçete **bu makinede doğrulandı**: temiz `Finished dev profile in 19m 45s`, exit 0.
(11 uyarı `mityu` lib'inin mevcut baseline'ı; `eval-harness` uyarısız derlenir.)
İlk derleme ~20 dk sürer — sonrası incremental:

```bash
cmd.exe /c 'call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" && set "PATH=C:\Strawberry\perl\bin;C:\Program Files\CMake\bin;C:\Program Files\LLVM\bin;%USERPROFILE%\.cargo\bin;%PATH%" && set "LIBCLANG_PATH=C:\Program Files\LLVM\bin" && cargo check -p eval-harness'
```

Kayıt toplamaya başlamadan **önce** bu komutun yeşil olduğu doğrulanmalı. 20 saat kayıt toplayıp
harness'in derlenmediğini keşfetmek sprintin en pahalı hatası olurdu.

### 3.2 Klip-başı vocabulary seçimi (iki-domain kararının zorunlu sonucu)

**Sorun:** `main.rs:528` jargon listesini **bir kez global** yükler, `main.rs:539` **tek bir**
vocab prompt'u üretir, ve `engines.rs:30` `MAX_PROMPT_CHARS = 600` (whisper'ın ~224 token bütçesi).
Birleşik ~140 terimlik listede prompt ~40-50 terimde kesilir ve **dosyada önce yazılan domain**
bias'ı alır; diğeri hiç almaz. Sonuç: `*_vocab` konfigürasyonları iki domainden biri için sessizce
geçersiz ölçüm üretir.

**Metrikte sorun yok:** `metrics.rs:63-76` `term_recall` kendini filtreler — yalnızca referansta
*geçen* terimleri puanlar. Yani birleşik liste terim-recall için doğrudur; bozulan sadece prompt
enjeksiyonu.

**Düzeltme — YAPILDI (2026-07-26).** Klip-başı **vocab seti** desteği eklendi. Plan başta "klip id
öneki" (`jl*`/`ji*`) diyordu; uygulamada **sidecar**'a çevrildi çünkü harness'te zaten aynı desende
`<id>.lang.txt` var ve sidecar yazım hatasında sessizce yanlış sete düşmez:

- **Set tanımı:** `eval/jargon.txt` = `default` seti; her `eval/jargon.<ad>.txt` = `<ad>` seti.
- **Klip seçimi:** `eval/<kova>/<id>.vocab.txt` içinde set adı (tek satır, `#` yorum olabilir).
  Sidecar yoksa `default`.
- **Etkilediği iki şey:** whisper `initial_prompt` bias'ı **ve** terim-recall skoru — ikisi de artık
  klibin kendi setinden gelir. Yani hukuk klibi başka domainin terimleriyle ne biaslanır ne puanlanır.
- **Fail-closed (üç ayrı yerde):** (a) tanımsız set adı **model yüklenmeden** hata verir
  (`validate_clips`) ve tanımlı setleri listeler; (b) sidecar okunamıyorsa veya **UTF-8 değilse**
  (Windows PowerShell 5.1 `>` yönlendirmesi UTF-16 yazar) koşu durur — `default`'a sessiz düşüş yok;
  (c) jargon kovasında birden fazla domain varsa **her domain ≥5 klip** ister
  (`validate_jargon_domain_coverage`), çünkü her biri kendi eşiğiyle kapılanıyor.
- **`--quick N` domain başına uygulanır.** Kova başına uygulansaydı alfabetik sıra (`ji*` < `jl*`)
  `--quick 5` ile hukuk domainini tamamen eler, kova kapsamı yine 5/5 geçer ve rapor tek-domainli
  bir "PASS" gibi okunurdu — hukuk hiç ölçülmemiş olmasına rağmen.
- `jargon.default.txt` adı `default` setiyle çakıştığı için reddedilir.

Dosyalar: `eval-harness/src/main.rs` (`VocabSet`/`VocabSets`, `read_vocab_set`, `load_vocab_sets`),
`eval-harness/src/engines.rs` (`load_jargon` → yola göre `load_jargon_file`, `MAX_PROMPT_CHARS` pub).
8 yeni test, `each_clip_gets_only_its_own_domain_vocabulary` doğrudan bu regresyonu pinliyor.

### 3.3 Rapor domain ayrımı — YAPILDI (2026-07-26)

**Sorun neydi:** `report.rs` terim yakalamayı **tüm `jargon` kovasının tek medyanı** olarak
hesaplayıp eşik tablosunda tek sütun gösteriyordu. İki domain aynı kovada olduğu için bu sayı
hukuk ile intralojistiği harmanlıyordu — ve kapı kararı tam da bu sayıya bakıyor.

Somut örnek (teste gömüldü): hukuk 0,60 (eşiği geçemez) + intralojistik 1,00 → harmanlanmış medyan
**tam 0,80** çıkar ve `PASS` okunur. Yani hukukun başarısızlığı tamamen görünmez olur.

**Yapılan:** domain kimliği olarak klibin **vocab seti** kullanıldı (§3.2'den; açık ve doğrulanmış).
- `Row.vocab_set` alanı eklendi; her satır hangi sözlükle puanlandığını taşır.
- `compute_domain_medians` → `config|kova|set` kırılımı; JSON'a `medians_by_domain` +
  `jargon_domains` olarak yazılır.
- **Eşik tablosu artık `config × jargon-domain` satırları üretir** — her domain kendi terim-yakalama
  sayısı ve kendi PASS/FAIL'i ile. GO metni "**her jargon domaininde** ≥ eşik" olarak düzeltildi.
- Birden fazla domain varsa rapora uyarı bloğu ve ayrı bir `config|kova|domain` medyan tablosu
  eklenir; tek domainli koşuda rapor eskisi gibi sade kalır.
- Klip tablosuna `Domain` sütunu eklendi (kanıt artefaktında hangi sözlüğün puanladığı görünür).

Testler: `merged_bucket_median_hides_a_failing_domain` yukarıdaki 0,60/1,00 → 0,80 senaryosunu
birebir pinliyor; `threshold_table_reports_each_jargon_domain_on_its_own_row`,
`single_domain_run_needs_no_domain_warning`, `run_without_jargon_clips_reports_recall_as_unavailable`.

---

## 4. Yürütme — 4 aşama

### Aşama 0 · Hazırlık (kayıt yok, ~1 gün)
1. §3.1 derleme reçetesini doğrula (yeşil `cargo check`).
2. §3.2 klip-başı vocab düzeltmesini yap + test ekle.
3. **Vocab setleri.** `eval/jargon.legal.txt` oluşturuldu (§6) — kendi çalışma alanına göre buda.
   `eval/jargon.txt` (mevcut intralojistik liste) `default` seti olarak kalır.
   Kayıtlar geldikten sonra her `jl*` klibi için `eval/jargon/<id>.vocab.txt` dosyasına tek satır
   `legal` yaz; `ji*` klipleri sidecar'sız kalıp `default`'u kullanır.
   **Müşteri adlarını ve gizli ürün kodlarını temizle** (`eval/README.md` uyarısı).

   *Bilinçli tercih:* `default` seti `quiet`/`field`/`multi` kliplerine de uygulanır — yani
   `*_vocab` konfigleri "domain prompt'u temiz sesi bozuyor mu?" sorusunu da ölçer (bugünkü
   davranış korundu). Katı domain izolasyonu istersen: `jargon.intra.txt` oluştur, `ji*` kliplerine
   `intra` sidecar'ı ver ve `jargon.txt`'i boşalt — o zaman domain-dışı klipler hiç bias almaz.
4. ~~**Eşik ADR'sini yaz** (§2.2 rakamları) → `DECISIONS.md`.~~ **YAPILDI** — ADR-0031
   (2026-07-26): §4 barı aynen kilitlendi, terim yakalama **domain başına** kapılandı, Türkçe CER
   ikincil metrik (barı gevşetmez), verdict insanda kalır. Rakamlar ölçümden önce yazıldı;
   değiştirmek yeni bir ADR gerektirir — bu ADR sonuçlar bilindikten sonra düzenlenemez.
5. KVKK aydınlatma + açık rıza metinlerini hazırla — **ayrı ayrı** belgeler
   (KVKK 18.02.2026 / 2026-347 ilke kararı). Hukukçu onayı gerekir.
6. `eval/evidence/manifest.template.csv` → `manifest.csv`; 25 satıra genişlet.

### Aşama 1 · Kayıt (~1-2 hafta, insan hızına bağlı)
- §5 tablosuna göre 25 kayıt. Format serbest (m4a/mp3/wav, telefon olur).
- Her kayıttan **önce** rıza; katılımcı adı/imza **Git dışı şifreli kasada**, manifest'e yalnızca
  opak `consent_evidence_id`.
- Dosyalar → `eval/raw/<bucket>/<id>.<ext>`.
- **Kırmızı çizgiler:** gerçek müşteri toplantısı yok, gizli iş yok, hareket hâlindeki araç yok,
  aktif tehlikeli saha yok. Prompt = gündem, senaryo metni değil — doğal konuşulacak.

### Aşama 2 · Normalize + taslak + insan referansı (~3-5 gün)
```bash
cargo run -p eval-harness -- prep            # → eval/<bucket>/<id>.wav (16 kHz mono s16)
cargo run -p eval-harness -- draft --engine whisper   # → <id>.draft.txt
```
Sonra **insan** her klibi baştan sona dinler, taslağı düzeltir, `<id>.ref.txt` olarak kaydeder.

Referans kuralları (`PHASE0_EVIDENCE_ACQUISITION.md` §4):
- Yalnız **konuşulan kelimeler**. Konuşmacı etiketi / zaman damgası / `[overlap]` **yazma** —
  normalizer bunları kelime sayar, WER'i bozar.
- Gerçekten söylenen dolgu sesleri, tekrarlar, yarım cümleler, code-switch'ler **korunur**.
  Konuşmacının gramerini "düzeltme", gündem metnini yapıştırma.
- Taslağı okumadan yeniden adlandırmak **inceleme değildir** ve kapıyı geçersiz kılar.
- İnsan; onay zamanı + audio/reference SHA-256 → `manifest.csv`.

### Aşama 3 · Ölçüm + verdict (~1 gün + insan kararı)
```bash
pwsh tools/phase0/verify-evidence.ps1        # kanıt bütünlüğü (izin/süre/hash/boş referans)
cargo run -p eval-harness -- run             # → eval/report.json + eval/report.md
```
Varsayılan 6 konfig: `whisper_large_v3(_vocab)`, `whisper_large_v3_turbo(_vocab)`,
`parakeet(_vocab)`. Notlar:
- `run` fail-closed: her kovada ≥5 geçerli çift yoksa **model yüklemeden** durur.
- `--quick N` yalnız daha büyük seti kırpar; `N < 5` gate raporu üretemez.
- Parakeet'in app entegrasyonunda hotword/vocab biasing **yok** → `parakeet_vocab` düz koşar,
  rapor bunu not eder. Bu bir bug değil, bilinen sınır.
- `large-v3-turbo` eksikse turbo konfigleri atlanır ("ATLANDI" notu), koşu durmaz.

Sonra **insan**: rapordaki diyarizasyon/çok-konuşmacı sanity alanını doldurur, canlı TTFT smoke
testini ayrıca yapar (harness'in `wall_secs`/RTF'i batch throughput'tur, **UI TTFT değildir**),
ve GO / CONDITIONAL / NO-GO imzalar.

---

## 5. Kayıt planı — 25 klip

Süre hedefi 3-5 dk (kabul aralığı 2-10 dk). Ayrı ayrı durumlar; tek uzun seansı parçalara bölmek
beş bağımsız kayıt sayılmaz.

**quiet (5)** — sessiz oda/ofis, 1-2 kişi, iyi mikrofon. `PHASE0_EVIDENCE_ACQUISITION.md` §2'deki
`q01`-`q05` aynen geçerli (sprint planlama, olay incelemesi, sözlü devir, tedarik karşılaştırma,
destek vakası).

**field (5)** — gerçek ortam gürültüsü. Gürültü dosyasını hoparlörden çalmak **geçersiz**.

| ID | Kişi | Senaryo | Karakteristik |
|---|---:|---|---|
| `f01` | 2 | Adliye koridoru / duruşma öncesi bekleme | Yankı, kalabalık uğultusu, uzak konuşmalar |
| `f02` | 2-3 | Keşif (yerinde inceleme) yürüyüşü | Dış mekân, rüzgâr, değişen mikrofon mesafesi |
| `f03` | 2 | Depo rampası incelemesi (yetkili güvenli nokta) | Gerçek yankı + uzak elleçleme sesi |
| `f04` | 2 | Konveyör/fan incelemesi (güvenli alan) | Sürekli makine gürültüsü |
| `f05` | 3 | Yankılı depoda vardiya devri | Yankı + arka plan sesler + 3 konuşmacı |

**multi (5)** — ≥3 konuşmacı, doğal kısa üst üste binmeler. `m01`-`m05` aynen geçerli
(koordinasyon, retrospektif, tasarım ödünleşimi, vardiya devri, backlog önceliklendirme).

**jargon (10)** — TR ağırlıklı, doğal TR-EN code-switching.

| ID | Kişi | Senaryo | Domain |
|---|---:|---|---|
| `jl01` | 2 | Sözleşme müzakeresi: fesih, cezai şart, tazminat maddeleri | Hukuk |
| `jl02` | 2 | Dava strateji toplantısı: bilirkişi raporuna itiraz, istinaf yolu | Hukuk |
| `jl03` | 2 | Müvekkil ilk görüşme: olay anlatımı, zamanaşımı, vekalet ücreti | Hukuk |
| `jl04` | 3 | KVKK uyum danışmanlığı: veri sorumlusu, açık rıza, ihlal bildirimi | Hukuk |
| `jl05` | 2 | Due-diligence / hisse devri devri — TR-EN yoğun | Hukuk |
| `ji01` | 2 | WMS-WCS-ERP entegrasyon incelemesi | İntralojistik |
| `ji02` | 2 | PLC/HMI/sensör arıza teşhisi | İntralojistik |
| `ji03` | 3 | AS/RS, AGV, AMR yerleşim incelemesi | İntralojistik |
| `ji04` | 2 | OEE, throughput, çevrim süresi incelemesi | İntralojistik |
| `ji05` | 2 | Kestirimci bakım + yedek parça devri | İntralojistik |

Hukuk senaryoları **kurgusal vaka** üzerinden oynanır — gerçek müvekkil/dosya **asla**.
Avukat-müvekkil gizliliği bu sprintin en sert kırmızı çizgisi.

---

## 6. `eval/jargon.legal.txt` — taslak terim listesi

Aşağıdaki liste bir başlangıçtır; **kendi dosya türlerine ve sık kullandığın terimlere göre
budayıp genişlet**. Terim-recall yalnızca referansta geçen terimleri puanladığı için fazla terim
zarar vermez, ama vocab prompt'u ~40-50 terimde kesilir → **en ayırt edici 40'ı başa koy**.

```
dava · davacı · davalı · müvekkil · vekaletname · vekalet ücreti · ihtarname
icra takibi · haciz · tebligat · dilekçe · cevap dilekçesi · replik · düplik
bilirkişi · bilirkişi raporu · keşif · tanık beyanı · duruşma · ara karar
gerekçeli karar · istinaf · temyiz · Yargıtay · Bölge Adliye Mahkemesi
asliye hukuk · sulh hukuk · ticaret mahkemesi · iş mahkemesi · arabuluculuk
tahkim · hakem heyeti · ihtiyati tedbir · ihtiyati haciz · zamanaşımı
hak düşürücü süre · sözleşme · fesih · tazminat · maddi tazminat · manevi tazminat
cezai şart · temerrüt · muvazaa · ibraname · protokol · konkordato · iflas
alacak · borçlu · kefil · teminat mektubu · ipotek · rehin
hisse devir sözleşmesi · pay sahipliği · genel kurul · yönetim kurulu kararı · ticaret sicili
KVKK · aydınlatma metni · açık rıza · veri sorumlusu · veri işleyen · ihlal bildirimi
due diligence · compliance · NDA · term sheet · closing · escrow
arbitration · jurisdiction · governing law · force majeure
```

---

## 7. Eşikler ve verdict

Ölçümden **önce** `DECISIONS.md`'ye yazılacak (Aşama 0.4):

| Karar | Koşul |
|---|---|
| **GO** (tam kapsam, saha dahil) | medyan WER ≤ %15 (quiet) **ve** ≤ %25 (field) **ve** jargon terim-recall ≥ %80 (vocab tuning sonrası) |
| **CONDITIONAL** (yalnız toplantı odası) | quiet barı tutar, field tutmaz → quiet ortamlar için ship; saha ertelenir (yaka mikrofonu, VAD, proje-başı vocabulary ile yeniden dene) |
| **NO-GO** | tuning sonrası quiet bile kullanılamaz → temel STT uygun değil; feature yazmadan önce motor/yaklaşım yeniden değerlendirilir |

Türkçe için **diyakritik-katlanmış CER** ikincil metrik olarak raporlanır (harness zaten katı + katlanmış
WER/CER üretiyor). Türkçe sondan eklemeli olduğu için kelime-bazlı WER yapısal olarak yüksek çıkar;
CER bu yanlılığı görünür kılar — ama **bar CER'e göre gevşetilmez**, kararı bilgilendirir.

**İki domain ayrı raporlanır — artık harness bunu kendi yapıyor** (§3.3). Eşik tablosu her jargon
domaini için ayrı satır ve ayrı PASS/FAIL üretir; `report.json` içinde `medians_by_domain` ve
`jargon_domains` alanları bulunur. **GO koşulu: her domain eşiği geçmeli.** Bir domain geçer diğeri
geçmezse sonuç GO değildir — geçen domain için CONDITIONAL değerlendirilir, geçemeyen domain açık
kanıt borcu olarak kalır.

---

## 8. Fallback dalları (katılımcı erişimi belirsiz)

| Engel | Dal |
|---|---|
| 3-4 kişi bulunamıyor → `multi` doldurulamaz | `multi` kovası **açık kanıt borcu** olarak işaretlenir. Kapı `CONDITIONAL` üstü veremez. `quiet`+`jargon` tamamlanır, ürün kapsamı 1-2 konuşmacılı toplantıya daraltılır ve bu **ürün kopyasında** da böyle yazılır. Harness fail-closed olduğu için rapor üretilemez — verdict `report.md` olmadan, eksik-kova gerekçesiyle insan tarafından yazılır. |
| Gerçek saha erişimi yok → `field` doldurulamaz | Aynı: saha iddiası **yasak** kalır (ADR-0027 zaten saha/doğruluk iddiasını yasaklıyor). `CONDITIONAL(meeting-room)` hedeflenir. |
| Hukuk katılımcısı yok | `jl*` 5 klip ertelenir; `ji*` 5 ile jargon kovası kapatılır. **Sonuç:** hukuk beachhead'i için kanıt yok → strateji dikey kararı design-partner bulunana kadar askıda kalır. |
| `large-v3-turbo` eksik/bozuk | Turbo konfigleri atlanır, koşu devam eder. Karar `large-v3` + Parakeet üzerinden verilir. |

Her dal **kanıt borcu olarak yazılır**, sessizce kapsam daraltılmaz.

---

## 9. Bir AI ajanının bu sprintte yapamayacakları

- Ses kaydı toplayamaz, rıza alamaz, katılımcı yerine geçemez.
- Makine taslağını "referans" diye onaylayamaz — insan düzeltmesi kapının tanımıdır.
- Diyarizasyon sanity alanını dolduramaz, TTFT smoke testini yapamaz.
- **GO / CONDITIONAL / NO-GO imzalayamaz.** Metriği hesaplar, kararı insan verir.
- Başarısız bir WER'in üstünden geçip saha-bağımlı feature'lara giremez.

---

## 10. Çıkış kriterleri (kapı kapandı sayılır)

1. `eval/report.md` + `eval/report.json` üretildi; kova başına WER/CER, jargon terim-recall
   (iki domain ayrı), batch wall-clock + RTF içeriyor.
2. `verify-evidence.ps1` temiz; 25 satırlık `manifest.csv` dolu, hash'ler eşleşiyor.
3. Rapordaki diyarizasyon/çok-konuşmacı alanı bir **insan** tarafından dolduruldu.
4. Verdict + kilitli eşikler + önerilen STT konfigürasyonu (motor + vocabulary)
   `DECISIONS.md`'ye ADR olarak işlendi.
5. `BACKLOG.md` A5 satırı `DEFERRED` → gerçek sonuç (PASS / CONDITIONAL / FAIL) olarak güncellendi.
6. Ancak bundan sonra: C8 pilotu (immutable imzalı RC'ye karşı) ve ona bağlı EPIC D/F/G.
