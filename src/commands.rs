use crate::state::{channel_type_label, default_channel_types, AppState};
use serenity::{
    all::{
        ActionRowComponent, ButtonStyle, ChannelId, CommandDataOption, CommandDataOptionValue,
        CommandInteraction, CommandOptionType, ComponentInteraction, ComponentInteractionDataKind,
        Context, CreateActionRow, CreateButton, CreateCommand, CreateCommandOption, CreateEmbed,
        CreateEmbedFooter, CreateInputText, CreateInteractionResponse,
        CreateInteractionResponseMessage, CreateModal, CreateSelectMenu, CreateSelectMenuKind,
        CreateSelectMenuOption, GuildId, InputTextStyle, ModalInteraction, Permissions, UserId,
    },
    http::Http,
};
use std::sync::Arc;

const DELETE_SELECT: &str = "senryu_delete_select:";
const DELETE_CONFIRM: &str = "senryu_delete_confirm:";
const DELETE_CANCEL: &str = "senryu_delete_cancel";
const DELETE_PAGE: &str = "senryu_delete_page:";
const DELETE_PAGE_SIZE: i64 = 25;
const CHANNEL_TOGGLE: &str = "senryu_channel_toggle:";
const CONTACT_CATEGORY: &str = "senryu_contact_category";
const CONTACT_MODAL: &str = "senryu_contact_modal";
const CONTACT_REPLY: &str = "senryu_contact_reply:";
const CONTACT_REPLY_MODAL: &str = "senryu_contact_reply_modal:";

pub fn user_commands(contact_enabled: bool) -> Vec<CreateCommand> {
    let mut commands = vec![
        CreateCommand::new("mute")
            .description("このチャンネルでの川柳検出をミュートします")
            .default_member_permissions(Permissions::MANAGE_CHANNELS)
            .dm_permission(false),
        CreateCommand::new("unmute")
            .description("このチャンネルでの川柳検出のミュートを解除します")
            .default_member_permissions(Permissions::MANAGE_CHANNELS)
            .dm_permission(false),
        CreateCommand::new("rank")
            .description("ギルド内で詠んだ回数が多い人のランキングを表示します")
            .dm_permission(false),
        CreateCommand::new("delete")
            .description("自分または指定ユーザーの川柳を選択して削除します")
            .add_option(CreateCommandOption::new(CommandOptionType::User, "user", "削除対象のユーザー（管理者のみ他ユーザー可）").required(true))
            .dm_permission(false),
        CreateCommand::new("channel")
            .description("チャンネルタイプ別の川柳検出設定を変更します")
            .default_member_permissions(Permissions::ADMINISTRATOR)
            .dm_permission(false),
        CreateCommand::new("doctor")
            .description("このチャンネルでBotが正常に動作するか診断します")
            .dm_permission(false),
        CreateCommand::new("detect")
            .description("川柳検出のオン/オフを切り替えます")
            .set_options(vec![
                CreateCommandOption::new(CommandOptionType::SubCommand, "on", "自分の川柳検出を有効にします"),
                CreateCommandOption::new(CommandOptionType::SubCommand, "off", "自分の川柳検出を無効にします"),
                CreateCommandOption::new(CommandOptionType::SubCommand, "status", "現在の川柳検出設定を表示します"),
                CreateCommandOption::new(CommandOptionType::SubCommand, "ban", "指定ユーザーの検出を無効化します（管理者）")
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::User, "user", "対象ユーザー").required(true)),
                CreateCommandOption::new(CommandOptionType::SubCommand, "unban", "指定ユーザーの検出無効化を解除します（管理者）")
                    .add_sub_option(CreateCommandOption::new(CommandOptionType::User, "user", "対象ユーザー").required(true)),
                CreateCommandOption::new(CommandOptionType::SubCommand, "list", "検出無効化ユーザー一覧を表示します（管理者）"),
            ])
            .dm_permission(false),
    ];
    if contact_enabled {
        commands.push(
            CreateCommand::new("contact")
                .description("Bot管理者へお問い合わせを送ります")
                .default_member_permissions(Permissions::ADMINISTRATOR)
                .dm_permission(false),
        );
    }
    commands
}

