use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use terminal_size::{Width, terminal_size};
use tui::{
    Frame, Terminal,
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs},
};

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

fn render_search_tab<B: Backend>(f: &mut Frame<B>, area: Rect, app: &App) {
    if app.loading {
        let loading_text = Paragraph::new("Loading...")
            .style(Style::default().fg(tui::style::Color::Yellow))
            .block(Block::default().borders(Borders::ALL).title("Results"));
        f.render_widget(loading_text, area);
        return;
    }

    if app.search_results.is_empty() {
        let help_message = if app.input.is_empty() {
            "Press 'e' to endter search mode, type your query, and press Enter to search."
        } else {
            "No results found. Try a different search term."
        };

        let help_text = Paragraph::new(help_message)
            .style(Style::default().fg(tui::style::Color::Gray))
            .block(Block::default().borders(Borders::ALL).title("Results"));
        f.render_widget(help_text, area);
        return;
    }

    let items: Vec<ListItem> = app
        .search_results
        .iter()
        .map(|anime| {
            let rating = anime
                .attributes
                .average_rating
                .as_ref()
                .map(|r| format!(" ({}*)", r))
                .unwrap_or_default();
            let title = format!("{}{}", anime.attributes.cononical_title, rating);

            ListItem::new(Spans::from(vec![Span::styled(title, Style::default())]))
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Results"))
        .highlight_style(
            Style::default()
                .fg(tui::style::Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    let mut state = tui::widgets::ListState::default();
    state.select(app.selected_anime_index);
    f.render_stateful_widget(list, area, &mut state);
}

fn render_details_tab<B: Backend>(f: &mut Frame<B>, area: Rect, app: &App) {
    if let Some(selected) = app.selected_anime_index {
        if selected < app.search_results.len() {
            let anime = &app.search_results[selected];
            let attrs = &anime.attributes;

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(
                    [
                        Constraint::Length(3),
                        Constraint::Length(3),
                        Constraint::Min(0),
                    ]
                    .as_ref(),
                )
                .split(area);
            // Title
            let title = Paragraph::new(attrs.cononical_title.clone())
                .style(
                    Style::default()
                        .fg(tui::style::Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
                .block(Block::default().borders(Borders::ALL).title("Title"));
            f.render_widget(title, chunks[0]);

            //Info
            let mut info = vec![];

            if let Some(rating) = &attrs.average_rating {
                info.push(format!("Rating: {}/100", rating));
            }

            if let Some(eps) = attrs.episode_count {
                info.push(format!("Episodes: {}", eps));
            }

            if let Some(status) = &attrs.status {
                info.push(format!("Status: {}", status));
            }

            if let Some(start) = &attrs.start_date {
                let date_str = if let Some(end) = &attrs.end_date {
                    format!("Aired: {} to {}", start, end)
                } else {
                    format!("Aired: {} to present", start)
                };

                info.push(date_str);
            }

            let info_text = Paragraph::new(info.join(" | "))
                .block(Block::default().borders(Borders::ALL).title("Info"));
            f.render_widget(info_text, chunks[1]);

            let synopsis = attrs
                .synopsis
                .clone()
                .unwrap_or_else(|| "No synopsis available.".to_string());

            let synopsis_text = Paragraph::new(synopsis)
                .block(Block::default().borders(Borders::ALL).title("Synopsis"))
                .wrap(tui::widgets::Wrap { trim: true });
            f.render_widget(synopsis_text, chunks[2]);
        }
    } else {
        let message = Paragraph::new("No anime selected.")
            .style(Style::default().fg(tui::style::Color::Gray))
            .block(Block::default().borders(Borders::ALL).title("Details"));
        f.render_widget(message, area);
    }
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
