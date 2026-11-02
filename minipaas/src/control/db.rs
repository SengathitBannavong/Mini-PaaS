use rusqlite::{Connection, params};

/// Open (or create) the db. Deployments still `running` were cut by a crash or restart
pub fn open(path: &str) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS deployments (
                   id          INTEGER PRIMARY KEY,
                   host        TEXT NOT NULL,
                   image       TEXT NOT NULL,      -- what the user asked for (tag)
                   digest      TEXT,               -- what actually ran; NULL if pull failed
                   replicas    INTEGER NOT NULL,
                   status      TEXT NOT NULL CHECK (status IN ('running','succeeded','failed')),
                   error       TEXT,
                   started_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
                   finished_at TEXT
                 );
                 UPDATE deployments SET status = 'failed', error = 'interrupted',
                   finished_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                 WHERE status = 'running';
    ",
    )?;
    Ok(conn)
}

/// Record a new Deployment
/// return its id
pub fn start(conn: &Connection, host: &str, image: &str, replicas: u8) -> rusqlite::Result<i64> {
    conn.execute(
        "
        INSERT INTO deployments (host, image, replicas, status) VALUES (?1, ?2, ?3, 'running');
    ",
        params![host, image, replicas],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn set_digest(conn: &Connection, id: i64, digest: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE deployments SET digest = ?1 WHERE id = ?2",
        params![digest, id],
    )?;
    Ok(())
}

/// `error` = None -> succeeded
pub fn finish(conn: &Connection, id: i64, error: Option<&str>) -> rusqlite::Result<()> {
    conn.execute(
        "
        UPDATE deployments SET status = ?1, error = ?2,
        finished_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?3
    ",
        params![
            if error.is_none() {
                "succeeded"
            } else {
                "failed"
            },
            error,
            id
        ],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_deployments_and_marks_interrupted_on_reopen() {
        let path = std::env::temp_dir().join(format!("minipaas-test-{}.db", std::process::id()));
        let path = path.to_str().unwrap();
        let _ = std::fs::remove_file(path);

        let conn = open(path).unwrap();
        let ok = start(&conn, "a.localhost", "img:v1", 1).unwrap();
        set_digest(&conn, ok, "img@sha256:1").unwrap();
        finish(&conn, ok, None).unwrap();
        let cut = start(&conn, "a.localhost", "img:v2", 1).unwrap();
        drop(conn); // "crash" while `cut` is running

        let conn = open(path).unwrap();
        let status = |id: i64| -> (String, Option<String>) {
            conn.query_row(
                "SELECT status, error FROM deployments WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap()
        };
        assert_eq!(status(ok), ("succeeded".into(), None));
        assert_eq!(status(cut), ("failed".into(), Some("interrupted".into())));
        std::fs::remove_file(path).unwrap();
    }
}