pub fn admin_commands() -> Vec<CreateCommand> {
    vec![CreateCommand::new("admin")
        .description("Bot管理者向けコマンド")
        .set_options(vec![
            CreateCommandOption::new(CommandOptionType::SubCommand, "stats", "Bot統計情報を表示します"),
            CreateCommandOption::new(CommandOptionType::SubCommand, "backup", "SQLiteバックアップを作成します"),
            CreateCommandOption::new(CommandOptionType::SubCommandGroup, "contact-message", "/contact の追加メッセージを管理します")
                .set_sub_options(vec![
                    CreateCommandOption::new(CommandOptionType::SubCommand, "set", "追加メッセージを設定します")
                        .add_sub_option(CreateCommandOption::new(CommandOptionType::String, "message", "表示するメッセージ").required(true).max_length(1000)),
                    CreateCommandOption::new(CommandOptionType::SubCommand, "clear", "追加メッセージを削除します"),
                    CreateCommandOption::new(CommandOptionType::SubCommand, "show", "現在の追加メッセージを表示します"),
                ]),
        ])
        .dm_permission(false)]
}

pub async fn register(http: &Http, state: &AppState) {
    match http.create_global_commands(&user_commands(!state.config.admin.contact_channel_id.is_empty())).await {
        Ok(cmds) => tracing::info!(count=cmds.len(), "global commands registered"),
        Err(err) => tracing::error!(error=%err, "failed to register global commands"),
    }
    if let Ok(id) = state.config.admin.guild_id.parse::<u64>() {
        match http.create_guild_commands(GuildId::new(id), &admin_commands()).await {
            Ok(_) => tracing::info!(guild_id=id, "admin commands registered"),
            Err(err) => tracing::error!(error=%err, "failed to register admin commands"),
        }
    }
}

pub async fn handle_command(ctx: &Context, c: &CommandInteraction, state: Arc<AppState>) {
    state.metrics.command();
    let result = match c.data.name.as_str() {
        "mute" => mute(ctx, c, &state, true).await,
        "unmute" => mute(ctx, c, &state, false).await,
        "rank" => rank(ctx, c, &state).await,
        "delete" => delete_menu(ctx, c, &state).await,
        "channel" => channel(ctx, c, &state).await,
        "doctor" => doctor(ctx, c, &state).await,
        "detect" => detect(ctx, c, &state).await,
        "contact" => contact(ctx, c, &state).await,
        "admin" => admin(ctx, c, &state).await,
        _ => Ok(()),
    };
    if let Err(err) = result {
        state.metrics.error();
        tracing::error!(command=%c.data.name, error=%err, "command failed");
        let _ = ephemeral(ctx, c, "処理に失敗しました。`/doctor` も確認してください。").await;
    }
}

async fn mute(ctx: &Context, c: &CommandInteraction, state: &AppState, enable_mute: bool) -> anyhow::Result<()> {
    let Some(guild_id) = c.guild_id else { return ephemeral(ctx,c,"このコマンドはサーバー内でのみ使用できます").await; };
    if !has_manage_channels(c) { return ephemeral(ctx,c,"このコマンドはサーバー管理者またはチャンネル管理権限を持つユーザーのみ使用できます").await; }
    if enable_mute {
        state.mute(&c.channel_id.to_string(), &guild_id.to_string()).await?;
        normal(ctx,c,"このチャンネルでの川柳検出をミュートしました").await
    } else {
        state.unmute(&c.channel_id.to_string()).await?;
        normal(ctx,c,"このチャンネルでの川柳検出のミュートを解除しました").await
    }
}

async fn rank(ctx: &Context, c: &CommandInteraction, state: &AppState) -> anyhow::Result<()> {
    let Some(guild_id) = c.guild_id else { return ephemeral(ctx,c,"このコマンドはサーバー内でのみ使用できます").await; };
    let ranks = state.db.ranking(&guild_id.to_string()).await?;
    let stats = state.db.server_stats(&guild_id.to_string()).await?;
    let mut embed = CreateEmbed::new().title("サーバー内ランキング")
        .description(if stats.total_senryus == 0 { "まだ誰も詠んでいません".to_string() } else { format!("累計 **{}** 句 / **{}** 人の詠み手",stats.total_senryus,stats.unique_authors) });
    let medals = ["🥇","🥈","🥉","🎖️","🎖️"];
    for r in ranks {
        let name = guild_id.member(&ctx.http, UserId::new(r.author_id.parse().unwrap_or_default())).await
            .map(|m| m.display_name().to_string()).unwrap_or_else(|_| format!("<@{}>",r.author_id));
        let medal = medals.get(r.rank.saturating_sub(1)).copied().unwrap_or("🎖️");
        embed = embed.field(format!("{medal} 第{}位: {}回",r.rank,r.count),name,true);
    }
    c.create_response(&ctx.http, CreateInteractionResponse::Message(CreateInteractionResponseMessage::new().embed(embed))).await?;
    Ok(())
}

