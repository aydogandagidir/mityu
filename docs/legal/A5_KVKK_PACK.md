# A5 Kayıt Toplaması — KVKK Uyum Analizi ve Metin Taslakları

> **BU BELGE HUKUKİ MÜTALAA DEĞİLDİR.** Bir geliştirici tarafından, uygulamanın
> gerçek veri akışları okunarak hazırlanmış bir **ön analiz ve taslak paketidir**.
> Amacı avukatın işini bitirmek değil, avukata "sıfırdan yaz" yerine "bunu
> denetle" diyebilmeni sağlamak.
>
> **§5'teki taslaklar avukat onayı almadan hiçbir katılımcıya gösterilemez.**
> Her taslağın başında kaldırılması gereken bir uyarı bloğu var; o blok
> duruyorsa metin onaylanmamış demektir.
>
> Hazırlayan: Claude (geliştirici asistanı) · Tarih: 2026-08-07 · Durum: **AVUKAT İNCELEMESİ BEKLİYOR**

---

## 1. Neden bu paket var

`docs/A5_SPRINT.md` Aşama 0.5, 25 gerçek-ses kaydı toplanmadan önce KVKK
aydınlatma ve açık rıza metinlerinin hazır olmasını şart koşuyor. Bu, sprintin
**tek kalan blokajı** — geri kalan her şey (harness, modeller, kanıt araçları)
hazır.

`docs/LEGAL_SIGNOFF_V1.0.4.md` zaten kayda geçirmiş: bağımsız hukuki inceleme
**alınmadı**, v1.0.4 öz-beyanla yayınlandı, ve "katılımcı-rıza tahsisi" ile
"KVKK bildiriminin avukatça güncellenmesi" açık kalemler olarak duruyor
(satır 84 ve 106-e).

**Buradaki risk sınıfı farklı, ve bu ayrım bu paketin varlık sebebi.** Bir ToS'u
öz-beyanla yayınlamanın riski esas olarak şirkete düşer: sözleşme hükmü
uygulanamaz çıkar, tüketici mevzuatı bakımından eleştirilir. Üçüncü kişilerin
sesini **hatalı rızayla kaydetmenin** riski ise kayıtta sesi bulunan kişilere
düşer ve şirket için idari para cezasının yanında **TCK 135** (kişisel verileri
hukuka aykırı olarak kaydetme) alanına girer. Aynı "kabul edilen artık risk"
kararının bu yükü taşıdığı varsayılmamalı.

---

## 2. En kritik ayrım: iki farklı hukuki bağlam

Bunlar karıştırılırsa hem gereksiz iş yapılır hem de asıl boşluk gözden kaçar.

| | **A. Ürün çalışma zamanı** | **B. A5 eval toplaması** |
|---|---|---|
| Kaydı kim yapıyor | Uygulamayı kullanan müşteri | **Bluedev / sen** |
| Veri sorumlusu | **Kullanıcı** (Bluedev veriye hiç erişmiyor) | **Blue Robot Teknolojileri Ltd. Şti.** |
| Bluedev'in rolü | Veri sorumlusu **değil** — ne veriyi görüyor ne saklıyor | Veri sorumlusunun ta kendisi |
| Gereken metin | Kullanıcıya "katılımcı rızası senin sorumluluğunda" uyarısı | **Aydınlatma + açık rıza metinleri** |
| Bugünkü durum | `RecordingConsentDialog.tsx` bunu yapıyor | **Yok — bu paketin konusu** |

**Bağlam A doğru kurgulanmış.** Local-first mimaride kayıt cihazda kalıyor ve
Bluedev'e ulaşmıyor; dolayısıyla o kayıtların veri sorumlusu kullanıcıdır.
Uygulamanın görevi kendisini veri sorumlusu konumuna sokmamak ve kullanıcıyı
uyarmaktır — mevcut diyalog tam olarak bunu yapıyor. `PRIVACY_POLICY.md:9` da
bunu doğru ifade ediyor: *"Because Mityu is local-first, the controller has no
access to your meeting content."*

**Bağlam B'de metin yok.** Ve A5 kayıtlarında sesi kaydedilen kişiler bakımından
veri sorumlusu doğrudan sensin.

---

## 3. Uyum matrisi — KVKK yükümlülüğü × bugünkü durum (Bağlam B)

Aşağıdaki "bugünkü durum" sütunu kodu ve dokümanı okuyarak doğrulandı; iddia
değil, tespit.

