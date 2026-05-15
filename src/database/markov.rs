use super::Database;
use crate::utils::constants::{BLACKLISTED_PREFIXES, MAX_RANDOM_MESSAGE_LENGTH};
use rand::{distributions::WeightedIndex, prelude::*};
use sqlx::Row;

impl Database {
    pub async fn generate_random_sentence(&self, guild_id: u64, channel_id: u64) -> Option<String> {
        let prefix_conditions = BLACKLISTED_PREFIXES
            .iter()
            .map(|_| "word1 NOT LIKE ? || '%'")
            .collect::<Vec<_>>()
            .join(" AND ");

        let query = format!("SELECT word1, word2 FROM markov_transitions WHERE guild_id = ? AND channel_id = ? AND {} ORDER BY RANDOM() LIMIT 1", prefix_conditions);

        let mut query_builder = sqlx::query(&query)
            .bind(guild_id as i64)
            .bind(channel_id as i64);

        for prefix in BLACKLISTED_PREFIXES {
            query_builder = query_builder.bind(prefix);
        }

        let start_row = match query_builder.fetch_optional(&self.pool).await {
            Ok(row) => row,
            Err(e) => {
                eprintln!("Database error fetching Markov start row: {}", e);
                return None;
            }
        };

        let (mut word_a, mut word_b) = match start_row {
            Some(row) => (row.get::<String, _>("word1"), row.get::<String, _>("word2")),
            None => return None,
        };

        let mut response = vec![word_a.clone(), word_b.clone()];
        let mut rng = StdRng::from_entropy();

        for _ in 0..MAX_RANDOM_MESSAGE_LENGTH {
            let candidates_res = sqlx::query_as(
                "SELECT word3, CAST(weight AS FLOAT) FROM markov_transitions WHERE word1 = ? AND word2 = ? AND guild_id = ?",
            )
            .bind(&word_a)
            .bind(&word_b)
            .bind(guild_id as i64)
            .fetch_all(&self.pool)
            .await;

            let mut candidates: Vec<(String, f32)> = match candidates_res {
                Ok(rows) => rows
                    .into_iter()
                    .map(|(w, wt): (String, f64)| (w, wt as f32))
                    .collect(),
                Err(e) => {
                    eprintln!("Database error fetching Markov candidates: {}", e);
                    return None;
                }
            };

            if candidates.len() <= 1 || rng.gen_bool(0.30) {
                let fallback_res = sqlx::query_as(
                    "SELECT word3, CAST(SUM(weight) AS FLOAT) FROM markov_transitions WHERE word2 = ? AND guild_id = ? GROUP BY word3",
                )
                .bind(&word_b)
                .bind(guild_id as i64)
                .fetch_all(&self.pool)
                .await;

                let fallback_candidates: Vec<(String, f32)> = match fallback_res {
                    Ok(rows) => rows
                        .into_iter()
                        .map(|(w, wt): (String, f64)| (w, wt as f32))
                        .collect(),
                    Err(e) => {
                        eprintln!("Database error fetching fallback Markov candidates: {}", e);
                        return None;
                    }
                };

                if !fallback_candidates.is_empty() {
                    candidates = fallback_candidates;
                }
            }

            if candidates.is_empty() {
                break;
            }

            let weights: Vec<f32> = candidates.iter().map(|c| c.1).collect();
            let dist = match WeightedIndex::new(&weights) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!(
                        "Math error creating WeightedIndex (likely all zero weights): {}",
                        e
                    );
                    break;
                }
            };
            let next_word = candidates[dist.sample(&mut rng)].0.clone();

            if next_word == "__end__" || next_word == "__END__" {
                break;
            }

            response.push(next_word.clone());
            word_a = word_b;
            word_b = next_word;
        }

        Some(response.join(" "))
    }
}
