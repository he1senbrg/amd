
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
use sqlx::PgPool;
use db::member::Member;
use std::{env, sync::Arc};
use serde_json::json;

mod db;

struct Data {
    pool: Arc<PgPool>,
}




#[derive(Debug, Deserialize, Clone)]
struct MemberAttendance {
    active_time: String,
    timeout: String,
    timein: String,
    name: String,
    
}


/*
#[serde(rename = "rollNo")]
    roll_no: String,
 */
struct Handler;

#[derive(Clone)]
struct MyState {
    pool: Arc<PgPool>,
}

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;



#[serenity::async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: serenity::Context, ready: serenity::Ready) {
        println!("{} is online!", ready.user.name);

        send_presense_report(ctx).await;
    }

    async fn message(&self, ctx: serenity::Context, msg: serenity::model::channel::Message) {
        if msg.content.to_lowercase().contains("namah shivaya") {
            msg.reply(&ctx.http, "Enne kollathirikkan pattuvo?").await.expect("");
        }
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
    database_demo(ctx).await.expect("");
    Ok(())
}


#[poise::command(prefix_command)]
async fn test_list(ctx: Context<'_>) -> Result<(), Error> {

    let initial_message = generate_report().await;
    const THE_LAB_CHANNEL_ID: u64 = 1281182109670572054;
    let channel_id = serenity::model::id::ChannelId::new(THE_LAB_CHANNEL_ID);


    let _sent_message = match channel_id.say(&ctx.http(), &initial_message).await {
        Ok(msg) => Some(msg),
        Err(why) => {
            println!("ERROR: Could not send message: {:?}", why);
            None
        },
    };

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
                if is_late(&member.timein) {
                    late.push(member.name.clone());
                }

                if absent_for_more_than_thirty_min(&member.timeout) {
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
        error!("ERROR: Could not parse timein for member.");
        return false;
    }
}

fn absent_for_more_than_thirty_min(time: &str) -> bool {
    if let Ok(timeout_time) = NaiveTime::parse_from_str(time, "%H:%M") {
        let kolkata_time_now = Utc::now().with_timezone(&Kolkata).time();

        let duration_since_timeout = kolkata_time_now.signed_duration_since(timeout_time);
        let thirty_minutes = chrono::Duration::minutes(30);

        return duration_since_timeout > thirty_minutes;
    } else {
        error!("ERROR: Could not parse timeout time for member.");
        return false;
    }
}

async fn get_presense_data() -> Result<Vec<MemberAttendance>, ReqwestError> {
    const URL: &str = "https://root.shuttleapp.rs/";

    let current_date = Utc::now().with_timezone(&Kolkata).format("%Y-%m-%d").to_string();

    let member_data = get_member_data().await.expect("");

    let query = json!({
        "query": format!(
            r#"
            query MyQuery {{
                getAttendance(date: "{}") {{
                    id
                    timein
                    timeout
                }}
            }}
        "#, current_date
        )
    });



    let response = reqwest::Client::new()
        .post(URL)
        .header("Content-Type", "application/json")
        .body(query.to_string())
        .send()
        .await?;

    let text_response = response.text().await?;
    println!("{:?}", &text_response);
    let json_response: serde_json::Value = serde_json::from_str(&text_response).expect("Failed to parse JSON");
    let members: Vec<MemberAttendance> = json_response["data"]["getAttendance"].as_array().unwrap().iter().map(|member| {
        MemberAttendance {
            name: member_data.iter().find(|&x| x.id == member["id"].as_i64().unwrap() as i32).unwrap().name.clone(),
            timein: match NaiveTime::parse_from_str(member["timein"].as_str().unwrap_or("00:00:00"), "%H:%M:%S%.f") {
                Ok(time) => time.format("%H:%M").to_string(),
                Err(_) => "Invalid Time".to_string(),
            },
            
            timeout: match NaiveTime::parse_from_str(member["timeout"].as_str().unwrap_or("00:00:00"), "%H:%M:%S%.f") {
                Ok(time) => time.format("%H:%M").to_string(),
                Err(_) => "Invalid Time".to_string(),
            },
            active_time: if member["timeout"].as_str().unwrap_or("00:00:00") == "00:00:00" {
                "Absent".to_string()
            } else {
                let timein = NaiveTime::parse_from_str(member["timein"].as_str().unwrap_or("00:00:00"), "%H:%M:%S%.f").unwrap();
                let timeout = NaiveTime::parse_from_str(member["timeout"].as_str().unwrap_or("00:00:00"), "%H:%M:%S%.f").unwrap();
                let active_time = timeout.signed_duration_since(timein);
                active_time.to_string()
            },
        }
    }).collect();

    members.iter().for_each(|member| {
        println!("{} : {}, {}, {}", member.name, member.timein, member.timeout, member.active_time);
    });

    // let members: Vec<MemberAttendance> = response.json().await?;

    Ok(members)
}

