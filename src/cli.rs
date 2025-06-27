use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "ebay")]
#[command(about = "eBay Bot Manger")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Build (Start the bot)
    Build {
        #[arg(long, short)]
        offer_percentage: i16,
    },

    /// TUI
    View,

    /// End (Kill the bot)
    Teardown,
}
