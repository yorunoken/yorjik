pub mod ingest;
pub mod markov;
pub mod messages;
pub mod stats;

use sqlx::{sqlite::SqlitePool, SqlitePool as Pool};

pub struct Database {
    pub pool: Pool,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = SqlitePool::connect(database_url).await?;
        Self::setup_tables(&pool).await?;
        Ok(Database { pool })
    }

    async fn setup_tables(pool: &Pool) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS messages (
                message_id INTEGER PRIMARY KEY,
                author_id INTEGER NOT NULL,
                channel_id INTEGER NOT NULL,
                guild_id INTEGER NOT NULL,
                content TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS word_counts (
                guild_id INTEGER NOT NULL,
                author_id INTEGER NOT NULL,
                word TEXT NOT NULL,
                count INTEGER NOT NULL DEFAULT 1,
                PRIMARY KEY (guild_id, author_id, word)
            );
            CREATE TABLE IF NOT EXISTS channel_stats (
                guild_id INTEGER NOT NULL,
                channel_id INTEGER NOT NULL,
                count INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (guild_id, channel_id)
            );
            CREATE TABLE IF NOT EXISTS markov_transitions (
                word1 TEXT NOT NULL,
                word2 TEXT NOT NULL,
                word3 TEXT NOT NULL,
                guild_id INTEGER NOT NULL,
                channel_id INTEGER NOT NULL,
                author_id INTEGER NOT NULL,
                weight INTEGER NOT NULL DEFAULT 1,
                PRIMARY KEY (word1, word2, word3, guild_id, channel_id, author_id)
            );
            CREATE TABLE IF NOT EXISTS guild_configs (
                guild_id INTEGER PRIMARY KEY,
                random_speak_chance REAL NOT NULL DEFAULT 0.01
            );
            CREATE INDEX IF NOT EXISTS idx_channel_stats_ranking ON channel_stats (guild_id, count DESC);
            CREATE INDEX IF NOT EXISTS idx_word_counts_ranking ON word_counts (guild_id, count DESC);
            CREATE INDEX IF NOT EXISTS idx_messages_guild_channel ON messages (guild_id, channel_id);
            CREATE INDEX IF NOT EXISTS idx_messages_guild_author ON messages (guild_id, author_id);
            CREATE INDEX IF NOT EXISTS idx_messages_guild ON messages (guild_id);
            CREATE INDEX IF NOT EXISTS idx_markov_lookup ON markov_transitions (guild_id, word1, word2);
            "#,
        )
        .execute(pool)
        .await?;

        Ok(())
    }
}
