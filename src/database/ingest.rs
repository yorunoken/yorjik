use super::Database;
use sqlx::SqliteConnection;
use std::collections::HashMap;

impl Database {
    pub async fn insert_message(
        &self,
        message_id: u64,
        author_id: u64,
        channel_id: u64,
        guild_id: u64,
        content: &str,
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        Self::save_raw_message(
            &mut tx, message_id, author_id, channel_id, guild_id, content,
        )
        .await?;
        Self::update_channel_stats(&mut tx, guild_id, channel_id).await?;
        Self::update_word_counts(&mut tx, guild_id, author_id, content).await?;
        Self::update_markov_chain(&mut tx, guild_id, channel_id, author_id, content).await?;

        tx.commit().await?;
        Ok(())
    }

    async fn save_raw_message(
        conn: &mut SqliteConnection,
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
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    async fn update_channel_stats(
        conn: &mut SqliteConnection,
        guild_id: u64,
        channel_id: u64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO channel_stats (guild_id, channel_id, count)
            VALUES (?, ?, 1)
            ON CONFLICT(guild_id, channel_id) DO UPDATE SET count = count + 1
            "#,
        )
        .bind(guild_id as i64)
        .bind(channel_id as i64)
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    async fn update_word_counts(
        conn: &mut SqliteConnection,
        guild_id: u64,
        author_id: u64,
        content: &str,
    ) -> Result<(), sqlx::Error> {
        let mut local_counts: HashMap<String, i32> = HashMap::new();

        for word in content.split_whitespace() {
            *local_counts.entry(word.to_lowercase()).or_insert(0) += 1;
        }

        for (word, count) in local_counts {
            sqlx::query(
                r#"
                INSERT INTO word_counts (guild_id, author_id, word, count)
                VALUES (?, ?, ?, ?)
                ON CONFLICT(guild_id, author_id, word) DO UPDATE SET count = count + excluded.count
                "#,
            )
            .bind(guild_id as i64)
            .bind(author_id as i64)
            .bind(word)
            .bind(count)
            .execute(&mut *conn)
            .await?;
        }
        Ok(())
    }

    async fn update_markov_chain(
        conn: &mut SqliteConnection,
        guild_id: u64,
        channel_id: u64,
        author_id: u64,
        content: &str,
    ) -> Result<(), sqlx::Error> {
        let mut markov_words: Vec<String> = content
            .split_whitespace()
            .map(|w| w.to_lowercase())
            .collect();

        if markov_words.is_empty() {
            return Ok(());
        }

        markov_words.push("__END__".to_string());

        if markov_words.len() >= 3 {
            for window in markov_words.windows(3) {
                sqlx::query(
                    r#"
                    INSERT INTO markov_transitions (word1, word2, word3, guild_id, channel_id, author_id, weight)
                    VALUES (?, ?, ?, ?, ?, ?, 1)
                    ON CONFLICT(word1, word2, word3, guild_id, channel_id, author_id) DO UPDATE SET weight = weight + 1
                    "#,
                )
                .bind(&window[0])
                .bind(&window[1])
                .bind(&window[2])
                .bind(guild_id as i64)
                .bind(channel_id as i64)
                .bind(author_id as i64)
                .execute(&mut *conn)
                .await?;
            }
        }
        Ok(())
    }
}
