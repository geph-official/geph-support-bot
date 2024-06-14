use serde_json::Value;
use sqlx::{Column, Connection, Executor, Row, SqliteConnection, SqlitePool};

use crate::openai::ChatEntry;

pub struct ChatHistoryDb {
    db_pool: SqlitePool,
}

impl ChatHistoryDb {
    /// Creates a new chat history database
    pub async fn new(db_path: &str) -> anyhow::Result<Self> {
        // create tables
        let mut conn = SqliteConnection::connect(&format!("file:{db_path}?mode=rwc")).await?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS chat_entries (
            thread BIGINT,
            chat_entry BLOB
        )",
        )
        .await?;

        Ok(Self {
            db_pool: SqlitePool::connect(db_path).await?,
        })
    }

    pub async fn add_msg(&self, thread: &str, chat_entry: ChatEntry) -> anyhow::Result<()> {
        sqlx::query("INSERT INTO chat_entries (thread, chat_entry) VALUES (?, ?)")
            .bind(thread)
            .bind(serde_json::to_value(chat_entry)?)
            .execute(&self.db_pool)
            .await?;
        Ok(())
    }

    /// Returns all chat_entries in DB with the given sender
    pub async fn get_convo_history(&self, thread: &str) -> anyhow::Result<Vec<ChatEntry>> {
        let chat_entries: Vec<(Value,)> =
            sqlx::query_as("SELECT chat_entry FROM chat_entries WHERE thread=?")
                .bind(thread)
                .fetch_all(&self.db_pool)
                .await?;
        Ok(chat_entries
            .into_iter()
            .map(|(val,)| serde_json::from_value::<ChatEntry>(val).unwrap())
            .collect())
    }

    pub async fn query(&self, sql: &str) -> anyhow::Result<String> {
        let rows = sqlx::query(sql).fetch_all(&self.db_pool).await?;
        let mut result_string = String::new();
        result_string.push_str(&format!("{} rows\n", rows.len()));

        if let Some(first_row) = rows.first() {
            let columns = first_row.columns();
            let column_names: Vec<&str> = columns.iter().map(|col| col.name()).collect();

            // Construct header line with column names
            result_string.push_str(&column_names.join(", "));
            result_string.push('\n');

            // Construct lines for each row
            for row in &rows {
                let values: Vec<String> = column_names
                    .iter()
                    .map(|name| row.try_get::<String, &str>(*name).unwrap_or_default())
                    .collect();

                result_string.push_str(&values.join(", "));
                result_string.push('\n');
            }
        }
        // println!("RESULT STRING = {result_string}");
        Ok(result_string)
    }
}
