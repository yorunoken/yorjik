use std::collections::HashMap;

use sqlx::{sqlite::SqlitePool, Row, SqlitePool as Pool};

pub struct Database {
    pool: Pool,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = SqlitePool::connect(database_url).await?;
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

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS word_counts (
                guild_id INTEGER NOT NULL,
                author_id INTEGER NOT NULL,
                word TEXT NOT NULL,
                count INTEGER NOT NULL DEFAULT 1,
                PRIMARY KEY (guild_id, author_id, word)
            )
            "#,
        )
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS channel_stats (
                guild_id INTEGER NOT NULL,
                channel_id INTEGER NOT NULL,
                count INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (guild_id, channel_id)
            )
            "#,
        )
        .execute(pool)
        .await?;

        // Create indexes for performance

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_channel_stats_ranking ON channel_stats (guild_id, count DESC)")
            .execute(pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_word_counts_ranking ON word_counts (guild_id, count DESC)")
            .execute(pool)
            .await?;

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

        sqlx::query(
            r#"
            INSERT INTO channel_stats (guild_id, channel_id, count)
            VALUES (?, ?, 1)
            ON CONFLICT(guild_id, channel_id) 
            DO UPDATE SET count = count + 1
            "#,
        )
        .bind(guild_id as i64)
        .bind(channel_id as i64)
        .execute(&self.pool)
        .await?;

        let prefix_list = [
            "$", "&", "!", ".", "m.", ">", "<", "[", "]", "@", "#", "%", "^", "*", ",",
        ];

        let mut local_counts: HashMap<String, i32> = HashMap::new();

        for word in content.split_whitespace() {
            let word_lower = word.to_lowercase();

            if prefix_list.iter().any(|&p| p == word_lower) {
                continue;
            }
            *local_counts.entry(word_lower).or_insert(0) += 1;
        }

        for (word, count) in local_counts {
            sqlx::query(
                r#"
                INSERT INTO word_counts (guild_id, author_id, word, count)
                VALUES (?, ?, ?, ?)
                ON CONFLICT(guild_id, author_id, word) 
                DO UPDATE SET count = count + excluded.count
                "#,
            )
            .bind(guild_id as i64)
            .bind(author_id as i64)
            .bind(word)
            .bind(count)
            .execute(&self.pool)
            .await?;
        }

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

        let bounds: Option<(i64, i64)> = sqlx::query_as(
            "SELECT MIN(message_id), MAX(message_id) FROM messages WHERE guild_id = ? AND channel_id = ?"
        )
        .bind(guild_id as i64)
        .bind(channel_id as i64)
        .fetch_optional(&self.pool)
        .await?;

        let (min_id, max_id) = match bounds {
            Some((min, max)) if min > 0 && max > 0 => (min, max),
            _ => return Ok(Vec::new()),
        };

        let query = format!(
            "SELECT content FROM messages 
             WHERE guild_id = ? 
             AND channel_id = ? 
             AND message_id >= (ABS(RANDOM()) % (? - ?) + ?) 
             AND LENGTH(content) > 10 
             AND {} 
             LIMIT ?",
            prefix_conditions
        );

        let mut query_builder = sqlx::query(&query)
            .bind(guild_id as i64)
            .bind(channel_id as i64)
            .bind(max_id)
            .bind(min_id)
            .bind(min_id);

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
            "SELECT channel_id FROM channel_stats WHERE guild_id = ? ORDER BY count DESC LIMIT 1",
        )
        .bind(guild_id as i64)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => Ok(row.get::<i64, _>("channel_id") as u64),
            None => Ok(0),
        }
    }

    pub async fn get_leaderboard_data(
        &self,
        guild_id: u64,
        target_user_id: Option<u64>,
        target_word: Option<&str>,
        min_length: i64,
        excludes: Option<Vec<String>>,
        limit: i64,
    ) -> Result<Vec<(String, u64, i64)>, sqlx::Error> {
        let mut sql = String::from(
            "SELECT word, author_id, count FROM word_counts WHERE guild_id = ? AND LENGTH(word) >= ?"
        );

        if target_user_id.is_some() {
            sql.push_str(" AND author_id = ?");
        }
        if target_word.is_some() {
            sql.push_str(" AND word = ?");
        }

        if let Some(ref ex) = excludes {
            if !ex.is_empty() {
                sql.push_str(" AND word NOT IN (");
                for (i, _) in ex.iter().enumerate() {
                    if i > 0 {
                        sql.push_str(", ");
                    }
                    sql.push_str("?");
                }
                sql.push(')');
            }
        }

        let mut query = sqlx::query_as::<_, (String, i64, i64)>(&sql)
            .bind(guild_id as i64)
            .bind(min_length);

        if let Some(uid) = target_user_id {
            query = query.bind(uid as i64);
        }
        if let Some(word) = target_word {
            query = query.bind(word);
        }
        if let Some(ex) = excludes {
            for word in ex {
                query = query.bind(word);
            }
        }

        query = query.bind(limit);

        let rows = query.fetch_all(&self.pool).await?;

        Ok(rows.into_iter().map(|(w, u, c)| (w, u as u64, c)).collect())
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

        let bounds: Option<(i64, i64)> = sqlx::query_as(
            "SELECT MIN(message_id), MAX(message_id) FROM messages WHERE guild_id = ?",
        )
        .bind(guild_id as i64)
        .fetch_optional(&self.pool)
        .await?;

        let (min_id, max_id) = match bounds {
            Some((min, max)) if min > 0 && max > 0 => (min, max),
            _ => return Ok(None),
        };

        let query = format!(
            "SELECT content, author_id FROM messages 
             WHERE guild_id = ? 
             AND message_id >= (ABS(RANDOM()) % (? - ?) + ?) 
             AND LENGTH(content) >= ? 
             AND {} 
             LIMIT 1",
            prefix_conditions
        );

        let mut query_builder = sqlx::query(&query)
            .bind(guild_id as i64)
            .bind(max_id)
            .bind(min_id)
            .bind(min_id)
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
