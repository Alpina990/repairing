# Arxitektura qarorlari

## Maqsadlar

CheklaBot’ning birinchi versiyasi monolit deployment, lekin modular monolith kod tuzilmasi. Bu kichik jamoaga tez ishlash imkonini beradi va hali isbotlanmagan trafik uchun microservice murakkabligini kiritmaydi.

## Qatlamlar

### `qalqon-core`

Pure domain: `ChatSettings`, `Sanction`, anti-flood sliding window, content policy, welcome renderer va `ModerationStore` porti. Telegram yoki PostgreSQL type’lari bu crate ichiga kirmaydi.

### `qalqon-storage`

SQLx orqali PostgreSQL adapteri. Query macro o‘rniga runtime-checked query ishlatilgan, shuning uchun CI compile vaqtida live DB talab qilmaydi. Migratsiya binary ichiga embed qilinadi va startda idempotent bajariladi.

### `qalqon-bot`

`teloxide` update adapteri. Bu qatlam Telegram admin huquqlarini tekshiradi, xabarni o‘chiradi va ban/mute API chaqiruvlarini bajaradi. Command authorizatsiyasi har safar fresh; message moderationdagi admin bypass TTL cache bilan.

Startup avval PostgreSQL migratsiyasi va Telegram `getMe` tekshiruvini bajaradi.
DB ulanishida chegaralangan linear backoff bor. `/healthz` process tirikligini,
`/readyz` esa PostgreSQL mavjudligini bildiradi; `--doctor` DB va Telegramni birga
tekshiradi. Self-hosted yoki test Bot API uchun transport URL konfiguratsiya orqali
almashtiriladi.

## Muhim invariantlar

- Admin va botlar avtomatik jazolanmaydi.
- Manual moderation commandi fresh Telegram admin tekshiruvisiz bajarilmaydi.
- Message moderation uchun adminlar chat kesimida bitta API chaqiruvida olinadi va qisqa TTL bilan saqlanadi.
- Warning yozilishi va uning yangi sonini o‘qish bitta DB transaction ichida.
- Flood trigger bo‘lgach hisoblagich reset qilinadi; bitta burst ketma-ket ko‘p jazo bermaydi.
- Telegram amalining audit yozuvi muvaffaqiyatsiz bo‘lsa, amalga rollback qilinmaydi; logda ogohlantirish qoladi.
- Token faqat environment orqali olinadi.

## Scale yo‘li

1. Hozir: bitta long-polling instance + PostgreSQL + in-process cache.
2. Katta trafik: webhook + load balancer, update-id deduplication, Redis-backed rate limiter.
3. Multi-region: outbox/event bus, partitioned audit, regional Telegram workers.

Core portlari shu o‘zgarishlarda moderation qoidalarini o‘zgartirmaslik uchun ajratilgan.

## Keyingi modul kontrakti

Captcha, federation, analytics yoki ML spam detector qo‘shilganda yangi qoidalar `qalqon-core`da typed decision qaytaradi. Telegram adapter faqat decision’ni API amaliga aylantiradi. DB schema har doim yangi forward-only migratsiya bilan o‘zgaradi.