async fn delete_menu(ctx: &Context, c: &CommandInteraction, state: &AppState) -> anyhow::Result<()> {
    let Some(guild_id) = c.guild_id else { return ephemeral(ctx,c,"このコマンドはサーバー内でのみ使用できます").await; };
    let requester = c.user.id;
    let target = user_option(&c.data.options).unwrap_or(requester);
    if target != requester && !is_admin(c) { return ephemeral(ctx,c,"他のユーザーの川柳を削除する権限がありません").await; }
    let total = state.db.count_author(&guild_id.to_string(),&target.to_string()).await?;
    if total == 0 { return ephemeral(ctx,c,"削除できる川柳がありません").await; }
    let msg = delete_page_message(state,&guild_id.to_string(),target,0,total).await?.ephemeral(true);
    c.create_response(&ctx.http,CreateInteractionResponse::Message(msg)).await?;
    Ok(())
}

async fn delete_page_message(state:&AppState,guild_id:&str,target:UserId,page:i64,total:i64)->anyhow::Result<CreateInteractionResponseMessage> {
    let total_pages = ((total + DELETE_PAGE_SIZE - 1) / DELETE_PAGE_SIZE).max(1);
    let page = page.clamp(0,total_pages-1);
    let rows = state.db.get_author_page(&state.crypto,guild_id,&target.to_string(),DELETE_PAGE_SIZE,page*DELETE_PAGE_SIZE).await?;
    let opts = rows.into_iter().map(|s| {
        let raw = format!("{}{} {} {}",if s.spoiler{"🔒 "}else{""},s.kamigo,s.nakashichi,s.shimogo);
        CreateSelectMenuOption::new(truncate(&raw,100),s.id.to_string())
    }).collect();
    let menu = CreateSelectMenu::new(format!("{DELETE_SELECT}{}",target.get()),CreateSelectMenuKind::String{options:opts}).placeholder("川柳を選択");
    let content = if total_pages>1 {format!("削除する川柳を選んでください（{}/{}ページ, 全{}件）:",page+1,total_pages,total)} else {"削除する川柳を選んでください:".to_string()};
    let mut components=vec![CreateActionRow::SelectMenu(menu)];
    if total_pages>1 {
        components.push(CreateActionRow::Buttons(vec![
            CreateButton::new(format!("{DELETE_PAGE}{}:{}",target.get(),page-1)).label("◀ 前へ").style(ButtonStyle::Secondary).disabled(page==0),
            CreateButton::new(format!("{DELETE_PAGE}{}:{}",target.get(),page+1)).label("次へ ▶").style(ButtonStyle::Secondary).disabled(page>=total_pages-1),
        ]));
    }
    Ok(CreateInteractionResponseMessage::new().content(content).components(components))
}

async fn channel(ctx: &Context, c: &CommandInteraction, state: &AppState) -> anyhow::Result<()> {
    let Some(guild_id) = c.guild_id else { return ephemeral(ctx,c,"このコマンドはサーバー内でのみ使用できます").await; };
    if !is_admin(c) { return ephemeral(ctx,c,"このコマンドはサーバー管理者のみ使用できます").await; }
    let msg = channel_message(state,&guild_id.to_string()).await;
    c.create_response(&ctx.http,CreateInteractionResponse::Message(msg)).await?;
    Ok(())
}

async fn channel_message(state:&AppState,guild_id:&str)->CreateInteractionResponseMessage {
    let settings=state.channel_settings(guild_id).await;
    let order=channel_order();
    let desc=order.iter().map(|k|format!("{} {}",if *settings.get(k).unwrap_or(&false){"✅"}else{"❌"},channel_type_label(*k))).collect::<Vec<_>>().join("\n");
    let embed=CreateEmbed::new().title("チャンネルタイプ別 川柳検出設定").description(desc).colour(0x00bfff);
    let mut rows=Vec::new();
    for chunk in order.chunks(5){
        let buttons=chunk.iter().map(|k|CreateButton::new(format!("{CHANNEL_TOGGLE}{k}")).label(short_channel_label(*k)).style(if *settings.get(k).unwrap_or(&false){ButtonStyle::Success}else{ButtonStyle::Secondary})).collect();
        rows.push(CreateActionRow::Buttons(buttons));
    }
    CreateInteractionResponseMessage::new().embed(embed).components(rows).ephemeral(true)
}

