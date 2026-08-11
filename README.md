# CheklaBot

CheklaBot — Telegram guruhlarini nazorat qilish uchun Rust’da yozilgan, modulli va production-oriented moderation bot.

Bu loyiha eski Rose/Saitama/Marie oilasidagi botlarning foydali moderation tajribasini o‘rgandi, lekin ularning GPL/AGPL kodini ko‘chirmaydi. Kod clean-room usulida, zamonaviy `teloxide`, PostgreSQL va aniq ajratilgan domain/storage/transport qatlamlari bilan yangidan yozilgan.

## Hozir ishlaydigan imkoniyatlar

- sliding-window anti-flood va `delete | warn | mute | ban` jazosi;
- Unicode-aware blocklist, avtomatik xabar o‘chirish va warning;
- warning limitiga yetganda avtomatik mute yoki ban;
- admin buyruqlari: warn, ban, unban, mute, unmute;
- guruh qoidalari va sozlanadigan welcome template;
- PostgreSQL migratsiyasi va moderation audit log;
- admin/policy TTL cache — har xabarda Telegram API yoki DB’ni urmaydi;
- startupda Telegram command menu’sini avtomatik ro‘yxatdan o‘tkazish;
- structured JSON loglar, graceful shutdown, Docker va CI.

## Arxitektura

```mermaid
flowchart LR
    TG["Telegram Bot API"] --> BOT["CheklaBot / qalqon-bot\ntransport + handlers"]
    BOT --> CORE["qalqon-core\npolicy + flood + ports"]
    BOT --> STORE["qalqon-storage\nSQLx adapter"]
    STORE --> PG[("PostgreSQL")]
    BOT --> CACHE["in-process TTL cache"]
```

- `qalqon-core` Telegram va SQLx’dan mustaqil; qoidalarni unit-test qilish oson.
- `qalqon-storage` `ModerationStore` portining PostgreSQL adapteri.
- `qalqon-bot` Telegram command/update adapteri va jazo ijrochisi.

Batafsil qarorlar: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md). O‘rganilgan loyihalar: [docs/RESEARCH.md](docs/RESEARCH.md).

Mini App API kontrakti, autentifikatsiya, barcha endpointlar va protection score
formulasi: [docs/MINI_APP_API.md](docs/MINI_APP_API.md).

Dependency audit va vaqtinchalik advisory qarorlari:
[docs/SECURITY_AUDIT.md](docs/SECURITY_AUDIT.md).

Pencil UX asosidagi React/TypeScript Mini App, lokal demo va Docker ishlatish
qo‘llanmasi: [frontend/README.md](frontend/README.md).

## Ishga tushirish

1. `@BotFather` orqali bot yarating. Guruh xabarlarini ko‘rishi va a’zolar indeksini yangilashi uchun botni guruhga admin qiling; kamida `Delete messages` va `Ban users` huquqlarini bering.
2. `.env.example`dan `.env` yarating, kuchli `POSTGRES_PASSWORD` va `TELOXIDE_TOKEN` kiriting.
3. Ishga tushiring:

```bash
docker compose up --build -d
docker compose logs -f bot
```

Bot DB migratsiyasini start vaqtida o‘zi bajaradi. Tokenni repoga commit qilmang.
Mini App a’zolar qidiruvini incoming message, `new_chat_members` va
`chat_member` hodisalaridan tuzilgan lokal indeks orqali bajaradi. Bot
qo‘shilishidan oldin hech qanday hodisasi ko‘rilmagan oddiy a’zo indeksga
birinchi xabari yoki membership yangilanishidan keyin tushadi; aniq Telegram ID
esa qidiruv vaqtida Telegram orqali tekshiriladi.

Ishga tushirishdan oldingi tezkor diagnostika:

```bash
docker compose run --rm bot --doctor
```

Ishlayotgan instance `GET /healthz` (process liveness) va `GET /readyz`
(PostgreSQL readiness) endpointlarini `HEALTH_ADDR` manzilida beradi. Container
healthcheck aynan readiness endpointini tekshiradi.

## Asosiy buyruqlar

Admin buyruqlarida userga tegishli xabarga reply qilish talab qilinadi.

```text
/settings
/setflood 8 10 mute
/setwarnlimit 3
/warn sabab
/mute 2h
/ban
/unban 123456789
/addblock reklama iborasi
/rmblock reklama iborasi
/blocklist
/setrules guruh qoidalari
/welcome on
/setwelcome Salom {first_name}, {chat_title}ga xush kelibsiz!
```

Duration birliklari: `m`, `h`, `d`, `w`. Welcome placeholderlari: `{first_name}`, `{username}`, `{user_id}`, `{chat_title}`.

## Mahalliy development

Rust 1.85+ va PostgreSQL kerak.

```bash
cp .env.example .env
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo run -p qalqon-bot
```

PostgreSQL integration testlarini ham ishlatish uchun alohida test bazasi URL’ini
bering:

```bash
TEST_DATABASE_URL=postgres://postgres:postgres@localhost:5432/postgres \
  cargo test --workspace --all-targets
```

`TELEGRAM_API_URL` faqat self-hosted Bot API yoki lokal integration test serveri
uchun kerak; odatiy Telegram ishlatishda uni sozlamang. DB vaqtincha tayyor
bo‘lmasa, bot konfiguratsiyadagi urinish/backoff chegarasigacha qayta ulanadi.

## Production eslatmalari

- Bir instance uchun long polling yetarli. Horizontal scalingdan oldin webhook ingress, Redis rate-limit va distributed deduplication qo‘shiladi.
- Telegram admin huquqlari commandda har safar fresh tekshiriladi; oddiy message moderation uchun 5 daqiqalik cache ishlatiladi.
- `OWNER_IDS` emergency override hisoblanadi; imkon qadar bo‘sh qoldiring yoki juda cheklangan saqlang.
- PostgreSQL backup va token rotation operator zimmasida.

## Litsenziya

MIT. Tahlil qilingan GPL/AGPL botlardan kod nusxalanmagan.
