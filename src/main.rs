#[global_allocator]
static ALLOC: snmalloc_rs::SnMalloc = snmalloc_rs::SnMalloc;
use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage,
        ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
        CreateChatCompletionRequestArgs,
    },
    Client,
};
use currency_rs::{Currency, CurrencyOpts};
use dotenv::dotenv;
use lazy_static::lazy_static;
use poise::{
    serenity_prelude::{self as serenity, ChannelId, CreateEmbed, EmbedAuthor},
    CreateReply,
};
use regex::Regex;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{collections::HashMap, env};
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

const CMC_API: &str = "https://pro-api.coinmarketcap.com/v2/cryptocurrency/quotes/latest";
const DISCORD_CHAR_LIMIT: usize = 1900;

lazy_static! {
    static ref CMC_KEY: String =
        env::var("CMC_KEY").expect("Expected a CoinMarketCap key in the environment");
    static ref REPLY_MAX_TOKEN: u16 = env::var("REPLY_MAX_TOKEN")
        .expect("Expected a GPT REPLY_MAX_TOKEN in the environment")
        .parse()
        .unwrap();
    static ref HISTORY_MAX_TOKEN: usize = env::var("HISTORY_MAX_TOKEN")
        .expect("Expected a GPT HISTORY_MAX_TOKEN in the environment")
        .parse()
        .unwrap();
    static ref GPT_ENGINE: String =
        env::var("GPT_ENGINE").expect("Expected a GPT Engine in the environment");
    static ref OPENAI_TOKEN: String =
        env::var("OPENAI_TOKEN").expect("Expected a OpenAI token in the environment");
    static ref OPENAI_ENDPOINT: String =
        env::var("OPENAI_ENDPOINT").expect("Expected a OpenAI endpoint in the environment");
    static ref OPENAI_COINFIG: OpenAIConfig = OpenAIConfig::new()
        .with_api_base(OPENAI_ENDPOINT.clone())
        .with_api_key(OPENAI_TOKEN.clone());
    static ref OPENAI_CLIENT: Client<OpenAIConfig> = Client::with_config(OPENAI_COINFIG.clone());
    static ref MISTRAL_ENGINE: String =
        env::var("MISTRAL_ENGINE").expect("Expected a Mistral Engine in the environment");
    static ref MISTRAL_TOKEN: String =
        env::var("MISTRAL_TOKEN").expect("Expected a Mistral token in the environment");
    static ref MISTRAL_ENDPOINT: String =
        env::var("MISTRAL_ENDPOINT").expect("Expected a Mistral endpoint in the environment");
    static ref MISTRAL_COINFIG: OpenAIConfig = OpenAIConfig::new()
        .with_api_base(MISTRAL_ENDPOINT.clone())
        .with_api_key(MISTRAL_TOKEN.clone());
    static ref MISTRAL_CLIENT: Client<OpenAIConfig> = Client::with_config(MISTRAL_COINFIG.clone());
    static ref HISTORY: Mutex<Vec<ChatCompletionRequestMessage>> = Mutex::new(Vec::new());
    static ref MISTRAL_HISTORY: Mutex<Vec<ChatCompletionRequestMessage>> = Mutex::new(Vec::new());
    static ref EMOJI_REPLACEMENTS: Vec<(&'static str, &'static str)> = vec![
        (":CLbox:", "<:CLbox:1051203986964893736>"),
        (":clPog:", "<:clPog:1004208874406039572>"),
        (":smugcat:", "<:smugcat:889673525030420480>"),
        (":cathink:", "<:cathink:889687946314272778>"),
        (":gmeow:", "<:gmeow:1021027182383997010>"),
        (":clnom:", "<:clnom:950943393045954570>"),
        (":blushycl:", "<:blushycl:933644628090028032>"),
        (":yuepetcl:", "<:yuepetcl:882811013739741184>"),
        (":clkms:", "<:clkms:960796681283203113>"),
        (":evilmewn:", "<:evilmewn:824967831510712330>"),
        (":HUH:", "<a:HUH:1010570028195774524>"),
        (":MYAAA:", "<a:MYAAA:1039322389294628946>"),
        (":clThonkSweat:", "<a:clThonkSweat:993207609102450808>"),
        (":clThonkSweat2:", "<a:clThonkSweat2:993207612361424919>"),
        (":cldance:", "<a:cldance:872280682121019462>"),
        (":clhearts:", "<a:clhearts:900513327606800395>"),
        (":petcl:", "<a:petcl:1053242378359689256>"),
        (":petloom:", "<a:petloom:837695455264636969>"),
        (":petmewny:", "<a:petmewny:828632539367342140>"),
        (
            ":upsidedownmewny:",
            "<a:upsidedownmewny:854905684625326092>"
        ),
    ];
}

