use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

fn main() {
    println!("Hello, world!");
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Search { query: String },
}

#[derive(Debug, Serialize, Deserialize)]
struct AnimeResponse {
    data: Vec<AnimeData>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnimeData {
    id: String,
    attributes: AnimeAttributes,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnimeAttributes {
    cononical_title: String,
    synopsis: Option<String>,
    average_rating: Option<String>,
    start_date: Option<String>,
    end_date: Option<String>,
    status: Option<String>,
    episode_count: Option<u16>,
}
