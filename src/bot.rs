use crate::{commands, config::Config, crypto::Crypto, db::Database, detector, detector::Detector, health, metrics::Metrics, state::AppState};
use anyhow::{Context as _, Result};
use serenity::{
    all::{
        ActivityData, Channel, ChannelId, Context, CreateAllowedMentions, CreateEmbed, CreateMessage,
        EventHandler, GatewayIntents, Guild, GuildId, Interaction, Message, MessageFlags, Ready, UnavailableGuild,
    },
    async_trait,
    Client,
};
use std::{sync::Arc, time::Duration};
use tokio::time;

pub fn run_blocking(token: String) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("senryu-bot")
        .build()
        .context("failed to create Tokio runtime")?;
    runtime.block_on(run_async(token))
}

async fn run_async(token: String) -> Result<()> {
    let config = Config::load(token)?;
    init_logging(&config);

    tracing::info!(version=env!("CARGO_PKG_VERSION"), db=%config.database.driver, "starting senryu-bot");
    let crypto = Crypto::new(&config.encryption.key)?;
    let db = Database::connect(&config.database.driver, &config.database.path, &config.database.dsn).await?;
    check_encryption_state(&db, &crypto).await?;
    let detector = Detector::new()?;
    let metrics = Arc::new(Metrics::new());
    let state = AppState::new(config.clone(), db.clone(), crypto, detector, metrics.clone());

    if let Some(backup) = &state.backup { backup.clone().spawn(); }
    if config.server.enabled {
        let host=config.server.host.clone(); let port=config.server.port; let m=metrics.clone(); let d=db.clone();
        tokio::spawn(async move { if let Err(e)=health::serve(host,port,m,d).await { tracing::error!(error=%e,"health server stopped"); } });
    }

    let intents = GatewayIntents::GUILDS | GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;
    let handler = Handler { state: state.clone() };
    let mut client = Client::builder(&config.discord.token, intents)
        .event_handler(handler)
        .await
        .context("failed to create Discord client; check the Bot token")?;

    tracing::info!("connecting to Discord gateway (auto-sharding enabled)");
    let shard_manager = client.shard_manager.clone();
    tokio::spawn(async move {
        wait_for_shutdown_signal().await;
        tracing::info!("shutdown signal received");
        shard_manager.shutdown_all().await;
    });
    client.start_autosharded().await.context("Discord client stopped with an error")?;
    Ok(())
}

async fn wait_for_shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut term = signal(SignalKind::terminate()).ok();
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {},
            _ = async { if let Some(ref mut s) = term { s.recv().await; } else { std::future::pending::<()>().await; } } => {},
        }
    }
    #[cfg(not(unix))]
    { let _ = tokio::signal::ctrl_c().await; }
}

fn init_logging(config:&Config){
    let filter=tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_|tracing_subscriber::EnvFilter::new(&config.log.level));
    if config.log.format.eq_ignore_ascii_case("json") { let _=tracing_subscriber::fmt().with_env_filter(filter).json().try_init(); }
    else { let _=tracing_subscriber::fmt().with_env_filter(filter).compact().try_init(); }
}

async fn check_encryption_state(db:&Database,crypto:&Crypto)->Result<()> {
    let marked=db.get_metadata("encryption_enabled").await?.as_deref()==Some("true");
    if marked&&!crypto.enabled(){ anyhow::bail!("database contains encrypted senryu data, but encryption.key is not configured"); }
    if crypto.enabled()&&!marked {
        let changed=db.encrypt_plaintext_rows(crypto).await?;
        db.set_metadata("encryption_enabled","true").await?;
        tracing::info!(changed,"existing plaintext senryu rows encrypted");
    }
    Ok(())
}

