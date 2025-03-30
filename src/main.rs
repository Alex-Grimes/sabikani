use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use terminal_size::{Width, terminal_size};

enum InputMode {
    Normal,
    Editing,
}

enum Tab {
    Search,
    Details,
}

#[derive(Subcommand)]
enum Commands {
    Search { query: String },
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
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
    #[serde(rename = "canonicalTitle")]
    cononical_title: String,
    synopsis: Option<String>,
    #[serde(rename = "averageRating")]
    average_rating: Option<String>,
    #[serde(rename = "startDate")]
    start_date: Option<String>,
    #[serde(rename = "endDate")]
    end_date: Option<String>,
    status: Option<String>,
    #[serde(rename = "episodeCount")]
    episode_count: Option<u16>,
}

struct App {
    input: String,
    input_mode: InputMode,
    active_tab: Tab,
    search_results: Vec<AnimeData>,
    selected_anime_index: Option<usize>,
    loading: bool,
}

impl App {
    fn new() -> App {
        App {
            input: String::new(),
            input_mode: InputMode::Normal,
            active_tab: Tab::Search,
            search_results: Vec::new(),
            selected_anime_index: None,
            loading: false,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Search { query } => {
            println!("Searching for: {}", query.yellow());

            let results = search_anime(query).await?;
            display_anime_results(&results.data);

            println!(
                "\nTo watch an anime, run: {} <anime_id>",
                "anime-cli watch".cyan()
            );
        }
    }
    Ok(())
}

async fn search_anime(query: &str) -> Result<AnimeResponse> {
    let client = Client::new();

    let url = format!("https://kitsu.io/api/edge/anime?filter[text]={}", query);

    let response = client
        .get(&url)
        .header("Accept", "application/vnd.api+json")
        .header("Content-Type", "application/vnd.api+json")
        .send()
        .await
        .context("Failed to send request to kitsu API")?;

    let anime_data = response
        .json::<AnimeResponse>()
        .await
        .context("Failed to parse anime data")?;

    Ok(anime_data)
}

fn display_anime_results(anime_list: &[AnimeData]) {
    if anime_list.is_empty() {
        println!("{}", "No results found.".red());
        return;
    }

    let width = if let Some((Width(w), _)) = terminal_size() {
        w as usize
    } else {
        80
    };

    println!("\n{}", "SEARCH RESULTS:".green().bold());
    println!("{}", "=".repeat(width.min(100)));

    for (i, anime) in anime_list.iter().enumerate() {
        let attrs = &anime.attributes;

        println!(
            "{}. {} (ID: {})",
            (i + 1).to_string().yellow().bold(),
            attrs.cononical_title.cyan().bold(),
            anime.id
        );

        if let Some(rating) = &attrs.average_rating {
            println!("  Rating: {}/100", rating.green());
        }

        if let Some(eps) = attrs.episode_count {
            println!("  Episodes: {}", eps.to_string().yellow());
        }

        if let Some(status) = &attrs.status {
            let status_colored = match status.as_str() {
                "finished" => status.green(),
                "current" => status.cyan(),
                "upcoming" => status.yellow(),
                _ => status.normal(),
            };
            println!("  Status: {}", status_colored);
        }

        if let Some(start) = &attrs.start_date {
            let date_str = if let Some(end) = &attrs.end_date {
                format!("{} to {}", start, end)
            } else {
                format!("{} to present", start)
            };
            println!("  Aired: {}", date_str.blue());
        }

        if let Some(synopsis) = &attrs.synopsis {
            let max_len = width.min(100) - 10;
            let disp_synopsis = if synopsis.len() > max_len {
                format!("{}...", &synopsis[..max_len])
            } else {
                synopsis.clone()
            };
            println!("  {}", disp_synopsis.truecolor(200, 200, 200));
        }

        println!("{}", "-".repeat(width.min(100)));
    }
}
