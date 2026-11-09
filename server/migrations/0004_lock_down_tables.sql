-- Keep every table private to this server.
--
-- On Supabase (and similar hosts) the "public" schema is also exposed through an
-- automatic web API that anyone holding the project's public "anon" key can call.
-- A table without row-level security would be readable and writable that way,
-- including the password hashes in "users". Switching row-level security on with
-- NO policies means that only the table owner (the role this server connects as)
-- can see or change anything; the anon / authenticated API roles get nothing.
--
-- On a plain PostgreSQL server the anon / authenticated roles do not exist and
-- the REVOKE lines are skipped. RULE FOR FUTURE MIGRATIONS: every new table must
-- be added here (or locked the same way); a test fails if one is missed.

DO $$
DECLARE
    t text;
BEGIN
    FOR t IN
        SELECT c.relname
        FROM pg_class c
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE n.nspname = current_schema()
          AND c.relkind = 'r'
          AND c.relname IN ('rules_versions', 'orders', 'order_revisions', 'stock_items',
                            'users', 'auth_sessions', 'app_settings', 'app_assets', '_sqlx_migrations')
    LOOP
        EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', t);
        IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'anon') THEN
            EXECUTE format('REVOKE ALL ON TABLE %I FROM anon', t);
        END IF;
        IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'authenticated') THEN
            EXECUTE format('REVOKE ALL ON TABLE %I FROM authenticated', t);
        END IF;
    END LOOP;
END
$$;
