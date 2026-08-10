# CheklaBot Mini App

Pencil dizaynidan React, TypeScript va Vite asosida qurilgan Telegram Mini App.
Original `.pen` hujjati frontend yaratish jarayonida o‘zgartirilmaydi.

## Lokal visual test

```powershell
cd frontend
npm install
npm run dev
```

`http://127.0.0.1:5173` development buildida Telegram initData bo‘lmasa,
faqat lokal demo adapter ishlaydi. Demo amallari Telegram yoki PostgreSQLga
yuborilmaydi.

## Telegram bilan real test

Production build demo rejimini default holatda o‘chiradi. Frontend requestlarda
`window.Telegram.WebApp.initData`ni `Authorization: tma ...` ko‘rinishida
backendga yuboradi. Vite development server `/api` requestlarini
`http://127.0.0.1:8081` manziliga proxy qiladi.

## Docker

```powershell
docker compose up --build -d frontend
```

Docker frontend Nginx orqali `/api`ni bot containerining `8081` portiga proxy
qiladi. Faqat lokal mock tekshiruvi uchun `.env`da `VITE_ALLOW_DEMO=true`
berish mumkin. Productionda bu qiymat `false` bo‘lib qolishi shart.

Coolify'da frontend va bot alohida app bo'lsa, bot app shared networkda
`chekla-bot-api` aliasiga ega bo'lishi kerak. Frontend Nginx `/api` requestlarini
shu aliasga yuboradi. Botdagi `MINI_APP_URL` frontendning HTTPS manziliga
o'rnatilganda Telegram private chat menyusida `Boshqaruv` tugmasi yaratiladi.

## Tekshiruvlar

```powershell
npm run typecheck
npm test
npm run build
```
