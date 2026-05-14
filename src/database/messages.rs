use super::Database;
use crate::utils::constants::BLACKLISTED_PREFIXES;
use sqlx::Row;

impl Database {
    pub async fn get_random_message(
        &self,
        guild_id: u64,
        min_letters_amount: u64,
    ) -> Result<Option<(String, u64)>, sqlx::Error> {
        let prefix_conditions = BLACKLISTED_PREFIXES
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

        for prefix in &BLACKLISTED_PREFIXES {
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