struct Data {} // User data, which is stored and accessible in all command invocations
type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

#[derive(Debug, Clone, Deserialize)]
pub struct QueryResponse {
    pub id: u16,
    pub name: String,
    pub symbol: String,
    pub slug: String,
    pub max_supply: Option<f64>,
    pub circulating_supply: f64,
    pub total_supply: f64,
    pub infinite_supply: bool,
    pub self_reported_circulating_supply: Option<f64>,
    pub self_reported_market_cap: Option<f64>,
    pub tvl_ratio: Option<f64>,
    pub last_updated: String,
    pub quote: Quote,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Quote {
    #[serde(rename = "USD")]
    pub usd: Usd,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Usd {
    pub price: f64,
    pub volume_24h: f64,
    pub volume_change_24h: f64,
    pub percent_change_1h: f64,
    pub percent_change_24h: f64,
    pub percent_change_7d: f64,
    pub percent_change_30d: f64,
    pub percent_change_60d: f64,
    pub percent_change_90d: f64,
    pub market_cap: f64,
    pub market_cap_dominance: f64,
    pub fully_diluted_market_cap: f64,
    pub tvl: Option<f64>,
    pub market_cap_by_total_supply: f64,
    pub last_updated: String,
}

fn sanitize_input(input: &str) -> String {
    // Define the regex pattern
    let pattern = Regex::new(r"^[a-zA-Z0-9_-]{1,64}$").unwrap();

    // Check if the input matches the regex pattern
    if pattern.is_match(input) {
        // If it matches, return the input unchanged
        String::from(input)
    } else {
        // If it doesn't match, remove invalid characters and return the modified string
        let modified_string: String = input
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
            .collect();

        modified_string
    }
}

fn replace_emoji(mut message: String) -> String {
    for (search, replace) in EMOJI_REPLACEMENTS.iter() {
        message = message.replace(search, replace);
    }

    message
}

fn format_currency(num: f64) -> String {
    if num <= 0.0001 {
        let otp = CurrencyOpts::new().set_symbol("").set_precision(9);
        Currency::new_float(num, Some(otp)).format()
    } else if num <= 0.001 {
        let otp = CurrencyOpts::new().set_symbol("").set_precision(8);
        Currency::new_float(num, Some(otp)).format()
    } else if num <= 0.01 {
        let otp = CurrencyOpts::new().set_symbol("").set_precision(7);
        Currency::new_float(num, Some(otp)).format()
    } else if num <= 0.1 {
        let otp = CurrencyOpts::new().set_symbol("").set_precision(6);
        Currency::new_float(num, Some(otp)).format()
    } else if num <= 1.0 {
        let otp = CurrencyOpts::new().set_symbol("").set_precision(5);
        Currency::new_float(num, Some(otp)).format()
    } else if num <= 10.0 {
        let otp = CurrencyOpts::new().set_symbol("").set_precision(4);
        Currency::new_float(num, Some(otp)).format()
    } else if num <= 100.0 {
        let otp = CurrencyOpts::new().set_symbol("").set_precision(3);
        Currency::new_float(num, Some(otp)).format()
    } else if num <= 1e5 {
        let otp = CurrencyOpts::new().set_symbol("").set_precision(2);
        Currency::new_float(num, Some(otp)).format()
    } else if num <= 1e6 {
        let otp = CurrencyOpts::new().set_symbol("").set_precision(1);
        Currency::new_float(num, Some(otp)).format()
    } else {
        let otp = CurrencyOpts::new().set_symbol("").set_precision(0);
        Currency::new_float(num, Some(otp)).format()
    }
}

fn format_pct(num: f64) -> String {
    let otp = CurrencyOpts::new().set_symbol("").set_precision(2);
    Currency::new_float(num, Some(otp)).format()
}

fn up_or_down_color(num: f64) -> (u8, u8, u8) {
    if num >= 1. {
        (16, 204, 132)
    } else if num <= -1. {
        (246, 70, 93)
    } else {
        (240, 204, 212)
    }
}

/// Query Price
#[poise::command(slash_command, prefix_command)]
pub async fn p(ctx: Context<'_>, #[description = "Symbol"] symbol: String) -> Result<(), Error> {
    ctx.defer().await?;
    let symbol = symbol.to_uppercase();
    let mut map = HashMap::new();
    map.insert("symbol", symbol.as_str());
    map.insert(
        "aux",
        "max_supply,circulating_supply,total_supply,market_cap_by_total_supply",
    );

    let client = reqwest::Client::new();
    match client
        .get(CMC_API.to_string())
        .header("X-CMC_PRO_API_KEY", CMC_KEY.as_str())
        .header(reqwest::header::ACCEPT, "application/json")
        .query(&map)
        .send()
        .await?
        .json::<Value>()
        .await
    {
        Ok(res) => {
            debug!("CMC response: {:?}", res);
            if let Some(json_object) = res["data"].as_object() {
                for (_key, value) in json_object {
                    let v: QueryResponse = serde_json::from_value(value[0].to_owned())?;
                    let icon_url = format!(
                        "https://s2.coinmarketcap.com/static/img/coins/64x64/{}.png",
                        v.id
                    );
                    let author: EmbedAuthor =
                        serde_json::from_value(json!({"name": v.symbol, "icon_url": icon_url}))?;

                    let otp = CurrencyOpts::new().set_symbol("").set_precision(0);
                    let fields = vec![
                        (
                            "Price",
                            format!(
                                "$ {} ({}%)",
                                format_currency(v.quote.usd.price),
                                format_pct(v.quote.usd.percent_change_24h)
                            ),
                            false,
                        ),
                        (
                            "Market Cap",
                            format!(
                                "$ {}\nCirculating Supply: {}",
                                format_currency(v.quote.usd.market_cap),
                                Currency::new_float(v.circulating_supply, Some(otp)).format()
                            ),
                            false,
                        ),
                    ];

                    let embed = CreateEmbed::default()
                        .author(author.into())
                        .fields(fields)
                        .color(up_or_down_color(v.quote.usd.percent_change_24h));

                    ctx.send(CreateReply::default().embed(embed)).await?;
                }
            }
        }
        Err(e) => {
            error!("{:?}", e);
            ctx.say(format!(
                "> **{}** - <{}> \n\nSomething went wrong, maybe the symbol?",
                symbol,
                ctx.author()
            ))
            .await?;
        }
    };
    Ok(())
}

/// Chat to SocksGPT
#[poise::command(slash_command, prefix_command)]
pub async fn chat(
    ctx: Context<'_>,
    #[description = "Chat to SocksGPT"] message: String,
) -> Result<(), Error> {
    info!("{:?} : {:?}", ctx.author().name, message);

    ctx.defer().await?;

    let mut history = HISTORY.lock().await;
    history.push(
        ChatCompletionRequestUserMessageArgs::default()
            .content(message.clone())
            .name(sanitize_input(&ctx.author().name)) // OpenAI only accept ^[a-zA-Z0-9_-]{1,64}$ in message.1.name
            .build()?
            .into(),
    );

    let mut request = CreateChatCompletionRequestArgs::default()
        .model(GPT_ENGINE.to_string())
        .max_tokens(*REPLY_MAX_TOKEN)
        .messages(history.clone())
        .build()?;

    debug!("HISTORY: {:?}", history);
    let mut s = serde_json::to_string(&request.messages)?;
    let mut bpe = tiktoken_rs::cl100k_base().unwrap();
    let mut tokens = bpe.encode_with_special_tokens(&s);
    info!("tokens len: {}", tokens.len());
    while tokens.len() > *HISTORY_MAX_TOKEN {
        info!("Exceeded token limit");
        history.remove(1);
        request = CreateChatCompletionRequestArgs::default()
            .model(GPT_ENGINE.to_string())
            .max_tokens(*REPLY_MAX_TOKEN)
            .messages(history.clone())
            .build()?;
        s = serde_json::to_string(&request.messages)?;
        bpe = tiktoken_rs::cl100k_base().unwrap();
        tokens = bpe.encode_with_special_tokens(&s);
        info!(
            "After removing an entry, new tokens length is: {}",
            tokens.len()
        );
    }

    match OPENAI_CLIENT.chat().create(request).await {
        Ok(response) => {
            debug!(
                "{}: Role: {}  Content: {:?}",
                response.choices[0].index,
                response.choices[0].message.role,
                response.choices[0].message.content
            );
            let mut text = response.choices[0].message.content.clone().unwrap();

            if text.starts_with('\"') {
                text = text[1..].to_string()
            }
            if text.ends_with('\"') {
                text = text[..1].to_string()
            }

            history.push(
                ChatCompletionRequestAssistantMessageArgs::default()
                    .content(text.clone())
                    .build()?
                    .into(),
            );
            drop(history);

            text = format!("> **{}** - <{}> \n\n{}", message, ctx.author(), text);

            text = replace_emoji(text);

            info!("Bot say : {}", text);
            if text.len() > DISCORD_CHAR_LIMIT {
                let chunks: Vec<String> = text
                    .chars()
                    .collect::<Vec<char>>()
                    .chunks(DISCORD_CHAR_LIMIT)
                    .map(|chunk| chunk.iter().collect::<String>())
                    .collect();
                for chunk in chunks {
                    ctx.say(chunk).await?;
                }
            } else {
                ctx.say(text).await?;
            }
        }
        Err(e) => {
            error!("{:?}", e);
            ctx.say(format!(
                "> **{}** - <{}> \n\nSomething went wrong, please try again later.",
                message,
                ctx.author()
            ))
            .await?;
        }
    };
    Ok(())
}

/// Chat to SocksMistral
#[poise::command(slash_command, prefix_command)]
pub async fn mistral(
    ctx: Context<'_>,
    #[description = "Chat to SocksMistral"] message: String,
) -> Result<(), Error> {
    info!("{:?} : {:?}", ctx.author().name, message);

    ctx.defer().await?;

    let mut history = MISTRAL_HISTORY.lock().await;
    history.push(
        ChatCompletionRequestUserMessageArgs::default()
            .content(message.clone()) // OpenAI only accept ^[a-zA-Z0-9_-]{1,64}$ in message.1.name
            .build()?
            .into(),
    );

    let mut request = CreateChatCompletionRequestArgs::default()
        .model(MISTRAL_ENGINE.to_string())
        .max_tokens(*REPLY_MAX_TOKEN)
        .messages(history.clone())
        .build()?;

    debug!("MISTRAL HISTORY: {:?}", history);
    let mut s = serde_json::to_string(&request.messages)?;
    let mut bpe = tiktoken_rs::cl100k_base().unwrap();
    let mut tokens = bpe.encode_with_special_tokens(&s);
    info!("tokens len: {}", tokens.len());
    while tokens.len() > *HISTORY_MAX_TOKEN {
        info!("Exceeded token limit");
        history.remove(1);
        request = CreateChatCompletionRequestArgs::default()
            .model(MISTRAL_ENGINE.to_string())
            .max_tokens(*REPLY_MAX_TOKEN)
            .messages(history.clone())
            .build()?;
        s = serde_json::to_string(&request.messages)?;
        bpe = tiktoken_rs::cl100k_base().unwrap();
        tokens = bpe.encode_with_special_tokens(&s);
        info!(
            "After removing an entry, new tokens length is: {}",
            tokens.len()
        );
    }

    match MISTRAL_CLIENT.chat().create(request).await {
        Ok(response) => {
            debug!(
                "{}: Role: {}  Content: {:?}",
                response.choices[0].index,
                response.choices[0].message.role,
                response.choices[0].message.content
            );
            let mut text = response.choices[0].message.content.clone().unwrap();

            if text.starts_with('\"') {
                text = text[1..].to_string()
            }
            if text.ends_with('\"') {
                text = text[..1].to_string()
            }

            history.push(
                ChatCompletionRequestAssistantMessageArgs::default()
                    .content(text.clone())
                    .build()?
                    .into(),
            );
            drop(history);

            text = format!("> **{}** - <{}> \n\n{}", message, ctx.author(), text);

            text = replace_emoji(text);

            info!("Bot say : {}", text);
            if text.len() > DISCORD_CHAR_LIMIT {
                let chunks: Vec<String> = text
                    .chars()
                    .collect::<Vec<char>>()
                    .chunks(DISCORD_CHAR_LIMIT)
                    .map(|chunk| chunk.iter().collect::<String>())
                    .collect();
                for chunk in chunks {
                    ctx.say(chunk).await?;
                }
            } else {
                ctx.say(text).await?;
            }
        }
        Err(e) => {
            error!("{:?}", e);
            ctx.say(format!(
                "> **{}** - <{}> \n\nSomething went wrong, please try again later.",
                message,
                ctx.author()
            ))
            .await?;
        }
    };
    Ok(())
}

/// BONK SocksGPT makes it lost memory
#[poise::command(slash_command, prefix_command)]
async fn bonk(ctx: Context<'_>) -> Result<(), Error> {
    let mut history = HISTORY.lock().await;
    history.truncate(1);
    info!("HISTORY: {:?}", history);
    drop(history);
    ctx.say("> **BONK** Lmeow, Socksy have forgotten everything ～")
        .await?;
    Ok(())
}

/// BONK SocksMistral makes it lost memory
#[poise::command(slash_command, prefix_command)]
async fn bonk_mistral(ctx: Context<'_>) -> Result<(), Error> {
    let mut history = MISTRAL_HISTORY.lock().await;
    history.truncate(1);
    info!("HISTORY: {:?}", history);
    drop(history);
    ctx.say("> **BONK** Lmeow, SocksMistral have forgotten everything ～")
        .await?;
    Ok(())
}

/// Delete SocksGPT's message [channel_id, message_id]
#[poise::command(slash_command, prefix_command)]
async fn delete(
    ctx: Context<'_>,
    #[description = "Delete SocksGPT's message [channel_id, message_id]"]
    channel_and_message_id: String,
) -> Result<(), Error> {
    let msg = channel_and_message_id.replace(' ', "");
    let tmp: Vec<&str> = msg.split(',').collect();
    let channel_id = ChannelId::new(tmp[0].parse::<u64>()?);
    if let Err(e) = channel_id
        .delete_message(&ctx.http(), tmp[1].parse::<u64>()?)
        .await
    {
        warn!("Failed to delete bot message: {}", e);
    }
    Ok(())
}

/// emm...
#[poise::command(slash_command, prefix_command)]
async fn emm(ctx: Context<'_>, emm: String) -> Result<(), Error> {
    if ctx.author().name == "cutesocks".to_string() {
        let tmp: Vec<&str> = emm.split(',').collect();
        let channel_id = ChannelId::new(tmp[0].parse::<u64>()?);
        channel_id
            .say(
                &ctx.http(),
                tmp.iter().skip(1).cloned().collect::<Vec<&str>>().join(","),
            )
            .await?;
    }
    Ok(())
}

#[poise::command(slash_command, prefix_command)]
pub async fn help(ctx: Context<'_>, command: Option<String>) -> Result<(), Error> {
    let configuration = poise::builtins::HelpConfiguration {
        // [configure aspects about the help message here]
        ..Default::default()
    };
    poise::builtins::help(ctx, command.as_deref(), configuration).await?;
    Ok(())
}

#[tokio::main()]
async fn main() -> Result<(), Error> {
    // Configure the client with your Discord bot token in the environment.
    dotenv().ok();
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    let system_prompt =
        std::fs::read_to_string("system_prompt.txt").expect("Can't read system_prompt.txt");
    let token: String =
        env::var("DISCORD_BOT_TOKEN").expect("Expected a Discord Bot token in the environment");
    let intents = serenity::GatewayIntents::non_privileged();

    HISTORY.lock().await.push(
        ChatCompletionRequestSystemMessageArgs::default()
            .content(system_prompt.clone())
            .build()?
            .into(),
    );

    MISTRAL_HISTORY.lock().await.push(
        ChatCompletionRequestSystemMessageArgs::default()
            .content(system_prompt)
            .build()?
            .into(),
    );

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![
                p(),
                chat(),
                mistral(),
                bonk(),
                bonk_mistral(),
                delete(),
                emm(),
                help(),
            ],
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data {})
            })
        })
        .build();

    let client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await;
    client.unwrap().start().await?;
    Ok(())
}