| # | KVKK yükümlülüğü | Bugünkü durum | Boşluk |
|---|---|---|---|
| 1 | **Aydınlatma yükümlülüğü** (m.10) — kimlik, amaç, aktarım, yöntem/hukuki sebep, m.11 hakları | Metin **yok** | §5.1 taslağı |
| 2 | **Açık rıza** (m.3/1-a; m.6 özel nitelikliyse) — belirli konuya ilişkin, bilgilendirmeye dayalı, özgür irade | Metin **yok** | §5.2 taslağı |
| 3 | **Aydınlatma ve açık rızanın AYRI belgeler olması** (Kurul 18.02.2026 t. 2026/347 ilke kararı) | — | Taslaklar ayrı hazırlandı ✅ |
| 4 | **Rızanın geri alınabilirliği** (m.7, m.11) | Manifestte geri-alma alanı **yok** | §6.2 |
| 5 | **Saklama süresi ve imha** | Manifestte `retention_delete_by` sütunu **var** ✅ | Süre belirlenmedi (§4.3) |
| 6 | **Veri minimizasyonu** (m.4) | Manifest **opak** `consent_evidence_id` kullanıyor; ad/imza repo dışında ✅ | — |
| 7 | **Veri güvenliği** (m.12) | Kimlik verisi repo dışı, erişim kontrollü kasada; `.gitignore` kanıt dosyalarını dışlıyor ✅ | Kasanın kendisi tanımlanmalı |
| 8 | **VERBİS kaydı** | **Bilinmiyor** | Avukat kararı (§4.4) |
| 9 | **Özel nitelikli veri sınıflandırması** | **Belirsiz** — belirleyici (§4.1) | Avukat kararı |
| 10 | **Çalışan katılımcıda özgür irade** | Plan "gönüllü yetişkin" diyor | §4.2 — bağlayıcı hale getirilmeli |

**Değerlendirme:** altyapı tarafı beklediğimden iyi. `manifest.template.csv`
zaten `permission_confirmed`, `consent_evidence_id`, `notice_version`,
`retention_delete_by` sütunlarını taşıyor ve `eval/evidence/README.md`
doğrulayıcının *"permission'ın hukuken geçerli olup olmadığını belirleyemez"*
olduğunu açıkça söylüyor. Yani kanıt iskeleti doğru tasarlanmış; eksik olan
**metinlerin kendisi** ve birkaç karar.

> ⚠️ `notice_version` sütunu, sürümlenmiş bir aydınlatma metnine işaret etmek
> için var — ama işaret edebileceği bir belge yok. §5.1 onaylandığında
> `A5-AYDINLATMA-v1.0` gibi bir sürüm etiketi alıp bu sütuna yazılmalı.

---

## 4. Avukatın karar vermesi gereken dört konu

Bunlar benim veremeyeceğim kararlar; her birinin sonucu operasyonu değiştiriyor.

### 4.1 Ses kaydı özel nitelikli (biyometrik) veri mi? — **en belirleyici soru**

İki okuma var:

- **Dar okuma (muhtemelen doğru):** KVKK'nın biyometrik veri rehberi, biyometrik
  veriyi *kişiyi tekil olarak tanımlama amacıyla* işlenen fiziksel/davranışsal
  özellik olarak tanımlar. A5'te amaç transkripsiyon doğruluğunu ölçmek;
  konuşmacı tanıma **yapılmıyor** — Mityu bunu ürün genelinde bilinçli olarak
  yapmıyor (konuşmacı adlandırma elle, duygu/kimlik çıkarımı yasak). Bu okumaya
  göre veri m.5 kapsamında **olağan kişisel veridir**.
- **Geniş/temkinli okuma:** ses doğası gereği tanımlayıcıdır ve kayıt sonradan
  tanımlama için kullanılabilir; ayrıca konuşulan içerik arızî olarak özel
  nitelikli veri (sağlık, sendika üyeliği vb.) barındırabilir.

**Pratik sonuç:** hangi okuma benimsenirse benimsensin **açık rıza almak her
ikisini de karşılar** (m.5/1 ve m.6/2 için geçerli sebep). Bu yüzden taslaklar
açık rıza üzerine kurulu. Ancak sınıflandırma yine de önemli: özel nitelikli
sayılırsa **biyometrik veri rehberinin ek yükümlülükleri** (özel teknik tedbirler,
gerekiyorsa etki değerlendirmesi) devreye girer.

