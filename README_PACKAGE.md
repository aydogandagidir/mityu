# Meetily Enterprise — Claude Code Geliştirme Paketi (Kullanım Kılavuzu)

Bu paket, `Zackriya-Solutions/meetily` (MIT) tabanında **enterprise, local-first** bir masaüstü uygulaması geliştirmen ve bunu ileride **multi-tenant SaaS**'a dönüştürebilmen için Claude Code ajanlarının anlayacağı biçimde hazırlanmış bir "geliştirme anayasası + ajan ekibi + iş akışları + mimari doküman" setidir.

> Not: Ajanlara ve Claude Code'a hitap eden dosyalar (`CLAUDE.md`, `.claude/`, `docs/`) bilinçli olarak **İngilizce** yazıldı — bunlar teknik doküman ve ajan promptu; hem taşınabilirlik hem de upstream CLAUDE.md ile tutarlılık için. Bu kılavuz (senin için) Türkçe.

## Repoyu gerçekten inceleyerek çıkarılan temel gerçekler
- Desteklenen uygulama **Tauri 2 (Rust) + Next.js** masaüstü uygulamasıdır. Ses yakalama, transkripsiyon (**whisper.cpp + Parakeet**, Rust engine'leri) ve özetleme (Rust provider client'ları, **BYOK**) hep Rust çekirdeğinde; UI Next.js.
- **DB = SQLite** (local-first). LLM sağlayıcıları: OpenAI, Anthropic/Claude, Groq, Ollama, OpenRouter.
- **Python/FastAPI `backend/` klasörü LEGACY/arşiv** (kimlik doğrulamasız CORS). Ekibin kendi CLAUDE.md'si "production'da kullanma, bağımlılık olarak diriltme" diyor. Biz de sadece referans olarak kullanıyoruz.
- Lisans **MIT** (whisper.cpp submodule dahil) → ticari kullanım serbest; sadece "Meetily" markasını kullanma, kendi markanı koy. STT model ve LLM API şartları koddan ayrıdır, ayrıca doğrula.

## Tasarımın özü (neden böyle?)
1. **Local-first üründür, özellik değil.** Uygulama internetsiz ve server'sız tam çalışmalı. Bu bizim tüketici-bulut rakiplerinden (Otter/Fireflies) ayrışma noktamız.
2. **Server opsiyonel ve eklemeli.** Takım/enterprise/SaaS yetenekleri, sonradan devreye giren **yeni ve temiz bir sync/collaboration server** ile gelir; legacy backend diriltilmez.
3. **İlk günden tenant-aware, ama pratikte tek-tenant.** Her kalıcı veri `workspace_id`/`tenant_id` taşır. Böylece SaaS'a geçiş bir yeniden-yazım değil, **eklemeli** olur.
4. **Her AI çıktısı insan onaylı + kaynağa bağlı** (transkript segmenti + timestamp). Güven, claim kanıt değeri ve EU AI Act Madde 50 (2 Ağustos 2026'da yürürlükte) için zorunlu.

## Paketin içindekiler
```
.
├── CLAUDE.md                  # Proje anayasası (ajanların uyduğu ana kural seti)
├── BOOTSTRAP.md               # Claude Code'a verilecek SIRALI başlangıç promptları + gate'ler
├── README_PACKAGE.md          # (bu dosya) Türkçe kullanım kılavuzu
├── .claude/
│   ├── settings.json          # İzinler + hook bağlama
│   ├── agents/                # 8 uzman subagent
│   ├── commands/              # 10 slash-command (phase0-validate dahil)
│   └── hooks/                 # 4 otomatik koruma (secret, tenant-scope, rust, ts)
├── .github/workflows/
│   └── ci.yml                 # Kalite kapıları: cargo fmt/clippy/test, pnpm lint/tsc, cross-tenant test
└── docs/
    ├── ARCHITECTURE.md        # Hedef mimari + fazlar + Mermaid
    ├── MULTITENANCY.md        # Çok-kiracılılık oyun kitabı (RLS, 5 kural, negatif test)
    ├── DATA_MODEL.md          # SQLite(client) ↔ Postgres(server) tek mantıksal model
    ├── CONTRACTS.md           # Somut arayüzler/kod iskeleti (AuthContext, Repository, LlmProvider, sync)
    ├── SCAFFOLD.md            # Eklenecek kodun birebir dizin/modül yerleşimi
    ├── BACKLOG.md             # SIRALI, uygulanabilir görev listesi (epic→task, kabul kriteri, gate)
    ├── PHASE0_VALIDATION.md   # Transkripsiyon doğrulama protokolü (çalıştırılabilir harness + go/no-go)
    ├── SETUP.md               # Dev-ortam ön koşulları + komutlar
    ├── CONVENTIONS.md         # Rust/TS/test/commit standartları
    ├── SECURITY_PRIVACY.md    # Şifreleme, OWASP LLM, KVKK/GDPR/AI Act
    ├── ROADMAP.md             # Faz 0-3 + 30/60/90 gün
    └── DECISIONS.md           # ADR günlüğü
```

## Kurulum (fork'una yerleştirme)
1. Meetily'yi fork'la ve klonla; submodule'leri al: `git submodule update --init --recursive`.
2. Bu paketteki dosyaları fork'unun köküne kopyala. **Önemli:** repoda zaten bir `CLAUDE.md` var — bu paketteki, onun bir **üst kümesi**. Ya üzerine yaz (paketteki upstream'e referans veriyor), ya da paketteki içeriği mevcut dosyanın başına ekle.
3. Ürün adı **Mityu** olarak ayarlandı (çalışma adı). Bundle identifier `com.bluedev.mityu`. Rebrand'ın uygulanacağı dosyalar ve değerler için `docs/ROADMAP.md` Phase 0'a bak. Marka adını kamuya açık lansmandan önce finalize et.
4. Hook'ları çalıştırılabilir yap: `chmod +x .claude/hooks/*.sh`.
5. Claude Code'u repo kökünde aç. `.claude/agents/` ve `.claude/commands/` otomatik tanınır.
6. **Tek giriş noktası: `BOOTSTRAP.md`.** İlk oturumda Claude Code'a şunu söyle: *"Follow BOOTSTRAP.md step by step. Do not skip any gate."* BOOTSTRAP; orient → environment → rebrand → decisions → Phase 0 doğrulama → seam'ler → backlog sırasını gate'lerle yürütür.

