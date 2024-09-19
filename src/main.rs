use chrono::Timelike;
use chrono::Utc;
use chrono::NaiveTime;
use chrono_tz::Asia::Kolkata;

use tracing::{error, info};

use serenity::prelude::*;
use serenity::model::{
    channel::Message, 
    gateway::Ready};

use reqwest::Error as ReqwestError;
use serde::Deserialize;


#[derive(Debug, Deserialize)]
struct Member {
    active_time: String,
    last_seen: String,
    login_time: String,
    name: String,
    #[serde(rename = "rollNo")]
    roll_no: String,
}

struct Bot;

#[serenity::async_trait]
impl EventHandler for Bot {
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.content == "$amdctl" {
            if let Err(e) = msg.channel_id.say(&ctx.http, "amFOSS Daemon is up and running!").await {
                error!("ERROR: Could not send message: {:?}.", e);
            }
        } else if msg.content == "$presense -l" {
            send_presense_present_list(ctx).await;
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("{} is online!", ready.user.name);

        send_presense_report(ctx).await;
    }
}

async fn send_presense_present_list(ctx: Context) {
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

    const THE_LAB_CHANNEL_ID: u64 = 1252600949164474391;
    let channel_id = serenity::model::id::ChannelId::new(THE_LAB_CHANNEL_ID);
    channel_id.say(&ctx.http, list).await.expect("");
}

async fn send_presense_report(ctx: Context) {
    // TODO: Test if necessary
    let ctx = std::sync::Arc::new(ctx);

    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
    let mut sent_message: Option<Message> = None;

    loop {
        interval.tick().await;

        const THE_LAB_CHANNEL_ID: u64 = 1208438766893670451;
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
async fn serenity(
    #[shuttle_runtime::Secrets] secrets: shuttle_runtime::SecretStore,
) -> shuttle_serenity::ShuttleSerenity {
    // Get the discord token set in `Secrets.toml`
    let token = secrets
        .get("DISCORD_TOKEN")
        .expect("'DISCORD_TOKEN' was not found");

    // Set gateway intents, which decides what events the bot will be notified about
    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;

    let client = Client::builder(&token, intents)
        .event_handler(Bot)
        .await
        .expect("ERROR: Could not create client.");

    Ok(client.into())
}