async fn doctor(ctx:&Context,c:&CommandInteraction,state:&AppState)->anyhow::Result<()> {
    let Some(guild_id)=c.guild_id else{return ephemeral(ctx,c,"このコマンドはサーバー内でのみ使用できます").await;};
    let channel=c.channel_id.to_channel(&ctx.http).await?;
    let mut lines=Vec::new(); let mut bad=false;
    if let serenity::all::Channel::Guild(gc)=channel {
        let enabled=state.channel_enabled(&guild_id.to_string(),gc.kind).await;
        lines.push(format!("{} チャンネルタイプ「{}」は検出{}です",if enabled{"✅"}else{"❌"},gc.kind.name(),if enabled{"対象"}else{"対象外"})); bad|=!enabled;
        let muted=state.is_muted(&gc.id.to_string()).await;
        lines.push(if muted{"❌ このチャンネルはミュートされています — `/unmute` で解除できます".into()}else{"✅ このチャンネルはミュートされていません".into()}); bad|=muted;
        if let Some(parent)=gc.parent_id { let pm=state.is_muted(&parent.to_string()).await; lines.push(if pm{"❌ 親チャンネルがミュートされています".into()}else{"✅ 親チャンネルはミュートされていません".into()}); bad|=pm; }
    }
    let p=c.app_permissions.unwrap_or(Permissions::empty());
    for (flag,name,required) in [(Permissions::VIEW_CHANNEL,"チャンネルの閲覧",true),(Permissions::SEND_MESSAGES,"メッセージの送信",true),(Permissions::READ_MESSAGE_HISTORY,"メッセージ履歴の閲覧",true),(Permissions::EMBED_LINKS,"埋め込みリンク",false),(Permissions::USE_EXTERNAL_EMOJIS,"外部の絵文字の使用",false)] {
        let ok=p.contains(flag); lines.insert(0,format!("{} {}",if ok{"✅"}else if required{"❌"}else{"⚠️"},name)); if required&&!ok{bad=true;}
    }
    let opted=state.opt_out_set_by(&guild_id.to_string(),&c.user.id.to_string()).await.is_some();
    lines.push(if opted{"⚠️ あなたは川柳検出を無効にしています — `/detect on` で有効にできます".into()}else{"✅ あなたの川柳検出は有効です".into()});
    let embed=CreateEmbed::new().title(format!("診断結果: {}",if bad{"問題が見つかりました"}else{"問題ありません"})).description(lines.join("\n")).colour(if bad{0xff0000}else{0x00ff00});
    c.create_response(&ctx.http,CreateInteractionResponse::Message(CreateInteractionResponseMessage::new().embed(embed).ephemeral(true))).await?; Ok(())
}

async fn detect(ctx:&Context,c:&CommandInteraction,state:&AppState)->anyhow::Result<()> {
    let Some(guild_id)=c.guild_id else{return ephemeral(ctx,c,"このコマンドはサーバー内でのみ使用できます").await;};
    let Some(top)=c.data.options.first() else{return ephemeral(ctx,c,"サブコマンドを指定してください").await;};
    let gid=guild_id.to_string(); let uid=c.user.id.to_string();
    match (top.name.as_str(),&top.value) {
        ("on",_)=>{ if state.opt_out_set_by(&gid,&uid).await.as_deref()==Some("admin"){ephemeral(ctx,c,"管理者によって川柳検出が無効化されています。解除するにはサーバー管理者に連絡してください。").await}else{state.clear_opt_out(&gid,&uid).await?;ephemeral(ctx,c,"川柳検出を有効にしました ✅").await} }
        ("off",_)=>{ if state.opt_out_set_by(&gid,&uid).await.as_deref()==Some("admin"){ephemeral(ctx,c,"管理者によって検出が無効化されています").await}else{state.set_opt_out(&gid,&uid,"self").await?;ephemeral(ctx,c,"川柳検出を無効にしました ✅").await} }
        ("status",_)=>{ let off=state.opt_out_set_by(&gid,&uid).await.is_some();ephemeral(ctx,c,if off{"現在の設定: 川柳検出 **無効**"}else{"現在の設定: 川柳検出 **有効**"}).await }
        ("ban",CommandDataOptionValue::SubCommand(opts))=>{ if !is_admin(c){return ephemeral(ctx,c,"このコマンドはサーバー管理者のみ使用できます").await;} let Some(user)=user_option(opts)else{return ephemeral(ctx,c,"ユーザーを指定してください").await;}; state.set_opt_out(&gid,&user.to_string(),"admin").await?;ephemeral(ctx,c,&format!("<@{}> の川柳検出を無効化しました ✅",user.get())).await }
        ("unban",CommandDataOptionValue::SubCommand(opts))=>{ if !is_admin(c){return ephemeral(ctx,c,"このコマンドはサーバー管理者のみ使用できます").await;} let Some(user)=user_option(opts)else{return ephemeral(ctx,c,"ユーザーを指定してください").await;}; state.clear_opt_out(&gid,&user.to_string()).await?;ephemeral(ctx,c,&format!("<@{}> の川柳検出無効化を解除しました ✅",user.get())).await }
        ("list",_)=>{ if !is_admin(c){return ephemeral(ctx,c,"このコマンドはサーバー管理者のみ使用できます").await;} let rows=state.db.list_opt_outs(&gid).await?; let text=if rows.is_empty(){"川柳検出を無効化しているユーザーはいません".into()}else{rows.iter().take(25).map(|(u,s)|format!("- <@{u}> ({})",if s=="admin"{"管理者BAN"}else{"自己設定"})).collect::<Vec<_>>().join("\n")}; ephemeral(ctx,c,&text).await }
        _=>ephemeral(ctx,c,"不明なサブコマンドです").await,
    }
}

