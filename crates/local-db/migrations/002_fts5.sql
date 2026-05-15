CREATE VIRTUAL TABLE IF NOT EXISTS files_fts USING fts5(
    title,
    description,
    content='files',
    content_rowid='id',
    tokenize='unicode61 remove_diacritics 1'
);

CREATE TRIGGER IF NOT EXISTS files_fts_insert AFTER INSERT ON files BEGIN
    INSERT INTO files_fts(rowid, title, description)
    VALUES (new.id, new.title, new.description);
END;

CREATE TRIGGER IF NOT EXISTS files_fts_update AFTER UPDATE ON files BEGIN
    INSERT INTO files_fts(files_fts, rowid, title, description)
    VALUES ('delete', old.id, old.title, old.description);
    INSERT INTO files_fts(rowid, title, description)
    VALUES (new.id, new.title, new.description);
END;

CREATE TRIGGER IF NOT EXISTS files_fts_delete AFTER DELETE ON files BEGIN
    INSERT INTO files_fts(files_fts, rowid, title, description)
    VALUES ('delete', old.id, old.title, old.description);
END;

PRAGMA user_version = 2;
