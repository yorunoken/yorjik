use rand::{seq::SliceRandom, Rng};
use sqlx::{sqlite::SqlitePool, Row, SqlitePool as Pool};

pub struct Database {
    pool: Pool,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = SqlitePool::connect(database_url).await?;

        // Create tables if they don't exist
        Self::setup_tables(&pool).await?;

        Ok(Database { pool })
    }

    async fn setup_tables(pool: &Pool) -> Result<(), sqlx::Error> {
        // Create messages table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                message_id INTEGER NOT NULL,
                author_id INTEGER NOT NULL,
                channel_id INTEGER NOT NULL,
                guild_id INTEGER NOT NULL,
                content TEXT NOT NULL
            )
            "#,
        )
        .execute(pool)
        .await?;

        // Create indexes for performance
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_messages_guild_channel ON messages (guild_id, channel_id)")
            .execute(pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_messages_guild_author ON messages (guild_id, author_id)")
            .execute(pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_messages_guild ON messages (guild_id)")
            .execute(pool)
            .await?;

        Ok(())
    }

    pub async fn insert_message(
        &self,
        message_id: u64,
        author_id: u64,
        channel_id: u64,
        guild_id: u64,
        content: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO messages (message_id, author_id, channel_id, guild_id, content) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(message_id as i64)
        .bind(author_id as i64)
        .bind(channel_id as i64)
        .bind(guild_id as i64)
        .bind(content)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_messages_for_markov(
        &self,
        guild_id: u64,
        channel_id: u64,
        prefixes: &[&str],
        limit: usize,
    ) -> Result<Vec<String>, sqlx::Error> {
        // Use a more efficient random sampling approach
        let count_query = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM messages WHERE guild_id = ? AND channel_id = ? AND LENGTH(content) > 10"
        )
        .bind(guild_id as i64)
        .bind(channel_id as i64)
        .fetch_one(&self.pool)
        .await?;

        if count_query < limit as i64 {
            // If we don't have enough messages, just get all of them
            let rows = sqlx::query(
                "SELECT content FROM messages WHERE guild_id = ? AND channel_id = ? AND LENGTH(content) > 10"
            )
            .bind(guild_id as i64)
            .bind(channel_id as i64)
            .fetch_all(&self.pool)
            .await?;

            let mut messages: Vec<String> = rows
                .iter()
                .map(|row| row.get::<String, _>("content"))
                .filter(|content| !prefixes.iter().any(|&prefix| content.starts_with(prefix)))
                .collect();

            // shuffle shuffle shuffle
            messages.shuffle(&mut rand::thread_rng());
            return Ok(messages);
        }

        // For large datasets, use random sampling with OFFSET
        let mut messages = Vec::with_capacity(limit);
        let mut attempts = 0;

        while messages.len() < limit && attempts < limit * 3 {
            let offset = { rand::thread_rng().gen_range(0..count_query - 100) as i64 };

            let rows = sqlx::query(
                "SELECT content FROM messages WHERE guild_id = ? AND channel_id = ? AND LENGTH(content) > 10 LIMIT 100 OFFSET ?"
            )
            .bind(guild_id as i64)
            .bind(channel_id as i64)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;

            for row in rows {
                let content: String = row.get("content");
                if !prefixes.iter().any(|&prefix| content.starts_with(prefix)) {
                    messages.push(content);
                    if messages.len() >= limit {
                        break;
                    }
                }
            }
            attempts += 1;
        }

        Ok(messages)
    }

    pub async fn get_most_popular_channel(&self, guild_id: u64) -> Result<u64, sqlx::Error> {
        let row = sqlx::query(
            "SELECT channel_id FROM messages WHERE guild_id = ? GROUP BY channel_id ORDER BY COUNT(*) DESC LIMIT 1"
        )
        .bind(guild_id as i64)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => Ok(row.get::<i64, _>("channel_id") as u64),
            None => Ok(0),
        }
    }

    pub async fn get_messages_for_leaderboard(
        &self,
        guild_id: u64,
        member_id: Option<u64>,
    ) -> Result<Vec<(String, u64)>, sqlx::Error> {
        let prefix_list: Vec<&str> = vec![
            "$", "&", "!", ".", "m.", ">", "<", "[", "]", "@", "#", "^", "*", ",", "https", "http",
        ];

        let rows = if let Some(member_id) = member_id {
            sqlx::query(
                "SELECT content, author_id FROM messages WHERE guild_id = ? AND author_id = ?",
            )
            .bind(guild_id as i64)
            .bind(member_id as i64)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query("SELECT content, author_id FROM messages WHERE guild_id = ?")
                .bind(guild_id as i64)
                .fetch_all(&self.pool)
                .await?
        };

        let messages: Vec<(String, u64)> = rows
            .iter()
            .filter(|row| {
                let content: String = row.get("content");
                !prefix_list
                    .iter()
                    .any(|&prefix| content.starts_with(prefix))
            })
            .map(|row| {
                (
                    row.get::<String, _>("content"),
                    row.get::<i64, _>("author_id") as u64,
                )
            })
            .collect();

        Ok(messages)
    }

    pub async fn get_random_message(
        &self,
        guild_id: u64,
        min_letters_amount: u64,
    ) -> Result<Option<(String, u64)>, sqlx::Error> {
        let prefix_list: Vec<&str> = vec![
            "$", "&", "!", ".", "m.", ">", "<", "[", "]", "@", "#", "^", "*", ",", "https", "http",
        ];

        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM messages WHERE guild_id = ? AND LENGTH(content) >= ?",
        )
        .bind(guild_id as i64)
        .bind(min_letters_amount as i64)
        .fetch_one(&self.pool)
        .await?;

        if count == 0 {
            return Ok(None);
        }

        let offset = {
            use rand::Rng;
            rand::thread_rng().gen_range(0..count)
        };

        let rows = sqlx::query(
            "SELECT content, author_id FROM messages WHERE guild_id = ? AND LENGTH(content) >= ? LIMIT 20 OFFSET ?"
        )
        .bind(guild_id as i64)
        .bind(min_letters_amount as i64)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        // Filter out blacklisted prefixes
        for row in rows {
            let content: String = row.get("content");
            if !prefix_list
                .iter()
                .any(|&prefix| content.starts_with(prefix))
            {
                return Ok(Some((content, row.get::<i64, _>("author_id") as u64)));
            }
        }

        // If no suitable message found in this batch, try once more
        let offset = {
            use rand::Rng;
            rand::thread_rng().gen_range(0..count.max(20) - 20)
        };
        let rows = sqlx::query(
            "SELECT content, author_id FROM messages WHERE guild_id = ? AND LENGTH(content) >= ? LIMIT 20 OFFSET ?"
        )
        .bind(guild_id as i64)
        .bind(min_letters_amount as i64)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        for row in rows {
            let content: String = row.get("content");
            if !prefix_list
                .iter()
                .any(|&prefix| content.starts_with(prefix))
            {
                return Ok(Some((content, row.get::<i64, _>("author_id") as u64)));
            }
        }

        Ok(None)
    }
}
