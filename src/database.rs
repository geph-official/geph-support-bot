use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{Connection, Executor, Row, SqliteConnection, SqlitePool};

use crate::Message;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Role {
    User,
    Assistant,
}

impl Role {
    pub fn to_string(&self) -> String {
        match self {
            Role::User => "user".to_owned(),
            Role::Assistant => "assistant".to_owned(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Platform {
    Telegram,
    Email,
}

impl Platform {
    pub fn to_string(&self) -> String {
        match self {
            Platform::Telegram => "telegram".to_owned(),
            Platform::Email => "email".to_owned(),
        }
    }
}

pub struct ChatHistoryDb {
    db_pool: SqlitePool,
}

impl ChatHistoryDb {
    /// Creates a new chat history database
    pub async fn new(db_path: &str) -> anyhow::Result<Self> {
        // create tables
        let mut conn = SqliteConnection::connect(&format!("file:{db_path}?mode=rwc")).await?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS conversations (
                convo_id BIGINT PRIMARY KEY,
                platform TEXT,
                metadata BLOB
            )",
        )
        .await?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS messages (
            convo_id BIGINT,
            text TEXT,
            sender TEXT,
            FOREIGN KEY(convo_id) REFERENCES conversations(convo_id)
        )",
        )
        .await?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS facts (
            fact TEXT
        )",
        )
        .await?;

        Ok(Self {
            db_pool: SqlitePool::connect(db_path).await?,
        })
    }

    pub async fn insert_fact(&self, fact: &str) -> anyhow::Result<()> {
        sqlx::query("INSERT INTO facts (fact) VALUES (?)")
            .bind(fact)
            .execute(&self.db_pool)
            .await?;
        Ok(())
    }

    pub async fn get_all_facts(&self) -> anyhow::Result<Vec<String>> {
        let facts = sqlx::query("SELECT * FROM facts")
            .fetch_all(&self.db_pool)
            .await?;
        let ret: Vec<String> = facts.iter().map(|row| row.get("fact")).collect();
        Ok(ret)
    }

    pub async fn insert_msg(
        &self,
        msg: &Message,
        platform: Platform,
        role: Role,
        metadata: Value,
    ) -> anyhow::Result<()> {
        let mut tx = self.db_pool.begin().await?;
        sqlx::query(
            "INSERT OR REPLACE INTO conversations (convo_id, platform, metadata) VALUES (?, ?, ?)",
        )
        .bind(msg.convo_id)
        .bind(platform.to_string())
        .bind(serde_json::to_vec(&metadata)?)
        .execute(&mut tx)
        .await?;
        sqlx::query("INSERT INTO messages (convo_id, text, sender) VALUES (?, ?, ?)")
            .bind(msg.convo_id)
            .bind(msg.text.clone())
            .bind(role.to_string())
            .execute(&mut tx)
            .await?;
        tx.commit().await?;

        Ok(())
    }

    /// Returns the convo id of a message if it exists in the database
    pub async fn txt_to_id(&self, text: &str) -> Option<i64> {
        if let Ok(row) = sqlx::query("SELECT convo_id FROM messages WHERE text=?")
            .bind(text)
            .fetch_one(&self.db_pool)
            .await
        {
            let id: i64 = row.get("convo_id");
            Some(id)
        } else {
            None
        }
    }

    /// Returns the convo id of an email thread, if it exists in the database
    pub async fn email_metadata_to_id(&self, email_meta: Value) -> Option<i64> {
        if let Ok(row) = sqlx::query("SELECT convo_id FROM conversations WHERE metadata=?")
            .bind(email_meta)
            .fetch_one(&self.db_pool)
            .await
        {
            let id: i64 = row.get("convo_id");
            Some(id)
        } else {
            None
        }
    }

    /// Returns all messages in DB with the given convo_id with sender info, as (sender, message)
    /// TODO: order of the messages
    pub async fn get_convo_history(&self, convo_id: i64) -> anyhow::Result<Vec<(String, String)>> {
        let rows = sqlx::query("SELECT sender, text FROM messages WHERE convo_id=?")
            .bind(convo_id)
            .fetch_all(&self.db_pool)
            .await?;

        Ok(rows
            .iter()
            .map(|row| (row.get("sender"), row.get("text")))
            .collect())
    }
}

// TODO!
pub async fn trim_convo_history(mut context: Vec<(String, String)>) -> Vec<(String, String)> {
    // trim context if too long
    // currently: simple truncation. may summarize with gpt to compress later
    while context
        .iter()
        .fold(0, |len, (s1, s2)| len + s1.len() + s2.len())
        > 10000
    {
        context.remove(0);
    }
    context
}