struct Handler { state: Arc<AppState> }

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        self.state.metrics.set_ready(true);
        self.state.metrics.set_guilds(ready.guilds.len());
        tracing::info!(user=%ready.user.name, guilds=ready.guilds.len(), "Discord shard ready");
        if !self.state.config.discord.playing.is_empty() { ctx.set_activity(Some(ActivityData::playing(&self.state.config.discord.playing))); }
        if self.state.start_tasks_once() {
            commands::register(&ctx.http,&self.state).await;
            spawn_daily_report(ctx.http.clone(),self.state.clone());
        }
    }

    async fn message(&self, ctx: Context, msg: Message) {
        if msg.author.bot { return; }
        self.state.metrics.message();
        if let Err(err)=handle_message(&ctx,&msg,&self.state).await { self.state.metrics.error(); tracing::warn!(message_id=%msg.id,error=%err,"message handling failed"); }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match interaction {
            Interaction::Command(c)=>commands::handle_command(&ctx,&c,self.state.clone()).await,
            Interaction::Component(c)=>commands::handle_component(&ctx,&c,self.state.clone()).await,
            Interaction::Modal(m)=>commands::handle_modal(&ctx,&m,self.state.clone()).await,
            _=>{}
        }
    }

    async fn guild_create(&self, ctx: Context, guild: Guild, is_new: Option<bool>) {
        self.state.metrics.set_guilds(ctx.cache.guild_count());
        if is_new.unwrap_or(false) {
            tracing::info!(guild_id=%guild.id,guild=%guild.name,"joined guild");
            notify_join(&ctx,&guild,&self.state).await;
            if self.state.config.discord.welcome_enabled { send_welcome(&ctx,&guild,&self.state).await; }
        } else { self.state.mark_welcome(&guild.id.to_string()); }
    }

    async fn guild_delete(&self, ctx: Context, incomplete: UnavailableGuild, full: Option<Guild>) {
        self.state.metrics.set_guilds(ctx.cache.guild_count());
        let gid=incomplete.id.to_string();
        self.state.clear_welcome(&gid);
        match self.state.db.delete_guild_data(&gid).await {
            Ok((senryus,optouts,configs))=>{tracing::info!(guild_id=%gid,senryus,optouts,configs,"guild data cleaned up");notify_leave(&ctx,incomplete.id,full.as_ref(),senryus,optouts,&self.state).await;}
            Err(e)=>{self.state.metrics.error();tracing::error!(guild_id=%gid,error=%e,"guild cleanup failed");}
        }
    }
}

async fn handle_message(ctx:&Context,msg:&Message,state:&AppState)->Result<()> {
    let Some(guild_id)=msg.guild_id else { let _=msg.channel_id.say(&ctx.http,"個チャはダメです").await; return Ok(()); };
    if state.config.admin.guild_id==guild_id.to_string(){ return Ok(()); }
    let channel=msg.channel_id.to_channel(&ctx.http).await?;
    let Channel::Guild(gc)=channel else{return Ok(());};
    if !state.channel_enabled(&guild_id.to_string(),gc.kind).await{return Ok(());}
    if handle_yome(ctx,msg,state).await?{return Ok(());}
    if state.is_muted(&msg.channel_id.to_string()).await{return Ok(());}
    if let Some(parent)=gc.parent_id { if state.is_muted(&parent.to_string()).await{return Ok(());} }
    if state.opt_out_set_by(&guild_id.to_string(),&msg.author.id.to_string()).await.is_some(){return Ok(());}
    if detector::contains_discord_tokens(&msg.content){return Ok(());}
    let spoiler=detector::contains_spoiler(&msg.content);
    let content=detector::strip_code_blocks(&detector::strip_spoiler_markers(&msg.content));
    if !detector::is_japanese_rich(&content){return Ok(());}
    let Some(parts)=state.detector.find_575(&content)? else{return Ok(());};

    let created=state.db.create_senryu(&state.crypto,&guild_id.to_string(),&msg.author.id.to_string(),&parts,spoiler).await?;
    state.metrics.detected();
    let verse=format!("{} {} {}",parts[0],parts[1],parts[2]);
    let text=if spoiler{format!("川柳を検出しました！\n||「{verse}」||")}else{format!("川柳を検出しました！\n「{verse}」")};
    let allowed=CreateAllowedMentions::new().all_users(false).all_roles(false).everyone(false).replied_user(false);
    let builder=CreateMessage::new().content(text).reference_message(msg).allowed_mentions(allowed).flags(MessageFlags::SUPPRESS_EMBEDS);
    if let Err(err)=msg.channel_id.send_message(&ctx.http,builder).await {
        let _=state.db.delete_senryu(created.id,&guild_id.to_string()).await;
        tracing::warn!(channel_id=%msg.channel_id,error=%err,"senryu reply failed; DB record rolled back");
        if is_permission_error(&err) {
            if state.mute(&msg.channel_id.to_string(),&guild_id.to_string()).await.is_ok(){state.metrics.auto_mute();tracing::warn!(channel_id=%msg.channel_id,"auto-muted channel due to missing permissions");}
        }
    }
    Ok(())
}

