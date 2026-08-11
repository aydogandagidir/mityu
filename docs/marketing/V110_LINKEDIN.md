# v1.1.0 — LinkedIn duyuru paketi

**Durum:** hazır, ama §5'teki zamanlama riskini okumadan yayınlamayın.

Bu dosyanın her cümlesi ADR-0027 §2 ve ADR-0037'ye karşı denetlendi. O ADR'ler
ölçülmemiş doğruluk iddiasını, diarization kalite iddiasını, saha/endüstriyel
konumlandırmayı ve "pilotla kanıtlanmış değer" iddiasını **yasaklıyor** — ve bu
yasak v1.1.0 için de sürüyor.

---

## 1. Neden "özellik çıktı" postu yazmıyoruz

Konuşmacı ayrımı rakiplerin hepsinde var. "Biz de ekledik" demek bizi doğrudan
doğruluk kıyaslamasına sokar — ki A5 `multi` kovası boş olduğu için o kıyası
yapamayız ve yapmamamız gerekiyor.

Güçlü olan hikâye **eklemediklerimiz**. Rakipler talk-time'ı "kim baskın
konuştu" tablosuna, katılım skoruna ve duygu analizine çeviriyor. Biz üçünü de
reddettik ve bunlar kodda uygulanmış, testle korunan kararlar — pazarlama sözü
değil. Strateji belgesinin wedge'i de tam olarak bu.

---

## 2. Post — Türkçe (birincil)

> Mityu 1.1.0 çıktı. Artık bir kayıttaki sesleri birbirinden ayırıyor ve her
> birinin ne kadar konuştuğunu gösteriyor — tamamen sizin cihazınızda, ses
> hiçbir yere gitmeden.
>
> Ama asıl anlatmak istediğim, eklemediklerimiz.
>
> **Konuşma süresi bir sıralama değil.** Konuşmacılar ilk konuştukları sıraya
> göre listeleniyor, süreye göre değil. Çünkü "kim daha çok konuştu" tablosu bir
> toplantı özeti değildir; bir performans ölçümüdür. Biz o ürünü yapmıyoruz, ve
> bu bir slogan değil — kodda böyle yazılı, testle korunuyor.
>
> **Etiketler anonim kalıyor — ve isim verme özelliği yok.** "Konuşmacı 1",
> "Konuşmacı 2"; onları yeniden adlandıracak bir alanı bilerek koymadık. Hangi
> sesin kime ait olduğunu saklamak KVKK ve GDPR anlamında biyometrik veri
> olurdu.
>
> **Duygu, katılım ya da kişilik çıkarımı yok.** Olmayacak da. EU AI Act'in
> duygu tanıma yasağı 2 Ağustos 2026'da yürürlüğe girdi.
>
> Ve dürüst olalım: sonuç **en iyi çaba** tahmini. Sizinkine benzer kayıtlarda
> ne sıklıkla doğru bildiğini ölçmedik — ürünün içinde de aynen böyle yazıyor.
> Önemli bir şeye dayandırmadan önce kaydı dinleyin.
>
> Windows için: mityu.bluedev.dev

**Karakter:** ~1.180 (LinkedIn 3.000 sınırı; "daha fazla" kesmesi ~210'da, yani
ilk iki satır kancayı taşımalı — taşıyor).

---

## 3. Post — İngilizce

> Mityu 1.1.0 is out. It now tells voices apart in a recording and shows how
> long each one spoke — entirely on your own device. No audio leaves it.
>
> What I actually want to talk about is what we left out.
>
> **Talk time is not a ranking.** Speakers are listed in the order they first
> spoke, never by duration. A leaderboard of who talked most isn't a meeting
> summary, it's a performance review. We're not building that — and it isn't a
> slogan, it's how the code is written and what the tests enforce.
>
> **Labels stay anonymous — and there is no rename control.** "Speaker 1",
> "Speaker 2", with no field to change them, deliberately. Storing which voice
> belongs to whom would be biometric data under GDPR and Türkiye's KVKK.
>
> **No emotion, engagement or personality inference.** There won't be. The EU AI
> Act's emotion-recognition ban took effect on 2 August 2026.
>
> And honestly: it's a **best-effort** estimate. We haven't measured how often
> it gets speakers right on recordings like yours, and the product says exactly
> that. Check it against the audio before you rely on it.
>
> Windows: mityu.bluedev.dev

