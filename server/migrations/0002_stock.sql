-- Aluminium stock and offcuts the owner has entered. Unlike prices and orders
-- this is a live inventory list: rows can be edited or removed. An order never
-- reads this table directly; the stock used by a calculation is copied into
-- that revision's request, so history is unaffected by later edits here.
CREATE TABLE stock_items (
    id          UUID PRIMARY KEY,
    profile     TEXT        NOT NULL,
    length_mm   TEXT        NOT NULL,
    quantity    INTEGER     NOT NULL CHECK (quantity >= 1),
    confirmed   BOOLEAN     NOT NULL DEFAULT FALSE,
    note        TEXT,
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
