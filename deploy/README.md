# Putting Amber Stores online

This puts the whole system on one small Linux server with a secure (HTTPS) address, so the
office computers and phones can all use the same saved orders, stock and prices.

> **Honest status:** the server program, the database setup, sign-in, quotations and the web
> app were all tested. The Docker files in this folder (`Dockerfile`, `docker-compose.yml`,
> `Caddyfile`) were written carefully but **have not been run yet** because the computer used
> to build the system has no Docker. Expect to fix a small thing or two on the first
> deployment, and do a trial run before the client depends on it.

## What you need

| Thing | Notes |
|---|---|
| A small Linux server (VPS) | 1 to 2 CPU, 2 GB memory, 20 GB disk is plenty. Ubuntu 22.04 or 24.04. |
| Docker with the compose plugin | `curl -fsSL https://get.docker.com | sh` on the server |
| A domain name | e.g. `quotes.amberstores.co.ke`. Add an **A record** pointing to the server's IP address. |
| Ports 80 and 443 open | Both are needed: 80 is how the free certificate is obtained. |
| Optional: an Anthropic API key | Only for the AI assistant. Usage is billed to whoever owns the key. |

Ongoing costs are the server rental, the domain renewal, and AI usage if you switch the assistant on.
Check each provider's current prices; none are fixed by this software.

## First-time setup

1. **Build the web app** (on the Windows computer that has Flutter):
   ```powershell
   cd C:\Users\user\OneDrive\Desktop\amber\app
   flutter build web --release
   ```
2. **Copy the project to the server** (everything except `target` folders), for example with
   `scp -r` or by zipping it. You need these folders on the server: `engine`, `cli`, `server`,
   `deploy`, `app/build/web`, plus `Cargo.toml` and `Cargo.lock`.
3. **Fill in the settings** on the server:
   ```bash
   cd amber/deploy
   cp .env.example .env
   nano .env        # set DOMAIN, DB_PASSWORD, BOOTSTRAP_ADMIN, (optional) an AI key: ANTHROPIC_API_KEY, OPENAI_API_KEY, GEMINI_API_KEY or GROQ_API_KEY
   ```
4. **Start it** (the first build takes several minutes):
   ```bash
   docker compose up -d --build
   docker compose ps          # all three should say "running" / "healthy"
   docker compose logs -f server
   ```
5. Open `https://your-domain`, sign in with the `BOOTSTRAP_ADMIN` details, then in **Settings**:
   change your password, add the other users, and fill in the **letterhead and payment details**.
6. For Android phones, install the app and type `https://your-domain` as the server address on the
   sign-in screen.

After the first administrator exists you can remove `BOOTSTRAP_ADMIN` from `.env` (it is ignored
once any user exists).

## Everyday operations

| Task | Command (in `amber/deploy`) |
|---|---|
| See if it is healthy | `docker compose ps` and open `https://your-domain/health` |
| Read the server log | `docker compose logs --tail 100 server` |
| Update after code changes | copy the new files over, then `docker compose up -d --build` |
| Update only the web app | rebuild it with `flutter build web --release`, copy `app/build/web`, no restart needed |
| Stop everything | `docker compose down` (your data stays in the database volume) |

Updating the server automatically applies any new database changes. Saved orders, rules history and
users are kept.

## Backups (do this before real use)

```bash
./backup.sh            # saves deploy/backups/amber-DATE.sql.gz, keeps the newest 30
```

Schedule it every night: `crontab -e` and add

```
30 2 * * * /home/YOU/amber/deploy/backup.sh >> /home/YOU/amber/deploy/backup.log 2>&1
```

**A backup that only lives on the same server is not enough.** Copy the files somewhere else regularly
(another computer, cloud storage). To restore onto a fresh server:

```bash
docker compose up -d db
gunzip -c backups/amber-DATE.sql.gz | docker compose exec -T db psql -U amber amber
docker compose up -d
```

Practise a restore once, on a spare server, before you need it.

## Security notes

- Only ports 80 and 443 are open. The database and the server program cannot be reached from outside.
- Everything is behind sign-in. Wrong passwords are slowed down. Passwords are stored only as hashes.
- Administrators can switch a person off at once; their open sessions stop working immediately.
- Keep `.env` private. Anyone with it has the database password and the AI key.
- Keep the server updated: `sudo apt update && sudo apt upgrade` now and then, and turn on your
  provider's automatic snapshots if they offer them.
- Sign-in sessions last 7 days. There is no two-step verification yet.

## A temporary link for a client demo (no server needed)

If you only want to show the client the system from your own computer for an hour, the
`cloudflared` tool already installed on your PC can give a temporary public HTTPS link:

```powershell
cd C:\Users\user\OneDrive\Desktop\amber
$env:AMBER_DEV_MEMORY="1"; $env:AMBER_BOOTSTRAP_ADMIN="demo:a-demo-password"
cargo run -p amber_server        # window 1
cloudflared tunnel --url http://localhost:8080     # window 2: prints the https link
```

Anyone with that link can reach the sign-in page, so use a throwaway password. In this mode nothing is
saved when the server stops. The web version of the app is not served this way: use the Windows app and
type the link as the server address.

## Using Supabase as the database

Supabase is a hosted PostgreSQL service, so you do not need the `db` service or Docker for the
database at all.

1. In the Supabase dashboard, open **Connect** and copy the **Session pooler** address (port **5432**).
   Use the session pooler, not the "transaction" one on port 6543: the server keeps long connections
   and uses database locks that the transaction pooler breaks.
2. Download the **SSL certificate** (Project Settings, Database, SSL Configuration). Save it as
   `deploy/supabase-ca.crt` on the server and as `supabase-ca.crt` next to `run-server.ps1` on your PC.
3. The address looks like this (put your real password in, with unusual characters percent-encoded):
   ```
   postgres://postgres.PROJECTREF:PASSWORD@aws-0-REGION.pooler.supabase.com:5432/postgres?sslmode=verify-full&sslrootcert=/certs/supabase-ca.crt
   ```
   - **On your PC:** run `.
un-server.ps1` (asks for the password, sets this up for you).
   - **On the server:** in `docker-compose.yml` set the `server` service `DATABASE_URL` to that address,
     mount the certificate (`- ./supabase-ca.crt:/certs/supabase-ca.crt:ro`), and delete the `db` service
     and its `depends_on`.
4. The server creates its tables the first time it starts and **locks every table down** (row-level
   security), because Supabase also publishes the `public` schema through a web API that the project's
   public "anon" key can call. The server connects with the database password only; it never uses the
   anon key, the service-role key or the JWT secret, so keep those private and out of the app.
5. Backups: Supabase takes its own (the free plan keeps few, paid plans keep more and offer
   point-in-time recovery). `backup.sh` is for the bundled database only. Check your Supabase plan.
6. Free Supabase projects pause after a week of no use, which would take the system offline until you
   resume the project in the dashboard.

Other managed databases work the same way: use their address with `?sslmode=verify-full` and their
certificate.
