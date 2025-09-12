pub mod collect;
pub mod generate;
pub mod guess;
pub mod leaderboard;
pub mod ping;

use serenity::all::{CommandInteraction, CreateCommand};
use serenity::futures::future::BoxFuture;
use serenity::prelude::*;
use serenity::Error;
use std::sync::Arc;

use crate::database::Database;

type CommandFn = for<'a> fn(
    &'a Context,            // Command context, `ctx`
    &'a CommandInteraction, // Command interaction, `command`
    Arc<Database>,          // Database connection
) -> BoxFuture<'a, Result<(), Error>>;

#[derive(Debug)]
pub struct Command {
    pub name: String,
    pub exec: CommandFn,
}

pub fn commands_vecs() -> Vec<Command> {
    vec![
        Command {
            name: "ping".into(),
            exec: |ctx, command, _db| Box::pin(ping::execute(ctx, command)),
        },
        Command {
            name: "guess".into(),
            exec: |ctx, command, db| Box::pin(guess::execute(ctx, command, db)),
        },
        Command {
            name: "generate".into(),
            exec: |ctx, command, db| Box::pin(generate::execute(ctx, command, db)),
        },
        Command {
            name: "leaderboard".into(),
            exec: |ctx, command, db| Box::pin(leaderboard::execute(ctx, command, db)),
        },
        Command {
            name: "collect".into(),
            exec: |ctx, command, db| Box::pin(collect::execute(ctx, command, db)),
        },
    ]
}

pub fn register_vecs() -> Vec<CreateCommand> {
    vec![
        ping::register(),
        generate::register(),
        leaderboard::register(),
        guess::register(),
        collect::register(),
    ]
}
