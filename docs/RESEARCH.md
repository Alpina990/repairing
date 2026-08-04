# Open-source tahlil

Tahlil 2026-08-02 kuni shallow clone va upstream hujjatlari orqali bajarildi. `.research` yakuniy git tarixiga kirmaydi.

| Loyiha | Kuchli tomon | Cheklov | CheklaBot’dagi qaror |
|---|---|---|---|
| [SaitamaRobot](https://github.com/AnimeKaizoku/SaitamaRobot) | Keng moderation funksiyalari: warns, locks, antiflood, welcome, federation | 2022’dan archived, GPL-3.0, eski Python handlerlari va juda bog‘langan modullar | Funksiya UX’i o‘rganildi; kod ko‘chirilmadi |
| [Gojo Satoru](https://github.com/Gojo-Bots/Gojo_Satoru) | Plugin naming va katta command qamrovi | AGPL-3.0, Python global dispatcher side-effectlari | Module discovery g‘oyasi roadmapga olindi, kod olinmadi |
| [Marvin](https://github.com/SphericalKat/marvin) | Rust, MIT, SQLx/PostgreSQL, handler/repository ajratilishi | `teloxide 0.7`, 2022’dan qolgan; barcha commandlar bitta katta match’da | PostgreSQL va Rust tanlovi tasdiqlandi; API `teloxide 0.17`ga qayta qurildi |
| [rust-lang-uz/telegram](https://github.com/rust-lang-uz/telegram) | O‘zbekcha UX, zamonaviyroq dptree branchlari | Bitta community uchun hard-code qilingan, umumiy moderation storage yo‘q | O‘zbekcha default matnlar va typed dispatcher yo‘li olindi |
| [teloxide](https://github.com/teloxide/teloxide) | MIT, typed commands, dptree, polling/webhook, graceful shutdown | Framework; mahsulot qoidalarini bermaydi | Telegram adapterining asosiy framework’i |

## Clean-room va litsenziya

Saitama/Gojo kabi GPL/AGPL manbalardan source line yoki modul implementatsiyasi ko‘chirilmagan. Faqat foydalanuvchiga ko‘rinadigan umumiy moderation tushunchalari — warning limit, flood window, welcome placeholder va blocklist — talab sifatida qayta ifodalangan. CheklaBot kodi MIT litsenziyada yangidan yozilgan.

## Nega mavjud botni fork qilmadik

Eng ko‘p feature’li eski botlar Python va copyleft litsenziyada, ko‘pi archived yoki fork-of-fork holatida. Ularni Rustga satrma-satr port qilish eski coupling va schema qarzini ham ko‘chiradi. Marvin permissive va Rust’da, ammo juda eski framework API’si hamda kichik feature qamroviga ega. Shu sabab eng yaxshi variant — funksional tadqiqot + yangi clean architecture.

## MVPdan keyingi prioritetlar

1. Captcha/join-request verification va timeout worker.
2. Link/media/sticker/language locklar.
3. Federation va shared ban list — signed audit hamda appeal bilan.
4. Prometheus metrics, Sentry/OpenTelemetry va admin web-panel.
5. Redis distributed flood guard, webhook va update deduplication.
6. `uz`, `ru`, `en` i18n kataloglari.