async fn get_member_data() -> Result<Vec<Member>, ReqwestError>  {
    const URL: &str = "https://root.shuttleapp.rs/";

    let query = json!({
        "query": r#"
            query MyQuery {
                getMember {
                    id
                    name
                }
            }
        "#
    });

    let response = reqwest::Client::new()
        .post(URL)
        .header("Content-Type", "application/json")
        .body(query.to_string())
        .send()
        .await?;

    let text_response = response.text().await?;
    println!("{:?}", &text_response);
    let json_response: serde_json::Value = serde_json::from_str(&text_response).expect("Failed to parse JSON");
    let members: Vec<Member> = json_response["data"]["getMember"].as_array().unwrap().iter().map(|member| {
        Member {
            name: member["name"].as_str().unwrap().to_string(),
            id: member["id"].as_i64().unwrap() as i32,
        }
    }).collect();

    members.iter().for_each(|member| {
        println!("{} : {}", member.id,member.name);
    });

    Ok(members)
}




async fn database_demo(ctx: Context<'_>) -> Result<(), Error> {
    let pool = &ctx.data().pool; // Access the pool from the context

    // Dereference the Arc and use it directly
    let table_names: Vec<String> = sqlx::query_scalar(
        "SELECT table_name FROM information_schema.tables WHERE table_schema='public';"
    )
    .fetch_all(pool.as_ref()) // Use `as_ref()` to get a reference to the Pool
    .await
    .expect("Failed to fetch table names");

    // Print the table names
    println!("Tables in the database:");
    for table in table_names {
        println!("{}", table);
    }

    Ok(())
}

#[shuttle_runtime::main]
async fn main(
        #[shuttle_shared_db::Postgres] pool: PgPool,
        #[shuttle_runtime::Secrets] secret_store: SecretStore,
    ) -> ShuttleSerenity {
    env::set_var("PGOPTIONS", "-c ignore_version=true");

    sqlx::migrate!().run(&pool).await.expect("Failed to run migrations");

    let pool = Arc::new(pool);
    let _state = MyState { pool: pool.clone() };

    match get_member_data().await {
        Ok(_members) => {
            println!("huh");
        },
        Err(_e) => {
            error!("ERROR: Failed to retrieve presence data.");
        }
    }

    // Get the discord token set in `Secrets.toml`
    let token = secret_store
        .get("DISCORD_TOKEN")
        .context("'DISCORD_TOKEN' was not found")?;
    
    // Set gateway intents, which decides what events the bot will be notified about
    let intents = serenity::GatewayIntents::GUILD_MESSAGES 
        | serenity::GatewayIntents::MESSAGE_CONTENT 
        | serenity::GatewayIntents::GUILD_MEMBERS 
        | serenity::GatewayIntents::GUILD_PRESENCES;

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
                test_list(),
            ],
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                println!("Logged in as {}", _ready.user.name);
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data { pool })
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