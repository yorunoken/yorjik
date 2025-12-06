use rand::rngs::StdRng;
use rand::Rng;
use rand::SeedableRng;
use std::sync::Arc;

use serenity::all::{ChannelId, Context, GuildId};

use crate::database::Database;
use crate::utils::markov_chain;
use crate::MarkovChainGlobal;

const DATABASE_MESSAGE_FETCH_LIMIT: usize = 5000;

pub async fn generate_markov_message(
    ctx: &Context,
    guild_id: GuildId,
    channel_id: ChannelId,
    custom_word: Option<&str>,
    database: Arc<Database>,
) -> Option<String> {
    {
        let data_read = ctx.data.read().await;
        if let Some(cache_lock) = data_read.get::<MarkovChainGlobal>() {
            let cache = cache_lock.read().await;
            if let Some(chain) = cache.get(&channel_id.get()) {
                let mut rng = rand::thread_rng();
                let max_words = rng.gen_range(1..15);
                return Some(chain.generate(max_words, custom_word));
            }
        }
    }

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

    let mut markov_chain = markov_chain::Chain::new();
    markov_chain.train(sentences);

    {
        let data_read = ctx.data.read().await;
        if let Some(cache_lock) = data_read.get::<MarkovChainGlobal>() {
            let mut cache = cache_lock.write().await;
            cache.insert(channel_id.get(), markov_chain.clone());
        }
    }

    let mut rng = StdRng::from_entropy();
    let max_words = rng.gen_range(1..15);
    Some(markov_chain.generate(max_words, custom_word))
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
