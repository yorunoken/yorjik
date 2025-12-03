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
                message_id INTEGER PRIMARY KEY,
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
        let prefix_conditions = prefixes
            .iter()
            .map(|_| "content NOT LIKE ? || '%'")
            .collect::<Vec<_>>()
            .join(" AND ");

        let query = format!(
            "SELECT content FROM messages WHERE guild_id = ? AND channel_id = ? AND LENGTH(content) > 10 AND {} LIMIT ? OFFSET ABS(RANDOM() % MAX((SELECT COUNT(*) FROM messages WHERE guild_id = ? AND channel_id = ? AND LENGTH(content) > 10) - ?, 1))",
            prefix_conditions
        );

        let mut query_builder = sqlx::query(&query)
            .bind(guild_id as i64)
            .bind(channel_id as i64);

        for prefix in prefixes {
            query_builder = query_builder.bind(*prefix);
        }

        let rows = query_builder
            .bind(limit as i64)
            .bind(guild_id as i64)
            .bind(channel_id as i64)
            .bind(limit as i64)
            .fetch_all(&self.pool)
            .await?;

        let messages: Vec<String> = rows
            .iter()
            .map(|row| row.get::<String, _>("content"))
            .collect();

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

        let prefix_conditions = prefix_list
            .iter()
            .map(|_| "content NOT LIKE ? || '%'")
            .collect::<Vec<_>>()
            .join(" AND ");

        let rows = if let Some(member_id) = member_id {
            let query = format!(
                "SELECT content, author_id FROM messages WHERE guild_id = ? AND author_id = ? AND {}",
                prefix_conditions
            );
            let mut query_builder = sqlx::query(&query)
                .bind(guild_id as i64)
                .bind(member_id as i64);

            for prefix in &prefix_list {
                query_builder = query_builder.bind(*prefix);
            }

            query_builder.fetch_all(&self.pool).await?
        } else {
            let query = format!(
                "SELECT content, author_id FROM messages WHERE guild_id = ? AND {}",
                prefix_conditions
            );
            let mut query_builder = sqlx::query(&query).bind(guild_id as i64);

            for prefix in &prefix_list {
                query_builder = query_builder.bind(*prefix);
            }

            query_builder.fetch_all(&self.pool).await?
        };

        let messages: Vec<(String, u64)> = rows
            .iter()
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

        let prefix_conditions = prefix_list
            .iter()
            .map(|_| "content NOT LIKE ? || '%'")
            .collect::<Vec<_>>()
            .join(" AND ");

        let query = format!(
            "SELECT content, author_id FROM messages WHERE guild_id = ? AND LENGTH(content) >= ? AND {} ORDER BY RANDOM() LIMIT 1",
            prefix_conditions
        );

        let mut query_builder = sqlx::query(&query)
            .bind(guild_id as i64)
            .bind(min_letters_amount as i64);

        for prefix in &prefix_list {
            query_builder = query_builder.bind(*prefix);
        }

        let row = query_builder.fetch_optional(&self.pool).await?;

        match row {
            Some(row) => Ok(Some((
                row.get::<String, _>("content"),
                row.get::<i64, _>("author_id") as u64,
            ))),
            None => Ok(None),
        }
    }
}
