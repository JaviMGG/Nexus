CREATE TABLE IF NOT EXISTS mail_state (
    account_id TEXT PRIMARY KEY,
    last_uid INTEGER NOT NULL DEFAULT 0
);
