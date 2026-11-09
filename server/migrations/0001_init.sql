-- Amber Stores: initial schema.
-- Rules versions and order revisions are APPEND-ONLY. A price or setting change
-- creates a new rules version; it never rewrites history, and an order revision
-- keeps the exact request, result and rules version it was calculated with.

CREATE TABLE rules_versions (
    version_no                  INTEGER GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    rules                       JSONB       NOT NULL,
    note                        TEXT        NOT NULL DEFAULT '',
    authorised_formula_change   BOOLEAN     NOT NULL DEFAULT FALSE,
    created_at                  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE orders (
    id             UUID PRIMARY KEY,
    customer_name  TEXT,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE order_revisions (
    order_id          UUID        NOT NULL REFERENCES orders (id),
    revision          INTEGER     NOT NULL CHECK (revision >= 1),
    request           JSONB       NOT NULL,
    result            JSONB       NOT NULL,
    rules_version_no  INTEGER     NOT NULL REFERENCES rules_versions (version_no),
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (order_id, revision)
);

CREATE INDEX orders_updated_at_idx ON orders (updated_at DESC);

CREATE FUNCTION forbid_modification() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'table % is append-only', TG_TABLE_NAME;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER rules_versions_append_only
    BEFORE UPDATE OR DELETE ON rules_versions
    FOR EACH ROW EXECUTE FUNCTION forbid_modification();

CREATE TRIGGER order_revisions_append_only
    BEFORE UPDATE OR DELETE ON order_revisions
    FOR EACH ROW EXECUTE FUNCTION forbid_modification();
