use rand::Rng;
use std::sync::Arc;

use serenity::all::{ChannelId, GuildId};

use crate::database::Database;
use crate::utils::markov_chain;

const DATABASE_MESSAGE_FETCH_LIMIT: usize = 5000;

pub async fn generate_markov_message(
    guild_id: GuildId,
    channel_id: ChannelId,
    custom_word: Option<&str>,
    database: Arc<Database>,
) -> Option<String> {
    let prefixes = [
        "$", "&", "!", ".", "m.", ">", "<", "[", "]", "@", "#", "^", "*", ",", "https", "http",
    ];

    let sentences = match database
        .get_messages_for_markov(
            guild_id.get(),
            channel_id.get(),
            &prefixes,
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

    let mut rng = rand::thread_rng();

    let mut markov_chain = markov_chain::Chain::new();
    markov_chain.train(sentences);

    let max_words = rng.gen_range(1..15);
    let generated_sentence = markov_chain.generate(max_words, custom_word);
    Some(generated_sentence)
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