---

## 4. Kısa varyant (yorum / repost / X)

> Mityu 1.1.0: konuşmacı ayrımı, tamamen cihazda.
>
> Konuşma süresini kimin çok konuştuğuna göre sıralamıyoruz — o bir toplantı
> özeti değil, performans ölçümüdür. Etiketler anonim ve isim verme özelliği
> yok, çünkü saklanan ses→kişi eşlemesi biyometrik veri.
>
> En iyi çaba tahmini; doğruluğunu ölçmedik ve ürün bunu söylüyor.

---

## 5. ⚠️ Yayınlamadan önce bilmeniz gerekenler

**(a) Beklemenin süresi — en büyük risk.** Bir geçiş, kaydın yaklaşık dörtte
biri kadar sürüyor (ölçüldü: 60 dk → 14 dk 27 sn, 90 dk → 21 dk 21 sn). Şu an
arayüzde **ilerleme göstergesi ve iptal yok** — sadece "Analysing…" yazıyor
(BACKLOG H12). LinkedIn'den gelen bir kullanıcı dalgası, ilk denemesinde 15
dakika sessiz bekleyip uygulamanın donduğunu düşünebilir. Bu, ilk izlenimi
kalıcı olarak bozar.

*Tavsiyem:* ya H12'yi (ilerleme + iptal) önce bitirin, ya da postta süreyi
açıkça yazın. İkinci seçenek için hazır cümle:

> "Bir saatlik kayıt yaklaşık 15 dakikada işleniyor; arka planda çalışıyor."

**(b) Windows'a özel.** macOS geliştirme aşamasında. Postta yazılı; yorumlarda
mutlaka sorulacak.

**(c) SmartScreen uyarısı sürüyor.** Authenticode sertifikası hâlâ alınmadı
(ADR-0029). İndiren kişi bir uyarı ekranı görecek. Yorumlarda buna hazır olun.

**(d) Konuşmacı sayısı kararsız çıkabilir.** İki sentetik testte aynı malzemede
60 dakikada 4, 90 dakikada 2 konuşmacı raporlandı. "En iyi çaba" ifadesi tam da
bunun için var — ama biri "bende yanlış saydı" derse, bu beklenen bir sonuç ve
öyle karşılanmalı.

---

## 6. ⛔ Bu postta ya da yorumlarda ASLA söylenmeyecekler

ADR-0027 §2 ve §3 gereği, ihlali sürüm politikasını bozar:

| Söylemeyin | Neden |
|---|---|
| "%X doğrulukla ayırıyor", "WER", herhangi bir sayı | Ölçülmedi; A5 `multi` kovası boş |
| "Türkçede/jargonda test edildi" | A5 hiç değerlendirilmedi |
| "Gürültülü sahada / fabrikada çalışır" | Saha konumlandırması A5'e bağlı |
| "Pilot müşterilerimizde kanıtlandı" | C8 hiç yapılmadı |
| "Rakip X'ten daha doğru" | Kıyas için ölçüm yok |
| "Doğruluk garantisi / SLA" | Yasak |
| Duygu, katılım, kişilik, performans çıkarımı ima etmek | Etik sınır + EU AI Act |

**Her zaman bulunması gereken:** doğruluğun dile, mikrofona, üst üste konuşmaya
ve ortama göre değiştiği; kullanıcının çıktıyı kaynak sese karşı kontrol etmesi
gerektiği. Yukarıdaki iki postta da var — çıkarmayın.

