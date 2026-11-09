# Client demo: Vercel (web app) + Render (server)

```
 client's browser --> Vercel (Flutter web app) --> Render (Rust server) --> Supabase (database)
                                                         \--> OpenAI / Gemini / Groq (assistant)
```

Everything below is done once. Allow about an hour, most of it waiting for the first Render build.

## 1. Put the project on GitHub (Render needs this)

1. On github.com create a new **private** repository (empty: no README, no .gitignore). Call it `amber`.
2. In PowerShell:
   ```powershell
   cd C:\Users\user\OneDrive\Desktop\amber
   git remote add origin https://github.com/YOUR-NAME/amber.git
   git push -u origin main
   ```
   The project is already committed. `.env` (all your secrets) is ignored by git and is NOT uploaded.

## 2. The server on Render

1. Make the settings to paste (this reads your private `.env`):
   ```powershell
   .\deploy\make-render-env.ps1
   ```
   It writes `render.env` (contains secrets, ignored by git). Delete it after step 2.
2. In Render: **New > Blueprint**, pick the `amber` repository. Render reads `render.yaml`
   and creates the `amber-server` web service (Docker, Frankfurt, free plan).
   When it asks for the secret values, or afterwards under the service's
   **Environment > Add from .env**, paste the contents of `render.env`.
3. Wait for the first build (10 to 20 minutes: it compiles the Rust server).
4. Open `https://YOUR-SERVICE.onrender.com/health`. It should show `{"status":"ok"}`.
   Write down that address.

## 3. The web app on Vercel

```powershell
.\deploy\vercel\deploy.ps1 -ApiUrl https://YOUR-SERVICE.onrender.com
```
- First run: a browser opens to log in to Vercel. Answer the questions: set up and deploy: Yes,
  link to an existing project: No, project name: `amber-demo`, code directory: `./`.
- When it finishes it prints the **Production** address, for example `https://amber-demo.vercel.app`.
- If Vercel shows a login wall to visitors: Project > Settings > Deployment Protection > turn it off for Production.

## 4. Tell the server which website may call it

```powershell
.\deploy\make-render-env.ps1 -WebUrl https://amber-demo.vercel.app
```
In Render, open **Environment** and set `AMBER_CORS_ORIGIN` to that address (or paste `render.env`
again), save, and let it redeploy (about a minute). Then delete `render.env`.

Open the Vercel address and sign in with your administrator account.

## Before you show the client

- **Wake the server.** Render's free plan puts the server to sleep after 15 minutes without use and
  takes about a minute to wake. Open `https://YOUR-SERVICE.onrender.com/health` a few minutes before
  the meeting. (A free uptime monitor such as UptimeRobot pinging that address every 5 minutes keeps it awake.)
- **Supabase** pauses a free project after about a week of no use. Check it is running.
- Create a staff account for the client to try (Settings > Users), and enter the real letterhead
  (Settings > Quotation letterhead).
- Run one full example order, save it, open the quotation PDF and the calculation PDF.

## After the demo

- Regenerate the OpenAI, Gemini and Groq keys and your Supabase database password (they have all been
  pasted into chat). Put the new values in `.env`, run `make-render-env.ps1` and paste again.
- For real use, move the server off Render's free plan (it sleeps) and put the web app on the client's own address.

## If something goes wrong

| Problem | What to check |
|---|---|
| Render build fails | Open the build log; send me the last 40 lines. The Docker image has not been built before this. |
| Web app says "Cannot reach the server" | The server is asleep (wait a minute and retry), or `-ApiUrl` was wrong: re-run step 3. |
| Sign-in works on the server but not from the web app | `AMBER_CORS_ORIGIN` is not the exact Vercel address (no path, `https://`). |
| `/health` says 503 | The database cannot be reached: check `DATABASE_URL` and that Supabase is not paused. |
| Assistant says it is switched off | No AI key reached Render: check Environment. |