### 4.2 Katılımcılar çalışan/stajyer/bağlı kişi olacak mı?

Olacaksa **rızanın "özgür irade" unsuru tartışmalı hale gelir** — EDPB'nin
2020/05 rehberi ve KVKK'nın paralel yaklaşımı, işveren-çalışan arasındaki güç
dengesizliğinde rızayı geçersiz sayabilir. Avukat ya (a) katılımcıların bağlı
olmamasını şart koşmalı, ya (b) farklı bir hukuki sebep kurgulamalı, ya da
(c) reddetmenin gerçekten sonuçsuz olduğunu gösteren ek tedbirler tanımlamalı.

### 4.3 Saklama süresi ne olacak?

`retention_delete_by` sütunu var ama süre yok. Ses ve referans transkript ne
kadar tutulacak? Öneri niteliğinde bir çapa: değerlendirme tekrarlanabilirliği
için makul bir süre (ör. verdict + 12 ay), sonunda imha. **Süreyi avukat
belirlemeli** ve metne yazılmalı — süresiz saklama beyanı kabul edilemez.

### 4.4 VERBİS kaydı gerekiyor mu?

Blue Robot Teknolojileri Ltd. Şti. için VERBİS kayıt yükümlülüğü doğuyor mu
(çalışan sayısı/yıllık mali bilanço eşikleri ve faaliyet niteliğine göre)?
Gerekiyorsa kayıt, veri toplamadan önce yapılmalı.

---

## 5. Metin taslakları

> Aşağıdaki iki metin **Kurul'un 18.02.2026 tarihli 2026/347 sayılı ilke kararı**
> gereği bilinçli olarak **ayrı belgeler** hâlinde hazırlanmıştır. Birleştirilmemeli;
> açık rıza, aydınlatmanın içine gömülmemelidir.
>
> Metinler Türkçedir çünkü katılımcılar Türkçe konuşacaktır — aydınlatmanın
> muhatabın anlayacağı dilde olması gerekir.

### 5.1 Taslak — Aydınlatma Metni