async fn contact(ctx:&Context,c:&CommandInteraction,state:&AppState)->anyhow::Result<()> {
    if state.config.admin.contact_channel_id.is_empty(){return ephemeral(ctx,c,"お問い合わせ機能は設定されていません").await;}
    if !is_admin(c){return ephemeral(ctx,c,"このコマンドはサーバー管理者のみ使用できます").await;}
    if let Err(sec)=state.contact_allowed(&c.user.id.to_string()){return ephemeral(ctx,c,&format!("お問い合わせのクールダウン中です。あと {}分{}秒 お待ちください",sec/60,sec%60)).await;}
    let extra=state.db.get_metadata("contact_additional_message").await?.unwrap_or_default();
    let mut desc="お困りの内容に近いカテゴリを選択してください。\n該当するものがない場合は「その他のお問い合わせ」からメッセージを送信できます。".to_string(); if !extra.is_empty(){desc.push_str("\n\n📌 ");desc.push_str(&extra);}
    let opts=vec![
        CreateSelectMenuOption::new("川柳が検出されない・精度が悪い","faq_detection"),
        CreateSelectMenuOption::new("短歌（5-7-5-7-7）は検出されますか？","faq_tanka"),
        CreateSelectMenuOption::new("個人チャット（DM）でBotが反応しません","faq_dm"),
        CreateSelectMenuOption::new("コマンドが応答しません","faq_command_error"),
        CreateSelectMenuOption::new("破調（字余り・字足らず）に対応してほしい","faq_hachou"),
        CreateSelectMenuOption::new("その他のお問い合わせ","other"),
    ];
    let menu=CreateSelectMenu::new(CONTACT_CATEGORY,CreateSelectMenuKind::String{options:opts}).placeholder("カテゴリを選択してください");
    let embed=CreateEmbed::new().title("お問い合わせ").description(desc).colour(0x5865F2).footer(CreateEmbedFooter::new("💡 Botの動作に問題がある場合は /doctor もお試しください"));
    c.create_response(&ctx.http,CreateInteractionResponse::Message(CreateInteractionResponseMessage::new().embed(embed).components(vec![CreateActionRow::SelectMenu(menu)]).ephemeral(true))).await?;Ok(())
}

async fn admin(ctx:&Context,c:&CommandInteraction,state:&AppState)->anyhow::Result<()> {
    let admin_guild=state.config.admin.guild_id.parse::<u64>().ok();
    if admin_guild!=c.guild_id.map(|g|g.get()) || !state.config.is_owner(c.user.id.get()){return ephemeral(ctx,c,"このコマンドはBot管理者のみ使用できます").await;}
    let Some(top)=c.data.options.first() else{return ephemeral(ctx,c,"サブコマンドを指定してください").await;};
    match (top.name.as_str(),&top.value){
        ("stats",_)=>{let s=state.db.db_stats().await?;let embed=CreateEmbed::new().title("Bot Statistics").field("Uptime",format!("{}s",state.metrics.uptime_seconds()),true).field("Database Driver",&state.config.database.driver,true).field("Total Senryus",s.senryu_count.to_string(),true).field("Muted Channels",s.muted_channel_count.to_string(),true).field("Opt Outs",s.opt_out_count.to_string(),true).field("Database Connected",s.connected.to_string(),true);c.create_response(&ctx.http,CreateInteractionResponse::Message(CreateInteractionResponseMessage::new().embed(embed).ephemeral(true))).await?;Ok(())}
        ("backup",_)=>{let Some(mgr)=&state.backup else{return ephemeral(ctx,c,"バックアップはSQLiteで backup.enabled=true の場合のみ利用できます").await;};let p=mgr.create_backup().await?;ephemeral(ctx,c,&format!("バックアップを作成しました: `{}`",p.file_name().unwrap_or_default().to_string_lossy())).await}
        ("contact-message",CommandDataOptionValue::SubCommandGroup(group))=>{let Some(sub)=group.first()else{return ephemeral(ctx,c,"サブコマンドを指定してください").await;};match (&*sub.name,&sub.value){("set",CommandDataOptionValue::SubCommand(opts))=>{let value=string_option(opts,"message").unwrap_or_default();state.db.set_metadata("contact_additional_message",&value).await?;ephemeral(ctx,c,"追加メッセージを設定しました ✅").await},("clear",_)=>{state.db.clear_metadata("contact_additional_message").await?;ephemeral(ctx,c,"追加メッセージを削除しました ✅").await},("show",_)=>{let v=state.db.get_metadata("contact_additional_message").await?.unwrap_or_default();ephemeral(ctx,c,if v.is_empty(){"追加メッセージは設定されていません"}else{&v}).await},_=>ephemeral(ctx,c,"不明なサブコマンドです").await}}
        _=>ephemeral(ctx,c,"不明なサブコマンドです").await,
    }
}

