use super::Database;
use sqlx::Row;

impl Database {
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
}
