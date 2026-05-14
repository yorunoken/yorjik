use rand::rngs::StdRng;
use rand::Rng;
use rand::SeedableRng;
use std::sync::Arc;

use serenity::all::{ChannelId, Context, GuildId};

use crate::database::Database;
use crate::utils::constants::BLACKLISTED_PREFIXES;
use crate::MarkovChainGlobal;

const DATABASE_MESSAGE_FETCH_LIMIT: usize = 5000;

pub async fn generate_markov_message(
    guild_id: GuildId,
    channel_id: ChannelId,
    custom_word: Option<&str>,
    database: Arc<Database>,
) -> Option<String> {
    let sentences = match database
        .get_messages_for_markov(
            guild_id.get(),
            channel_id.get(),
            &BLACKLISTED_PREFIXES,
            DATABASE_MESSAGE_FETCH_LIMIT,
        )
        .await
    {
        Ok(sentences) => sentences,
        Err(e) => {
            eprintln!("Failed to fetch messages for markov chain: {}", e);
            return None;
        }
    };

    if sentences.len() < 500 {
        return None;
    }

    // TODO: return the generated message here
    Some("radom msg".to_string())
}

pub async fn get_most_popular_channel(guild_id: GuildId, database: Arc<Database>) -> u64 {
    match database.get_most_popular_channel(guild_id.get()).await {
        Ok(channel_id) => channel_id,
        Err(e) => {
            eprintln!("Failed to get most popular channel: {}", e);
            0
        }
    }
}