pub async fn handle_component(ctx:&Context,co:&ComponentInteraction,state:Arc<AppState>){
    let result=if co.data.custom_id.starts_with(CHANNEL_TOGGLE){component_channel(ctx,co,&state).await}
        else if co.data.custom_id.starts_with(DELETE_SELECT){component_delete_select(ctx,co,&state).await}
        else if co.data.custom_id.starts_with(DELETE_CONFIRM){component_delete_confirm(ctx,co,&state).await}
        else if co.data.custom_id.starts_with(DELETE_PAGE){component_delete_page(ctx,co,&state).await}
        else if co.data.custom_id==DELETE_CANCEL{component_update(ctx,co,"削除をキャンセルしました",vec![]).await}
        else if co.data.custom_id==CONTACT_CATEGORY{component_contact_category(ctx,co).await}
        else if co.data.custom_id.starts_with(CONTACT_REPLY){component_contact_reply(ctx,co).await}
        else{Ok(())};
    if let Err(err)=result{state.metrics.error();tracing::error!(error=%err,"component interaction failed");}
}

async fn component_channel(ctx:&Context,co:&ComponentInteraction,state:&AppState)->anyhow::Result<()> {
    if !component_admin(co){return component_ephemeral(ctx,co,"このボタンはサーバー管理者のみ使用できます").await;}
    let Some(gid)=co.guild_id else{return Ok(());}; let kind=co.data.custom_id.trim_start_matches(CHANNEL_TOGGLE).parse::<i16>()?;state.toggle_channel_type(&gid.to_string(),kind).await?;
    let msg=channel_message(state,&gid.to_string()).await;co.create_response(&ctx.http,CreateInteractionResponse::UpdateMessage(msg.ephemeral(false))).await?;Ok(())
}


async fn component_delete_page(ctx:&Context,co:&ComponentInteraction,state:&AppState)->anyhow::Result<()> {
    let Some(gid)=co.guild_id else{return Ok(());};
    let raw=co.data.custom_id.trim_start_matches(DELETE_PAGE);
    let mut parts=raw.splitn(2,':');
    let target=parts.next().and_then(|v|v.parse::<u64>().ok()).map(UserId::new).unwrap_or(co.user.id);
    let page=parts.next().and_then(|v|v.parse::<i64>().ok()).unwrap_or(0);
    if target!=co.user.id&&!component_admin(co){return component_ephemeral(ctx,co,"他のユーザーの削除操作を行う権限がありません").await;}
    let total=state.db.count_author(&gid.to_string(),&target.to_string()).await?;
    if total==0{return component_update(ctx,co,"削除できる川柳がありません",vec![]).await;}
    let msg=delete_page_message(state,&gid.to_string(),target,page,total).await?;
    co.create_response(&ctx.http,CreateInteractionResponse::UpdateMessage(msg)).await?;Ok(())
}

async fn component_delete_select(ctx:&Context,co:&ComponentInteraction,state:&AppState)->anyhow::Result<()> {
    let Some(gid)=co.guild_id else{return Ok(());}; let values=match &co.data.kind{ComponentInteractionDataKind::StringSelect{values}=>values,_=>return Ok(())};let Some(id)=values.first().and_then(|v|v.parse::<i64>().ok())else{return Ok(());};let Some(s)=state.db.get_senryu(&state.crypto,id,&gid.to_string()).await? else{return component_update(ctx,co,"川柳が見つかりませんでした",vec![]).await;};
    let target=co.data.custom_id.trim_start_matches(DELETE_SELECT).parse::<u64>().unwrap_or(co.user.id.get());if s.author_id!=co.user.id.to_string() && (!component_admin(co)||s.author_id!=target.to_string()){return component_update(ctx,co,"この川柳を削除する権限がありません",vec![]).await;}
    let text=if s.spoiler{format!("||「{} {} {}」||を削除しますか？",s.kamigo,s.nakashichi,s.shimogo)}else{format!("「{} {} {}」を削除しますか？",s.kamigo,s.nakashichi,s.shimogo)};
    let buttons=vec![CreateButton::new(format!("{DELETE_CONFIRM}{id}")).label("削除する").style(ButtonStyle::Danger),CreateButton::new(DELETE_CANCEL).label("キャンセル").style(ButtonStyle::Secondary)];component_update(ctx,co,&text,vec![CreateActionRow::Buttons(buttons)]).await
}

