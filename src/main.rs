use chrono::Timelike;
use chrono::Utc;
use chrono::NaiveTime;
use chrono_tz::Asia::Kolkata;
use tracing::error;
use serenity::model::channel::Message;
use reqwest::Error as ReqwestError;
use serde::Deserialize;
use poise::serenity_prelude as serenity;
use anyhow::Context as _;
use shuttle_runtime::SecretStore;
use shuttle_serenity::ShuttleSerenity;
use serenity::prelude::*;

struct Data {}

#[derive(Debug, Deserialize)]
struct Member {
    active_time: String,
    last_seen: String,
    login_time: String,
    name: String,
    
}
/*
#[serde(rename = "rollNo")]
    roll_no: String,
 */
struct Handler;

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

#[serenity::async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: serenity::Context, ready: serenity::Ready) {
        println!("{} is online!", ready.user.name);

        send_presense_report(ctx).await;
    }
}

#[poise::command(prefix_command)]
async fn amdctl(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("amFOSS Daemon is up and running!").await?;
    Ok(())
}

#[poise::command(prefix_command)]
async fn presence_list(ctx: Context<'_>) -> Result<(), Error> {
    send_presense_present_list(ctx).await;
    Ok(())
}

async fn send_presense_present_list(ctx: Context<'_>) {
    let members = get_presense_data().await.expect("");

    let mut present_members: Vec<String> = Vec::new();
    for member in members {
        if member.active_time != "Absent" {
            present_members.push(member.name);
        }
    }
    
    let datetime = Utc::now().with_timezone(&Kolkata);
    let date_str = datetime.format("%d %B %Y").to_string();

    let mut list = format!(
        "# Attendance Report - {}\n",
        date_str
    );

    if !present_members.is_empty() {
        list.push_str(&format!("\n## Present\n"));
        for (index, name) in present_members.iter().enumerate() {
            list.push_str(&format!("{}. {}\n", index + 1, name));
        }
    }

    ctx.say(list).await.expect("");
}

async fn send_presense_report(ctx: serenity::Context) {
    // TODO: Test if necessary
    let ctx = std::sync::Arc::new(ctx);

    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
    let mut sent_message: Option<Message> = None;

    loop {
        interval.tick().await;

        const THE_LAB_CHANNEL_ID: u64 = 1281182109670572054;
        let channel_id = serenity::model::id::ChannelId::new(THE_LAB_CHANNEL_ID);

        let kolkata_now = Utc::now().with_timezone(&Kolkata);
        if kolkata_now.hour() == 18 && kolkata_now.minute() == 00 {

            let initial_message = generate_report().await;

            sent_message = match channel_id.say(&ctx.http, &initial_message).await {
                Ok(msg) => Some(msg),
                Err(why) => {
                    println!("ERROR: Could not send message: {:?}", why);
                    None
                },
            }
        }

        if kolkata_now.hour() == 19 && kolkata_now.minute() == 00 {
            if let Some(initial_message) = &sent_message {
                let new_message = generate_report().await;
                
                let edited_message = serenity::builder::EditMessage::new().content(new_message);
                channel_id.edit_message(&ctx.http, &initial_message.id, edited_message).await.expect("");
            }
        }
    }
}

async fn generate_report() -> String {

    let datetime = Utc::now().with_timezone(&Kolkata);
    let (absentees, late) = get_stragglers().await.expect("");

    let date_str = datetime.format("%d %B %Y").to_string();

    let mut report = format!(
        "# Presense Report - {}\n",
        date_str
    );

    if !absentees.is_empty() {
        report.push_str(&format!("\n## Absent\n"));
        for (index, name) in absentees.iter().enumerate() {
            report.push_str(&format!("{}. {}\n", index + 1, name));
        }
    }

    if !late.is_empty() {
        report.push_str(&format!("\n## Late\n"));
        for (index, name) in late.iter().enumerate() {
            report.push_str(&format!("{}. {}\n", index + 1, name));
        }
    }

    report
}

async fn get_stragglers() -> Result<(Vec<String>, Vec<String>), ReqwestError> {

    let mut absentees = Vec::new();
    let mut late = Vec::new();

    match get_presense_data().await {
        Ok(members) => {
            for member in members {
                if member.active_time == "Absent" {
                    absentees.push(member.name.clone());
                    continue;
                }
                // Check if they arrived after 5:45 PM
                if is_late(&member.login_time) {
                    late.push(member.name.clone());
                }

                if absent_for_more_than_thirty_min(&member.last_seen) {
                    absentees.push(member.name.clone());
                }
            }
            Ok((absentees, late))
        },
        Err(e) => {
            error!("ERROR: Failed to retrieve presense data.");
            return Err(e);
        }
    }
}

fn is_late(time: &str) -> bool {
    if let Ok(time) = NaiveTime::parse_from_str(time, "%H:%M") {
        let five_forty_five_pm = NaiveTime::from_hms_opt(17, 45, 0).expect("Hardcoded value cannot fail.");
        return time > five_forty_five_pm;
    } else {
        error!("ERROR: Could not parse login_time for member.");
        return false;
    }
}

fn absent_for_more_than_thirty_min(time: &str) -> bool {
    if let Ok(last_seen_time) = NaiveTime::parse_from_str(time, "%H:%M") {
        let kolkata_time_now = Utc::now().with_timezone(&Kolkata).time();

        let duration_since_last_seen = kolkata_time_now.signed_duration_since(last_seen_time);
        let thirty_minutes = chrono::Duration::minutes(30);

        return duration_since_last_seen > thirty_minutes;
    } else {
        error!("ERROR: Could not parse last_seen time for member.");
        return false;
    }
}

async fn get_presense_data() -> Result<Vec<Member>, ReqwestError> {
    const URL: &str = "https://labtrack.pythonanywhere.com/current_day";

    let response = reqwest::get(URL).await?;
    let members: Vec<Member> = response.json().await?;

    Ok(members)
}

#[shuttle_runtime::main]
async fn main(#[shuttle_runtime::Secrets] secret_store: SecretStore) -> ShuttleSerenity  {
    // Get the discord token set in `Secrets.toml`
    let token = secret_store
    .get("DISCORD_TOKEN")
    .context("'DISCORD_TOKEN' was not found")?;
    
    // Set gateway intents, which decides what events the bot will be notified about
    let intents = serenity::GatewayIntents::GUILD_MESSAGES | serenity::GatewayIntents::MESSAGE_CONTENT | serenity::GatewayIntents::GUILD_MEMBERS | serenity::GatewayIntents::GUILD_PRESENCES;

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            prefix_options: poise::PrefixFrameworkOptions {
                prefix: Some('$'.into()),
                case_insensitive_commands: true,
                ..Default::default()
            },
            commands: vec![
                amdctl(),
                presence_list(),
            ],
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                println!("Logged in as {}", _ready.user.name);
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data {})
            })
        })
        .build();

    let client = serenity::Client::builder(token, intents)
        .event_handler(Handler)
        .framework(framework)
        .await
        .expect("ERROR: Could not create client.");

        Ok(client.into())
}