## İş akışı (paket bunu kendisi yönetir — senin kararına bırakılmaz)
- **`BOOTSTRAP.md`** sıralı başlangıcı, **`docs/BACKLOG.md`** ise ondan sonrasını epic→task olarak, kabul kriterleri ve gate'lerle tanımlar. Claude Code bu listeyi yukarıdan aşağıya işler; sırayı kendi kafasına göre değiştirmez.
- İki **atlanamaz insan kapısı** pakette açıkça işaretli: (1) **Phase 0 transkripsiyon doğrulaması** (`docs/PHASE0_VALIDATION.md` — kendi sesinle WER ölçümü, go/no-go), (2) **production öncesi güvenlik/hukuk denetimi** (`/security-review` + uzman). Bunlar "opsiyonel" değil; ajan bunları geçmeden ileri gidemez.

## Günlük kullanım örnekleri
- Yeni özellik: `/feature toplantı özetini PDF olarak dışa aktar`
- Server hazırlığı: `/prep-multitenant meetings storage` (tek-tenant kalır ama tenant-aware olur)
- İzolasyon denetimi: `/tenant-check`
- Güvenlik denetimi (release öncesi): `/security-review`
- Şema değişikliği: `/db-migration action_item tablosuna assignee ekle`
- Ses sorunu: `/audio-debug Windows'ta system audio gelmiyor`
- Sürüm: `/release 0.2.0`

## Kırmızı çizgiler (paket bunları ajanlara zorlar)
- Varsayılan olarak ham ses/transkripti üçüncü tarafa gönderme.
- Legacy `backend/`'i production bağımlılığı yapma.
- Tenant verisinde tenant-scope'suz sorgu, cross-tenant erişim, god-mode yok.
- LLM anahtarını SQLite'ta düz metin tutma / commit'leme.
- İnsan onayı + kaynak bağı olmadan AI çıktısı yayınlama.
- Audio pipeline ile DB şemasını aynı değişiklikte elleme.

## Sınırlar / dürüst notlar
- Hook'lar birinci-savunma katmanıdır (heuristik); güvenlik/tenant izolasyonunu **kanıtlamaz**. Asıl kanıt: Postgres RLS + repository katmanı + zorunlu cross-tenant negatif test (`sync-server-architect` ve `qa-release-engineer` bunu şart koşar).
- Multi-tenant server henüz yazılmadı; paket onu **doğru şekilde başlatman** için seam'leri ve kuralları veriyor. Server dili kararı `DECISIONS.md`'de senin.
- KVKK/GDPR/AI Act bölümleri teknik/operasyonel rehberdir, **hukuki görüş değildir**; saha kaydı için işçi/sendika hukuku ve biyometrik eşik konularında uzman doğrulaması gerekir.