async fn component_delete_confirm(ctx:&Context,co:&ComponentInteraction,state:&AppState)->anyhow::Result<()> {
    let Some(gid)=co.guild_id else{return Ok(());};let id=co.data.custom_id.trim_start_matches(DELETE_CONFIRM).parse::<i64>()?;let Some(s)=state.db.get_senryu(&state.crypto,id,&gid.to_string()).await? else{return component_update(ctx,co,"川柳が見つかりませんでした（既に削除された可能性があります）",vec![]).await;};if s.author_id!=co.user.id.to_string()&&!component_admin(co){return component_update(ctx,co,"この川柳を削除する権限がありません",vec![]).await;}state.db.delete_senryu(id,&gid.to_string()).await?;let text=if s.spoiler{format!("||「{} {} {}」||を削除しました",s.kamigo,s.nakashichi,s.shimogo)}else{format!("「{} {} {}」を削除しました",s.kamigo,s.nakashichi,s.shimogo)};component_update(ctx,co,&text,vec![]).await
}

async fn component_contact_category(ctx:&Context,co:&ComponentInteraction)->anyhow::Result<()> {
    let values=match &co.data.kind{ComponentInteractionDataKind::StringSelect{values}=>values,_=>return Ok(())};let Some(v)=values.first()else{return Ok(());};
    if v.starts_with("faq_"){return component_update(ctx,co,"FAQ: https://senryu-bot.u16.io/faq",vec![]).await;}
    let modal=CreateModal::new(CONTACT_MODAL,"その他のお問い合わせ").components(vec![CreateActionRow::InputText(CreateInputText::new(InputTextStyle::Short,"件名","contact_subject").max_length(100)),CreateActionRow::InputText(CreateInputText::new(InputTextStyle::Paragraph,"内容","contact_message").max_length(1000))]);co.create_response(&ctx.http,CreateInteractionResponse::Modal(modal)).await?;Ok(())
}

async fn component_contact_reply(ctx:&Context,co:&ComponentInteraction)->anyhow::Result<()> {
    let target=co.data.custom_id.trim_start_matches(CONTACT_REPLY);let modal=CreateModal::new(format!("{CONTACT_REPLY_MODAL}{target}"),"お問い合わせへ返信").components(vec![CreateActionRow::InputText(CreateInputText::new(InputTextStyle::Paragraph,"返信内容","reply_message").max_length(1500))]);co.create_response(&ctx.http,CreateInteractionResponse::Modal(modal)).await?;Ok(())
}

pub async fn handle_modal(ctx:&Context,m:&ModalInteraction,state:Arc<AppState>){
    let r=if m.data.custom_id==CONTACT_MODAL{submit_contact(ctx,m,&state).await}else if m.data.custom_id.starts_with(CONTACT_REPLY_MODAL){submit_contact_reply(ctx,m,&state).await}else{Ok(())};if let Err(err)=r{state.metrics.error();tracing::error!(error=%err,"modal interaction failed");}
}

async fn submit_contact(ctx:&Context,m:&ModalInteraction,state:&AppState)->anyhow::Result<()> {
    let subject=modal_value(m,"contact_subject");let body=modal_value(m,"contact_message");let Some(cid)=state.config.admin.contact_channel_id.parse::<u64>().ok().map(ChannelId::new)else{return modal_ephemeral(ctx,m,"お問い合わせ先が設定されていません").await;};let guild=m.guild_id.map(|x|x.to_string()).unwrap_or_default();let embed=CreateEmbed::new().title("お問い合わせ").field("件名",&subject,false).field("内容",&body,false).field("送信者",format!("<@{}> ({})",m.user.id,m.user.id),false).field("サーバーID",guild,false).colour(0x5865F2);let button=CreateButton::new(format!("{CONTACT_REPLY}{}",m.user.id.get())).label("返信").style(ButtonStyle::Primary);cid.send_message(&ctx.http,serenity::all::CreateMessage::new().embed(embed).components(vec![CreateActionRow::Buttons(vec![button])])).await?;modal_ephemeral(ctx,m,"お問い合わせを送信しました ✅").await
}

