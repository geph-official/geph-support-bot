use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{
    migrate::MigrateDatabase, Connection, Executor, Row, Sqlite, SqliteConnection, SqlitePool,
};

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

// #[derive(sqlx::FromRow)]
// struct DbMessage {
//     convo_id: String,
//     text: String,
//     sender: String,
// }

// #[derive(sqlx::FromRow)]
// struct DbMetadata {
//     convo_id: u64,
//     platform: Platform,
// }

pub struct ChatHistory {
    db_pool: SqlitePool,
}

impl ChatHistory {
    /// Creates a new chat history database
    pub async fn new(db_path: &str) -> anyhow::Result<Self> {
        if !Sqlite::database_exists(db_path).await? {
            Sqlite::create_database(db_path).await?;

            // create tables
            let mut conn = SqliteConnection::connect(db_path).await?;
            conn.execute(
                "CREATE TABLE IF NOT EXISTS messages (
                convo_id BIGINT PRIMARY KEY,
                text TEXT,
                sender TEXT
            )",
            )
            .await?;

            conn.execute(
                "CREATE TABLE IF NOT EXISTS metadata (
                convo_id BIGINT REFERENCES messages(convo_id) ON DELETE CASCADE,
                platform TEXT,
                metadata BLOB
            )",
            )
            .await?;
        }
        Ok(Self {
            db_pool: SqlitePool::connect(db_path).await?,
        })
    }

    pub async fn add_msg(
        &mut self,
        msg: Message,
        platform: Platform,
        role: Role,
        metadata: Value,
    ) -> anyhow::Result<()> {
        let mut tx = self.db_pool.begin().await?;
        sqlx::query("INSERT INTO messages (convo_id, text, sender) VALUES (?, ?, ?)")
            .bind(msg.convo_id)
            .bind(msg.text)
            .bind(role.to_string())
            .execute(&mut tx)
            .await?;
        sqlx::query("INSERT INTO metadata (convo_id, platform, metadata)")
            .bind(msg.convo_id)
            .bind(platform.to_string())
            .bind(serde_json::to_vec(&metadata)?)
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

    /// Returns all messages in DB with the given convo_id with sender info, as (sender, message)
    /// TODO: order of the messages
    pub async fn get_context(&self, convo_id: i64) -> anyhow::Result<Vec<(String, String)>> {
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
