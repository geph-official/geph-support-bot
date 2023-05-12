use sqlx::{
    migrate::MigrateDatabase, Connection, Executor, Row, Sqlite, SqliteConnection, SqlitePool,
};

pub struct ChatHistory {
    db_pool: SqlitePool,
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

impl ChatHistory {
    /// Creates a new chat history database
    pub async fn new(db_path: &str) -> anyhow::Result<Self> {
        if !Sqlite::database_exists(db_path).await? {
            Sqlite::create_database(db_path).await?;

            // create tables
            // note: sqlite doesn't have 64 bit integers, so we store the convo_id as a string
            let mut conn = SqliteConnection::connect(db_path).await?;
            conn.execute(
                "CREATE TABLE IF NOT EXISTS messages (
                convo_id TEXT PRIMARY KEY,
                text TEXT,
                sender TEXT
            )",
            )
            .await?;

            conn.execute(
                "CREATE TABLE IF NOT EXISTS metadata (
                convo_id TEXT REFERENCES messages(convo_id) ON DELETE CASCADE,
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

    /// Returns the convo id of a message if it exists in the database
    pub async fn txt_to_id(&self, text: &str) -> Option<u64> {
        if let Ok(row) = sqlx::query("SELECT convo_id FROM messages WHERE text=?")
            .bind(text)
            .fetch_one(&self.db_pool)
            .await
        {
            let id: String = row.get("convo_id");
            Some(id.parse::<u64>().unwrap())
        } else {
            None
        }
    }

    /// Returns all messages in DB with the given convo_id with sender info, formatted as:
    /// "sender: message"
    /// TODO: order of the messages
    pub async fn get_context(&self, convo_id: u64) -> anyhow::Result<Vec<String>> {
        let rows = sqlx::query("SELECT sender, text FROM messages WHERE convo_id=?")
            .bind(convo_id.to_string())
            .fetch_all(&self.db_pool)
            .await?;

        let ret = rows
            .iter()
            .map(|row| {
                let sender: String = row.get("sender");
                let text: String = row.get("text");
                sender + ": " + &text
            })
            .collect();

        Ok(ret)
    }
}