async fn handle_yome(ctx:&Context,msg:&Message,state:&AppState)->Result<bool>{
    let Some(gid)=msg.guild_id else{return Ok(false);};
    match msg.content.as_str(){
        "詠め"=>{let rows=state.db.random_three(&state.crypto,&gid.to_string()).await?;if rows.is_empty(){msg.channel_id.say(&ctx.http,"まだ誰も詠んでいません。あなたが先に詠んでください。").await?;}else{let verse=format!("{} {} {}",rows[0].kamigo,rows[1].nakashichi,rows[2].shimogo);let mut names=Vec::new();for s in &rows{if let Ok(uid)=s.author_id.parse::<u64>(){if let Ok(m)=gid.member(&ctx.http,uid).await{let n=m.display_name().to_string();if !names.contains(&n){names.push(n);}}}}msg.channel_id.send_message(&ctx.http,CreateMessage::new().content(format!("ここで一句\n「{verse}」\n詠み手: {}",names.join(", "))).allowed_mentions(CreateAllowedMentions::new().all_users(false).all_roles(false).everyone(false).replied_user(false)).flags(MessageFlags::SUPPRESS_EMBEDS)).await?;}Ok(true)}
        "詠むな"=>{match state.db.get_last_senryu(&state.crypto,&gid.to_string()).await?{None=>{msg.reply(&ctx.http,"まだ誰も詠んでいません。").await?;},Some(s)=>{let author=if s.author_id==msg.author.id.to_string(){"お前".into()}else if let Ok(uid)=s.author_id.parse::<u64>(){gid.member(&ctx.http,uid).await.map(|m|m.display_name().to_string()).unwrap_or_else(|_|format!("<@{}>",s.author_id))}else{format!("<@{}>",s.author_id)};let verse=format!("{} {} {}",s.kamigo,s.nakashichi,s.shimogo);let body=if s.spoiler{format!("{author}が||「{verse}」||って詠んだのが最後やぞ")}else{format!("{author}が「{verse}」って詠んだのが最後やぞ")};msg.channel_id.send_message(&ctx.http,CreateMessage::new().content(body).reference_message(msg).allowed_mentions(CreateAllowedMentions::new().all_users(false).all_roles(false).everyone(false).replied_user(false)).flags(MessageFlags::SUPPRESS_EMBEDS)).await?;}}Ok(true)}
        _=>Ok(false)
    }
}

fn is_permission_error(err:&serenity::Error)->bool { let s=err.to_string(); s.contains("50001")||s.contains("50013")||s.contains("160002")||s.contains("Missing Access")||s.contains("Missing Permissions") }

async fn send_welcome(ctx:&Context,guild:&Guild,state:&AppState){
    if !state.mark_welcome(&guild.id.to_string()){return;}
    let Some(channel)=guild.system_channel_id else{state.clear_welcome(&guild.id.to_string());return;};
    let embed=CreateEmbed::new().title("川柳検出Bot へようこそ！").description("このBotはメッセージから川柳（五・七・五）を自動検出してお知らせします。")
        .field("川柳の検出","普段の会話から五・七・五のリズムを自動で見つけます。特別な操作は不要です！",false)
        .field("「詠め」「詠むな」","「詠め」と発言するとサーバー内の川柳からランダムに一句詠みます。「詠むな」で直前の句を表示します。",false)
        .field("便利なコマンド","`/mute` `/unmute` — チャンネルごとの検出ON/OFF\n`/rank` — サーバー内ランキング\n`/detect off` — 自分の検出を無効化\n`/channel` — チャンネルタイプ別の設定\n`/doctor` — Bot動作の診断",false)
        .field("よくある質問","https://senryu-bot.u16.io/faq",false).colour(0x5865F2);
    if let Err(err)=channel.send_message(&ctx.http,CreateMessage::new().embed(embed)).await{tracing::warn!(guild_id=%guild.id,error=%err,"welcome message failed");}
}

async fn notify_join(ctx:&Context,guild:&Guild,state:&AppState){
    let Some(cid)=parse_channel(&state.config.admin.log_channel_id)else{return;};
    let _=cid.send_message(&ctx.http,CreateMessage::new().embed(CreateEmbed::new().title("サーバー参加").description(format!("**{}**\nID: `{}`\nメンバー: {}",guild.name,guild.id,guild.member_count)).colour(0x00ff00))).await;
}
async fn notify_leave(ctx:&Context,id:GuildId,full:Option<&Guild>,senryus:u64,optouts:u64,state:&AppState){
    let Some(cid)=parse_channel(&state.config.admin.log_channel_id)else{return;};let name=full.map(|g|g.name.clone()).unwrap_or_else(||"Unknown Guild".into());let _=cid.send_message(&ctx.http,CreateMessage::new().embed(CreateEmbed::new().title("サーバー退出").description(format!("**{name}**\nID: `{id}`\n削除した川柳: {senryus}\n削除した設定: {optouts}")).colour(0xff0000))).await;
}

fn spawn_daily_report(http:Arc<serenity::http::Http>,state:Arc<AppState>){
    let Some(cid)=parse_channel(&state.config.admin.report_channel_id)else{return;};tokio::spawn(async move{let mut tick=time::interval(Duration::from_secs(86400));tick.tick().await;loop{tick.tick().await;match state.db.db_stats().await{Ok(s)=>{let _=cid.send_message(&http,CreateMessage::new().embed(CreateEmbed::new().title("川柳検出Bot デイリーレポート").field("Total Senryus",s.senryu_count.to_string(),true).field("Muted Channels",s.muted_channel_count.to_string(),true).field("Opt Outs",s.opt_out_count.to_string(),true).field("Uptime",format!("{}s",state.metrics.uptime_seconds()),true))).await;},Err(e)=>tracing::error!(error=%e,"daily report DB stats failed")}}});
}
fn parse_channel(s:&str)->Option<ChannelId>{s.parse::<u64>().ok().map(ChannelId::new)}