async fn submit_contact_reply(ctx:&Context,m:&ModalInteraction,state:&AppState)->anyhow::Result<()> {
    if !state.config.is_owner(m.user.id.get()){return modal_ephemeral(ctx,m,"Bot管理者のみ返信できます").await;}let uid=m.data.custom_id.trim_start_matches(CONTACT_REPLY_MODAL).parse::<u64>()?;let body=modal_value(m,"reply_message");match UserId::new(uid).create_dm_channel(&ctx.http).await{Ok(dm)=>{dm.send_message(&ctx.http,serenity::all::CreateMessage::new().content(format!("川柳検出Bot 管理者からの返信です。\n\n{body}"))).await?;modal_ephemeral(ctx,m,"返信を送信しました ✅").await},Err(_)=>modal_ephemeral(ctx,m,"DMを送信できませんでした").await}
}

fn modal_value(m:&ModalInteraction,id:&str)->String{for row in &m.data.components{for c in &row.components{if let ActionRowComponent::InputText(t)=c{if t.custom_id==id{return t.value.clone().unwrap_or_default();}}}}String::new()}

async fn ephemeral(ctx:&Context,c:&CommandInteraction,text:&str)->anyhow::Result<()>{c.create_response(&ctx.http,CreateInteractionResponse::Message(CreateInteractionResponseMessage::new().content(text).ephemeral(true))).await?;Ok(())}
async fn normal(ctx:&Context,c:&CommandInteraction,text:&str)->anyhow::Result<()>{c.create_response(&ctx.http,CreateInteractionResponse::Message(CreateInteractionResponseMessage::new().content(text))).await?;Ok(())}
async fn component_ephemeral(ctx:&Context,c:&ComponentInteraction,text:&str)->anyhow::Result<()>{c.create_response(&ctx.http,CreateInteractionResponse::Message(CreateInteractionResponseMessage::new().content(text).ephemeral(true))).await?;Ok(())}
async fn component_update(ctx:&Context,c:&ComponentInteraction,text:&str,components:Vec<CreateActionRow>)->anyhow::Result<()>{c.create_response(&ctx.http,CreateInteractionResponse::UpdateMessage(CreateInteractionResponseMessage::new().content(text).components(components))).await?;Ok(())}
async fn modal_ephemeral(ctx:&Context,m:&ModalInteraction,text:&str)->anyhow::Result<()>{m.create_response(&ctx.http,CreateInteractionResponse::Message(CreateInteractionResponseMessage::new().content(text).ephemeral(true))).await?;Ok(())}

fn has_manage_channels(c:&CommandInteraction)->bool{c.member.as_ref().and_then(|m|m.permissions).is_some_and(|p|p.intersects(Permissions::ADMINISTRATOR|Permissions::MANAGE_CHANNELS))}
fn is_admin(c:&CommandInteraction)->bool{c.member.as_ref().and_then(|m|m.permissions).is_some_and(|p|p.contains(Permissions::ADMINISTRATOR))}
fn component_admin(c:&ComponentInteraction)->bool{c.member.as_ref().and_then(|m|m.permissions).is_some_and(|p|p.contains(Permissions::ADMINISTRATOR))}
fn user_option(opts:&[CommandDataOption])->Option<UserId>{opts.iter().find_map(|o|match o.value{CommandDataOptionValue::User(id)=>Some(id),_=>None})}
fn string_option(opts:&[CommandDataOption],name:&str)->Option<String>{opts.iter().find_map(|o|if o.name==name{if let CommandDataOptionValue::String(ref s)=o.value{Some(s.clone())}else{None}}else{None})}
fn truncate(s:&str,n:usize)->String{let mut out=s.chars().take(n).collect::<String>();if s.chars().count()>n{out.push_str("…");}out}
fn channel_order()->Vec<i16>{let d=default_channel_types();let wanted=[serenity::all::ChannelType::Text,serenity::all::ChannelType::Voice,serenity::all::ChannelType::Stage,serenity::all::ChannelType::News,serenity::all::ChannelType::Forum,serenity::all::ChannelType::NewsThread,serenity::all::ChannelType::PublicThread,serenity::all::ChannelType::PrivateThread];wanted.into_iter().map(|x|i16::from(u8::from(x))).filter(|x|d.contains_key(x)).collect()}
fn short_channel_label(k:i16)->&'static str{let ct=|v|i16::from(u8::from(v));if k==ct(serenity::all::ChannelType::Text){"テキスト"}else if k==ct(serenity::all::ChannelType::Voice){"ボイス"}else if k==ct(serenity::all::ChannelType::Stage){"ステージ"}else if k==ct(serenity::all::ChannelType::News){"アナウンス"}else if k==ct(serenity::all::ChannelType::Forum){"フォーラム"}else if k==ct(serenity::all::ChannelType::NewsThread){"ニュース"}else if k==ct(serenity::all::ChannelType::PublicThread){"公開"}else if k==ct(serenity::all::ChannelType::PrivateThread){"プライベート"}else{"不明"}}
