use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use octocrab::models::IssueState;
use octocrab::models::issues::Issue;
use rusqlite::{Connection, params};

const DB_FILE_NAME: &str = "notehub.db";

pub struct Storage {
    conn: Connection,
}

#[derive(Debug)]
pub struct StoredIssueSummary {
    pub number: i64,
    pub title: String,
}

#[derive(Debug)]
pub struct StoredIssueDetail {
    pub number: i64,
    pub title: String,
    pub body: Option<String>,
    pub updated_at: DateTime<Utc>,
}

impl Storage {
    pub fn open() -> Result<Self> {
        let path = database_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        let conn = Connection::open(&path)
            .with_context(|| format!("failed to open database at {}", path.display()))?;
        Self::apply_pragmas(&conn)?;
        Self::migrate(&conn)?;
        Ok(Self { conn })
    }

    pub fn upsert_issue(&self, repo: &str, issue: &Issue) -> Result<()> {
        let external_id = issue.number.to_string();
        let updated_at = issue.updated_at.clone();
        let synced_at = Utc::now();
        let body = issue.body.clone().unwrap_or_default();

        self.conn.execute(
            "INSERT INTO documents (repo, kind, external_id, title, body, updated_at, synced_at)
             VALUES (?1, 'issue', ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(repo, kind, external_id) DO UPDATE SET
                 title=excluded.title,
                 body=excluded.body,
                 updated_at=excluded.updated_at,
                 synced_at=excluded.synced_at",
            params![
                repo,
                &external_id,
                &issue.title,
                &body,
                &updated_at.to_rfc3339(),
                &synced_at.to_rfc3339()
            ],
        )?;

        let document_id: i64 = self.conn.query_row(
            "SELECT id FROM documents WHERE repo=?1 AND kind='issue' AND external_id=?2",
            params![repo, &external_id],
            |row| row.get(0),
        )?;

        let state = match issue.state {
            IssueState::Open => "open",
            IssueState::Closed => "closed",
            _ => "unknown",
        };
        let labels = if issue.labels.is_empty() {
            String::new()
        } else {
            issue
                .labels
                .iter()
                .map(|label| label.name.clone())
                .collect::<Vec<_>>()
                .join(", ")
        };

        self.conn.execute(
            "INSERT INTO issue_meta (document_id, number, state, labels)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(document_id) DO UPDATE SET
                 number=excluded.number,
                 state=excluded.state,
                 labels=excluded.labels",
            params![document_id, issue.number as i64, state, labels],
        )?;

        Ok(())
    }

    pub fn list_issues(&self, repo: &str) -> Result<Vec<StoredIssueSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT issue_meta.number, documents.title
             FROM documents
             JOIN issue_meta ON issue_meta.document_id = documents.id
             WHERE documents.repo = ?1 AND documents.kind = 'issue'
             ORDER BY issue_meta.number DESC",
        )?;

        let rows = stmt.query_map([repo], |row| {
            Ok(StoredIssueSummary {
                number: row.get(0)?,
                title: row.get(1)?,
            })
        })?;

        let mut issues = Vec::new();
        for row in rows {
            issues.push(row?);
        }
        Ok(issues)
    }

    pub fn get_issue(&self, repo: &str, number: u64) -> Result<Option<StoredIssueDetail>> {
        let mut stmt = self.conn.prepare(
            "SELECT documents.title, documents.body, documents.updated_at
             FROM documents
             JOIN issue_meta ON issue_meta.document_id = documents.id
             WHERE documents.repo = ?1 AND documents.kind = 'issue' AND issue_meta.number = ?2",
        )?;

        let mut rows = stmt.query(params![repo, number as i64])?;
        if let Some(row) = rows.next()? {
            let updated_at_str: String = row.get(2)?;
            let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            Ok(Some(StoredIssueDetail {
                number: number as i64,
                title: row.get(0)?,
                body: row.get(1)?,
                updated_at,
            }))
        } else {
            Ok(None)
        }
    }

    fn apply_pragmas(conn: &Connection) -> Result<()> {
        conn.pragma_update(None, "journal_mode", &"WAL")?;
        conn.pragma_update(None, "foreign_keys", &"ON")?;
        Ok(())
    }

    fn migrate(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS documents (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                repo TEXT NOT NULL,
                kind TEXT NOT NULL,
                external_id TEXT NOT NULL,
                title TEXT NOT NULL,
                body TEXT,
                updated_at TEXT NOT NULL,
                synced_at TEXT NOT NULL,
                UNIQUE(repo, kind, external_id)
            );

            CREATE TABLE IF NOT EXISTS issue_meta (
                document_id INTEGER PRIMARY KEY,
                number INTEGER NOT NULL,
                state TEXT,
                labels TEXT,
                FOREIGN KEY(document_id) REFERENCES documents(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS notes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                document_id INTEGER NOT NULL,
                anchor TEXT,
                body TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY(document_id) REFERENCES documents(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS sync_state (
                repo TEXT NOT NULL,
                resource TEXT NOT NULL,
                cursor TEXT,
                updated_at TEXT NOT NULL,
                PRIMARY KEY (repo, resource)
            );",
        )?;
        Ok(())
    }
}

fn database_path() -> Result<PathBuf> {
    let dirs = directories::ProjectDirs::from("com", "LexicalMathical", "NoteHub")
        .context("unable to determine data directory")?;
    Ok(dirs.data_dir().join(DB_FILE_NAME))
}
