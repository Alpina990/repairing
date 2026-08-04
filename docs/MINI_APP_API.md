# CheklaBot Mini App API

API default holatda `0.0.0.0:8081` da ishlaydi. Docker Compose lokal test uchun
uni `http://127.0.0.1:8081` manziliga chiqaradi. Telegram Mini App production
deploymenti HTTPS reverse proxy talab qiladi.

## Autentifikatsiya

Frontend Telegram bergan **raw** `Telegram.WebApp.initData` qiymatini har bir
requestda yuboradi:

```http
Authorization: tma query_id=...&user=...&auth_date=...&hash=...
```

Muqobil header: `X-Telegram-Init-Data`. Backend Telegram hujjatlaridagi
HMAC-SHA256 algoritmi bilan imzoni tekshiradi va `auth_date` yoshi
`MINI_APP_AUTH_MAX_AGE_SECS`dan oshgan initData'ni rad etadi. Chatga tegishli
har bir endpoint Telegram Bot API orqali foydalanuvchining administrator
huquqini fresh tekshiradi. `OWNER_IDS` Mini App API uchun bypass bermaydi.

Frontend misoli:

```js
const initData = window.Telegram.WebApp.initData;
const response = await fetch(`${API_URL}/api/me`, {
  headers: { Authorization: `tma ${initData}` },
});
const body = await response.json();
```

Success envelope: `{ "data": ... }`. Error envelope:

```json
{"error":{"code":"admin_required","message":"..."}}
```

Asosiy UX error kodlari: `admin_required`, `group_only`, `reply_required`,
`target_is_admin`, `target_is_bot`, `invalid_duration`, `invalid_limit`,
`not_found`. API-only kodlar: `init_data_required`, `invalid_init_data`,
`init_data_expired`, `telegram_error`, `already_exists`, `invalid_filter`,
`invalid_format`, `invalid_status`, `module_required`, `module_unavailable`.
`reply_required` Telegram command transportida ishlatiladi; ID asosidagi HTTP
moderation requestiga reply kerak emas.

## Endpointlar

```text
GET    /api/me
GET    /api/chats
GET    /api/chats/{chat_id}

GET    /api/chats/{chat_id}/members?q=alisher&limit=50
GET    /api/chats/{chat_id}/members/{user_id}
GET    /api/chats/{chat_id}/members/{user_id}/warnings

POST   /api/chats/{chat_id}/moderation/warn
POST   /api/chats/{chat_id}/moderation/unwarn
POST   /api/chats/{chat_id}/moderation/mute
POST   /api/chats/{chat_id}/moderation/unmute
POST   /api/chats/{chat_id}/moderation/ban
POST   /api/chats/{chat_id}/moderation/unban

GET    /api/chats/{chat_id}/settings
PATCH  /api/chats/{chat_id}/settings

GET    /api/chats/{chat_id}/blocklist?q=reklama&limit=100
POST   /api/chats/{chat_id}/blocklist
DELETE /api/chats/{chat_id}/blocklist/{term_id}

GET    /api/chats/{chat_id}/audit?action=warn&source=admin&q=alisher&from=...&to=...&cursor=...
GET    /api/chats/{chat_id}/audit/stats
GET    /api/chats/{chat_id}/audit/export?format=csv

GET    /api/chats/{chat_id}/overview
GET    /api/chats/{chat_id}/health
GET    /api/chats/{chat_id}/modules
PATCH  /api/chats/{chat_id}/modules/{module_key}

GET    /api/chats/{chat_id}/incidents?status=detected&cursor=...
PATCH  /api/chats/{chat_id}/incidents/{incident_id}
```

Moderation body:

```json
{
  "target_user_id": 884201,
  "reason": "Takroriy reklama",
  "duration_secs": 3600
}
```

Settings PATCH body may contain any subset of:

```json
{
  "flood_limit": 8,
  "flood_window_secs": 10,
  "flood_action": "mute",
  "warn_limit": 3,
  "warn_action": "mute",
  "mute_duration_secs": 3600,
  "welcome_enabled": true,
  "welcome_template": "Xush kelibsiz, {first_name}!",
  "rules": "Guruh qoidalari"
}
```

Module PATCH body: `{ "enabled": true, "config": {} }`. `anti_flood` va
`welcome` toggle'lari real `ChatSettings`ga bog'langan; `blocklist` toggle'i
incoming moderationda tekshiriladi. Hali implementatsiya qilinmagan `captcha`,
`anti_raid`, `link_filter`, `reports` yoqilsa `module_unavailable` qaytadi.

Incident PATCH body: `{ "status": "acknowledged" }` yoki
`{ "status": "resolved" }`. `average_response_seconds` faqat haqiqiy
`acknowledged_at - detected_at` qiymatlarining o'rtachasidir.

## Protection score

Score faqat modul `enabled && healthy && configured` bo'lganda uning vaznini
oladi. Vaznlar jami 100:

```text
telegram_permissions 20    database 12       admin_auth 8
anti_flood 10              warning_policy 8  blocklist 8
audit 8                    member_index 5    captcha 5
anti_raid 4                welcome 3         rules 3
link_filter 3              reports 2         incident_response 1
```

`overview.protection_score_breakdown` har bir modul bo'yicha weight, earned va
reason qaytaradi. Bot holati hardcode emas: PostgreSQL readiness, botning guruhda
mavjudligi/adminligi, delete/restrict/ban huquqlari va oxirgi update vaqti real
tekshiriladi.

## CORS va deployment

Frontend boshqa origin'da bo'lsa `.env`ga aniq HTTPS origin yozing:

```env
MINI_APP_ORIGIN=https://miniapp.example.com
```

Wildcard origin ishlatilmaydi. Reverse proxy `/api/*`ni containerdagi `8081`
portga yo'naltirishi, TLSni tugatishi va request headerlarini saqlashi kerak.
