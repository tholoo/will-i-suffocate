use aqi::{co, no2, ozone8, pm10, pm2_5, so2_1, AirQuality, AirQualityLevel};
use serde::Deserialize;
use std::{collections::HashMap, time::Duration};
use teloxide::{prelude::*, utils::command::BotCommands};
use tokio::time::timeout;

// --------------------- //
// BEGIN WAQI Data Model //
// --------------------- //

#[allow(unused)]
#[derive(Debug, Deserialize)]
struct ApiResponse {
    status: String,
    data: PollutionData,
}

#[allow(unused)]
#[derive(Debug, Deserialize)]
struct PollutionData {
    aqi: u32,
    idx: u32,
    attributions: Vec<Attribution>,
    city: City,
    dominentpol: String,
    iaqi: HashMap<String, IaqiValue>,
    time: Time,
    forecast: Forecast,
}

#[allow(unused)]
#[derive(Debug, Deserialize)]
struct Attribution {
    url: String,
    name: String,
}

#[allow(unused)]
#[derive(Debug, Deserialize)]
struct City {
    geo: Vec<f64>,
    name: String,
    url: String,
    location: String,
}

#[allow(unused)]
#[derive(Debug, Deserialize)]
struct IaqiValue {
    v: f64,
}

#[allow(unused)]
#[derive(Debug, Deserialize)]
struct Time {
    s: String,
    tz: String,
    v: u64,
    iso: String,
}

#[allow(unused)]
#[derive(Debug, Deserialize)]
struct Forecast {
    daily: HashMap<String, Vec<DailyForecast>>,
}

#[allow(unused)]
#[derive(Debug, Deserialize)]
struct DailyForecast {
    avg: u32,
    day: String,
    max: u32,
    min: u32,
}

// ------------------- //
// BEGIN Bot Commands  //
// ------------------- //

#[tokio::main]
async fn main() {
    let bot = Bot::from_env();

    Command::repl(bot, answer).await;
}

#[derive(BotCommands, Clone)]
#[command(
    rename_rule = "lowercase",
    description = "These commands are supported:"
)]
enum Command {
    #[command(description = "start the bot.")]
    Start,
    #[command(description = "display this text.")]
    Help,
    #[command(description = "get pollution data for a city.")]
    Wis { city: String },
}

async fn answer(bot: Bot, msg: Message, cmd: Command) -> ResponseResult<()> {
    match cmd {
        Command::Help | Command::Start => {
            bot.send_message(msg.chat.id, Command::descriptions().to_string())
                .await?
        }
        Command::Wis { city } => {
            if city.trim().is_empty() {
                bot.send_message(msg.chat.id, "Usage:\n/wis city_name")
                    .await?;
                return Ok(());
            }
            let result = match get_city_pollution_emoji(city.as_str()).await {
                Ok(text) => text,
                Err(e) => {
                    println!("{e}");
                    format!("Couldn't get data for {city}")
                }
            };
            bot.send_message(msg.chat.id, result).await?
        }
    };

    Ok(())
}

// --------------------- //
// BEGIN Helper Functions//
// --------------------- //

async fn get_city_pollution_emoji(city: &str) -> Result<String, Box<dyn std::error::Error>> {
    let data = get_city_pollution(city).await?;

    let dominant = data.dominentpol.as_str();

    let val = data
        .iaqi
        .get(dominant)
        .ok_or_else(|| format!("Data for dominant pollutant ({dominant}) not available."))?
        .v;

    let aqi_level = calc_aqi_by_name(dominant, val)
        .map_err(|e| format!("Failed to determine AQI from {dominant}: {e}"))?;

    println!("City: {city}, Dominant pol: {dominant}, value: {val}, => {aqi_level:?}");

    let current_date = data
        .time
        .s
        .split_whitespace()
        .next()
        .ok_or("Failed to parse date")?;

    let (emoji, progress_bar) = air_quality_to_emoji(aqi_level.level(), aqi_level.aqi());
    let mut text = format!(
        "💚➔ 💛➔ 🧡➔ ❤️➔ 💜➔ 🖤\n{}\n{} {}\n{}\n",
        data.city.name, current_date, emoji, progress_bar
    );

    if let Some(forecast_list) = data.forecast.daily.get(dominant) {
        for day in forecast_list {
            if day.day.as_str() > current_date {
                let forecast_val = day.avg as f64;

                let forecast_aqi_level = calc_aqi_by_name(dominant, forecast_val)
                    .map_err(|e| format!("Forecast AQI calc failed for {dominant}: {e}"))?;

                let (emoji, progress_bar) =
                    air_quality_to_emoji(forecast_aqi_level.level(), forecast_aqi_level.aqi());
                text.push_str(&format!("{} {}\n{}\n", day.day, emoji, progress_bar));
            }
        }
    }

    Ok(text)
}

async fn get_city_pollution(city: &str) -> Result<PollutionData, Box<dyn std::error::Error>> {
    let aqi_token = std::env::var("AQI_TOKEN").expect("AQI_TOKEN must be set!");

    let url = format!("https://api.waqi.info/feed/{city}/?token={aqi_token}");
    let result = timeout(Duration::from_secs(10), reqwest::get(url)).await;

    match result {
        Ok(Ok(response)) => {
            let resp = response.json::<ApiResponse>().await?;
            if resp.status == "ok" {
                Ok(resp.data)
            } else {
                Err(format!("API returned an error: {}", resp.status).into())
            }
        }
        Ok(Err(e)) => Err(Box::new(e)),            // reqwest error
        Err(_) => Err("Request timed out".into()), // Timeout error
    }
}

fn air_quality_to_emoji(level: AirQualityLevel, aqi: u32) -> (String, String) {
    use AirQualityLevel::*;

    let progress_bar_size = 10;
    let progress = ((aqi.min(500) as f64) / 25.0).ceil() as usize;
    let progress = progress.min(progress_bar_size);
    let progress_bar: String = "█".repeat(progress) + &"░".repeat(progress_bar_size - progress);
    let progress_bar = format!("{} [{}] {}", "🌳", progress_bar, "💀");

    let emoji = match level {
        Good => "💚",
        Moderate => "💛",
        UnhealthySensitive => "🧡",
        Unhealthy => "❤️",
        VeryUnhealthy => "💜",
        Hazardous => "🖤",
    };

    (emoji.into(), progress_bar)
}

fn calc_aqi_by_name(pollutant: &str, value: f64) -> Result<AirQuality, String> {
    match pollutant.to_lowercase().as_str() {
        "pm25" => pm2_5(value).map_err(|e| e.to_string()),
        "pm10" => pm10(value).map_err(|e| e.to_string()),
        "o3" => ozone8(value).map_err(|e| e.to_string()),
        "no2" => no2(value).map_err(|e| e.to_string()),
        "so2" => so2_1(value).map_err(|e| e.to_string()),
        "co" => co(value).map_err(|e| e.to_string()),
        other => Err(format!("Unsupported or unknown pollutant: {other}")),
    }
}