---

## 7. Görsel

**Elimizde hazır olan:** `target/ui-shots/design-speakers.png` — gerçek
bileşenler, fixture veri. Üstünde "Design preview / Fixture data" yazıyor.

**Karar sizin:** o başlığı kırpıp panel görüntüsünü kullanmak, fixture veriyi
gerçek kayıt gibi göstermek olur. Polar ürün görsellerinde `/design/report`
render'ları kullanılmıştı, yani emsal var — ama orada da "gerçek uygulama
ekranı" olarak sunulmuştu ve bu sınırda bir tercih.

**Daha temiz iki seçenek:**
1. Gerçek bir kayıtta özelliği çalıştırıp uygulamadan ekran görüntüsü almak
   (en güçlüsü — gerçek veri, gerçek arayüz)
2. Görsel yerine metin postu + mityu.bluedev.dev link önizlemesi

**Not:** LinkedIn'de link önizlemesi olan postlar erişim kaybeder. Yaygın çözüm:
linki ilk yoruma koymak, postta sadece "mityu.bluedev.dev" yazmak.

---

## 8. Hashtag

Az ve konuyla ilgili olsun; LinkedIn 3–5'ten fazlasını cezalandırıyor.

`#KVKK #EUAIAct #OnDevice #PrivacyByDesign #MeetingNotes`

Türkçe post için `#KVKK` ve `#YapayZeka`; İngilizce için `#GDPR` ve `#EUAIAct`.

---

## 9. Yorumlara hazırlık

**"Doğruluğu nasıl?"**
> Ölçmedik ve bu yüzden bir sayı vermiyorum. Gerçek, rızalı çok-konuşmacılı
> kayıtlardan oluşan bir değerlendirme setimiz henüz yok; o olmadan söylenecek
> her rakam uydurma olur. Ürünün içinde de "en iyi çaba" yazıyor.

**"Konuşmacılara nasıl isim veriyorum?"**
> Vermiyorsunuz — Mityu'da öyle bir alan yok, ve bu bilinçli. Etiketler
> "Konuşmacı 1", "Konuşmacı 2" olarak kalıyor. Hangi sesin kime ait olduğunu
> saklamak KVKK m.6 ve GDPR Art. 9 anlamında biyometrik özel nitelikli veri
> olurdu; saklamamak işlemeyi o kategorinin dışında tutuyor. İsim gerekiyorsa
> kendi notunuza yazarsınız.

**"Talk-time yüzdeleri neyin yüzdesi?"**
> Konuşmanın, toplantı süresinin değil. İnsanlar üst üste konuştuğu için
> toplam konuşma süresi kaydın uzunluğunu aşabiliyor; toplantıya bölseydik
> yüzdeler %100'ü geçerdi.

**"macOS?"**
> Geliştirmede. Bugün Windows 10/11 64-bit.

**"Neden bu kadar uzun sürüyor?"**
> Tamamen cihazınızın işlemcisinde çalışıyor, buluta bir şey göndermiyor.
> Bir saatlik kayıt yaklaşık 15 dakika. [H12 bitmediyse ekleyin: "Şu an
> ilerleme göstergesi yok — üzerinde çalışıyoruz."]

---

## 10. İkinci post için saklanacak hikâye (bunu bu postla karıştırmayın)

Bu sürümde kendi ses bileşenimizin **GPL-3.0** lisanslı eSpeak NG kodunu statik
olarak bağladığını fark ettik — release derlemesinde 51 sembol — ve kapalı
kaynak bir installer'da dağıtmadan önce temizledik. Artık her derlemede üç ayrı
noktada otomatik denetleniyor.

Uyum konumlandırması yapan bir ürün için bu güçlü bir güvenilirlik hikâyesi,
ama teknik ve ayrı bir post hak ediyor. Bu postu ondan uzak tutun; iki mesaj
birbirini zayıflatır.