```
┌──────────────────────────────────────────────────────────────────────┐
│ ⚠️ ONAYLANMAMIŞ TASLAK — AVUKAT İNCELEMESİNDEN GEÇMEDİ.              │
│ Bu kutu duruyorsa metin kullanılamaz. Avukat onayladıktan sonra bu    │
│ kutuyu kaldır ve belgeye sürüm etiketi ver (ör. A5-AYDINLATMA-v1.0).  │
└──────────────────────────────────────────────────────────────────────┘

SES KAYDI ALINMASINA İLİŞKİN AYDINLATMA METNİ

1) Veri sorumlusunun kimliği
Blue Robot Teknolojileri ve Ticaret Limited Şirketi ("bluedev")
İçerenköy Mah. Topçu İbrahim Sk. Quick Tower Sitesi No: 8-10d, Ataşehir/İstanbul
MERSİS: 0178185796600001 · VKN: 1781857966 · E-posta: info@bluedev.dev

2) İşlenen kişisel veriler
- Ses kaydınız (konuşmanız),
- Ses kaydından üretilen ve bir kişi tarafından düzeltilen yazılı metin
  (transkript),
- Kaydın süresi, ortamı, konuşmacı sayısı ve dili gibi teknik kayıt bilgileri.

Kaydın içeriğinde kimliğinizi belirtmeniz gerekmez ve bu talep edilmez.
Kayıtlar kurgusal senaryolar üzerinden yapılır; gerçek müşteri görüşmesi,
gizli iş konuşması veya kişisel bilgilerinizin paylaşıldığı bir içerik
kaydedilmez.

3) İşleme amacı
Kişisel verileriniz, bluedev'in geliştirdiği Mityu adlı masaüstü yazılımının
konuşmayı yazıya dönüştürme (transkripsiyon) doğruluğunun ölçülmesi ve
iyileştirilmesi amacıyla işlenir. Kayıtlarınız reklam, pazarlama, profilleme
veya sizi tanımlama amacıyla KULLANILMAZ.

4) Hukuki sebep ve toplama yöntemi
Kişisel verileriniz, tarafınızca ayrıca ve açıkça verilen AÇIK RIZANIZA
dayanılarak (KVKK m.5/1 ve ilgili olduğu ölçüde m.6/2) işlenir. Veriler,
kayıt sırasında kullanılan cihazın mikrofonu aracılığıyla doğrudan sizden
elektronik ortamda toplanır.

5) Aktarım
Kişisel verileriniz yurt içinde veya yurt dışında ÜÇÜNCÜ KİŞİLERE
AKTARILMAZ. Ses kaydı ve transkript, değerlendirmeyi yürüten cihazda ve
bluedev'in erişim kontrollü ortamında tutulur; bulut hizmetlerine
yüklenmez, yapay zekâ sağlayıcılarına gönderilmez, yayımlanmaz ve herhangi
bir kamuya açık veri kümesine dâhil edilmez.
[AVUKAT: Yalnızca hukuken zorunlu hâller — örneğin yetkili kamu kurumu
talebi — için istisna ifadesi eklenip eklenmeyeceğine karar verilecek.]

6) Saklama süresi
Kişisel verileriniz, yukarıdaki amaç için gerekli olan süre boyunca ve en geç
[AVUKAT BELİRLEYECEK — bkz. §4.3] tarihine kadar saklanır; bu sürenin sonunda
silinir veya yok edilir.

7) Haklarınız (KVKK m.11)
Veri sorumlusuna başvurarak; kişisel verinizin işlenip işlenmediğini öğrenme,
işlenmişse buna ilişkin bilgi talep etme, işlenme amacını ve amacına uygun
kullanılıp kullanılmadığını öğrenme, eksik veya yanlış işlenmişse düzeltilmesini
isteme, silinmesini veya yok edilmesini isteme, bu işlemlerin verilerin
aktarıldığı üçüncü kişilere bildirilmesini isteme, münhasıran otomatik
sistemlerle analiz edilmesi suretiyle aleyhinize bir sonucun ortaya çıkmasına
itiraz etme ve kanuna aykırı işleme sebebiyle zarara uğramanız hâlinde zararın
giderilmesini talep etme haklarına sahipsiniz.

Taleplerinizi info@bluedev.dev adresine iletebilirsiniz.

8) Rızanızı geri alma
Verdiğiniz açık rızayı DİLEDİĞİNİZ ZAMAN, gerekçe göstermeksizin geri
alabilirsiniz. Geri alma hâlinde kaydınız değerlendirmeden çıkarılır ve silinir.
Geri alma, geri alma tarihinden önceki hukuka uygun işlemeyi etkilemez.
Talebiniz için: info@bluedev.dev

9) Katılımın gönüllülüğü
Kayda katılım tamamen gönüllüdür. Katılmamanız veya sonradan vazgeçmeniz
hâlinde herhangi bir olumsuz sonuçla karşılaşmazsınız.
```

### 5.2 Taslak — Açık Rıza Metni (AYRI BELGE)

```
┌──────────────────────────────────────────────────────────────────────┐
│ ⚠️ ONAYLANMAMIŞ TASLAK — AVUKAT İNCELEMESİNDEN GEÇMEDİ.              │
│ Bu kutu duruyorsa metin kullanılamaz. Aydınlatma metniyle aynı        │
│ sayfaya BASILMAMALI; ayrı belge olarak imzalatılmalıdır.              │
└──────────────────────────────────────────────────────────────────────┘

AÇIK RIZA BEYANI

Blue Robot Teknolojileri ve Ticaret Limited Şirketi tarafından hazırlanan
"Ses Kaydı Alınmasına İlişkin Aydınlatma Metni"ni [sürüm: ..............]
okudum ve anladım.

Bu kapsamda; ses kaydımın ve bu kayıttan üretilecek yazılı metnin, Mityu adlı
yazılımın konuşmayı yazıya dönüştürme doğruluğunun ölçülmesi ve iyileştirilmesi
amacıyla, aydınlatma metninde belirtilen süre boyunca işlenmesine

☐ AÇIK RIZA VERİYORUM        ☐ AÇIK RIZA VERMİYORUM

Rızamı dilediğim zaman gerekçe göstermeksizin geri alabileceğimi, geri almam
hâlinde kaydımın silineceğini ve katılmamanın benim için herhangi bir olumsuz
sonuç doğurmayacağını biliyorum.

Ad Soyad : ...........................................
Tarih    : ...........................................
İmza     : ...........................................

[Bu belge repoya KONULMAZ. Erişim kontrollü kasada saklanır; manifeste yalnızca
opak consent_evidence_id yazılır.]
```

---

## 6. Metinlerin ötesinde kapatılması gereken operasyonel boşluklar

### 6.1 `notice_version` işaret edecek bir belge kazanmalı
Aydınlatma onaylandığında sürüm etiketi alır (`A5-AYDINLATMA-v1.0`) ve her
manifest satırına o katılımcının imzaladığı **sürüm** yazılır. Metin sonradan
değişirse eski kayıtlar eski sürümle işaretli kalır — bu, sonradan "hangi metni
imzaladı" sorusunu cevaplanabilir kılan şeydir.

### 6.2 Geri alma mekanizması manifestte yok
Manifest rızanın **alındığını** kaydediyor ama **geri alındığını** kaydedecek
bir alan yok. Bir katılımcı rızasını geri alırsa: kaydı silinmeli, manifest
satırı geri-alma tarihiyle işaretlenmeli ve bulunduğu kova 5 klibin altına
düşerse **kapı fail-closed durmalı** — ki bu doğru davranıştır, sessizce 4
klipli bir kovayla ölçüm yapılmamalıdır. Öneri: `consent_withdrawn_at_utc`
sütunu ve doğrulayıcıda karşılığı.

### 6.3 Kanıt kasası tanımlı değil
`eval/evidence/README.md` "erişim kontrollü bir yerde sakla" diyor ama yer
tanımlı değil. Somut olarak neresi olduğu, kimin eriştiği ve nasıl yedeklendiği
yazılmalı.

### 6.4 Ürün tarafında bir gözlem (Bağlam A)
`RecordingConsentDialog` yalnızca İngilizce ve *"Don't show this again on this
device"* seçeneği var. Bu bir KVKK aydınlatması olmadığı için doğrudan bir ihlal
değil; ancak Türkiye pazarına satış yapılacaksa (strateji beachhead'i Türkiye
odaklı) hem Türkçe hem de "bir kez onayla, hep geçerli" davranışının kullanıcıya
doğru anlatılması avukatla gözden geçirilmelidir. Bu, A5'i bloke etmez.

---

## 7. Kayıt başlamadan önce sağlanması gereken kontrol listesi

1. ☐ §4.1–4.4 kararları avukat tarafından verildi
2. ☐ §5.1 aydınlatma metni onaylandı, uyarı kutusu kaldırıldı, sürüm etiketi verildi
3. ☐ §5.2 açık rıza metni onaylandı ve **ayrı belge** olarak basıldı
4. ☐ Saklama süresi metne yazıldı
5. ☐ VERBİS yükümlülüğü netleşti, gerekiyorsa kayıt yapıldı
6. ☐ Kanıt kasası tanımlandı ve erişim sınırlandı
7. ☐ `consent_withdrawn_at_utc` alanı ve doğrulayıcı desteği eklendi (§6.2)
8. ☐ Katılımcıların bağlı çalışan olmadığı teyit edildi (veya §4.2 çözümü uygulandı)

**Bu listenin tamamı işaretlenmeden hiçbir kayıt alınmamalıdır.**

---

## 8. Bu belgenin doğrulamadığı ve doğrulayamayacağı şeyler

Oturum boyunca uyguladığım kurala sadık kalarak, neyi kanıtlamadığımı açıkça
yazıyorum:

- **Metinlerin hukuki geçerliliğini doğrulamadım.** Bunlar taslaktır; KVKK m.10
  unsurlarını kapsayacak şekilde yapılandırıldılar, ancak bir avukatın
  değerlendirmesinin yerine geçmezler.
- **Özel nitelikli veri sınıflandırmasını yapmadım** (§4.1). İki okumayı ve
  pratik sonucunu sundum; kararı vermedim.
- **VERBİS yükümlülüğünü belirlemedim** (§4.4).
- **Saklama süresini belirlemedim** (§4.3) — metinde bilinçli olarak boş bırakıldı.
- **"Kusursuz altyapı" teyidi vermiyorum.** Uyum matrisindeki ✅ işaretleri,
  kodda ve dokümanda **doğruladığım teknik durumları** gösterir; hukuki
  yeterlilik beyanı değildir. Teknik olarak doğru kurulmuş bir kanıt zinciri,
  hukuken geçersiz bir rızayı geçerli hâle getirmez.
- **Öz-beyan yolunun bu bağlam için uygun olduğunu teyit etmiyorum.** §1'de
  açıkladığım gibi buradaki risk üçüncü kişilere düşüyor ve TCK 135 alanına
  giriyor; bu, v1.0.4 ToS'u için kabul edilen artık riskle aynı sınıf değildir.
