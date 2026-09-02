use crate::{backup::BackupManager, config::Config, crypto::Crypto, db::Database, detector::Detector, metrics::Metrics};
use parking_lot::{Mutex, RwLock};
use serenity::model::channel::ChannelType;
use std::{collections::{HashMap, HashSet}, sync::{Arc, atomic::{AtomicBool, Ordering}}, time::Instant};

pub struct AppState {
    pub config: Config,
    pub db: Database,
    pub crypto: Crypto,
    pub detector: Detector,
    pub metrics: Arc<Metrics>,
    pub backup: Option<Arc<BackupManager>>,
    channel_cache: RwLock<HashMap<String, HashMap<i16, bool>>>,
    mute_cache: RwLock<HashMap<String, bool>>,
    opt_out_cache: RwLock<HashMap<(String,String), Option<String>>>,
    welcome_sent: Mutex<HashSet<String>>,
    contact_cooldown: Mutex<HashMap<String, Instant>>,
    tasks_started: AtomicBool,
}

impl AppState {
    pub fn new(config: Config, db: Database, crypto: Crypto, detector: Detector, metrics: Arc<Metrics>) -> Arc<Self> {
        let backup = if config.backup.enabled && db.sqlite_path().is_some() {
            Some(Arc::new(BackupManager::new(config.backup.clone(), db.clone())))
        } else { None };
        Arc::new(Self {
            config, db, crypto, detector, metrics, backup,
            channel_cache: RwLock::new(HashMap::new()),
            mute_cache: RwLock::new(HashMap::new()),
            opt_out_cache: RwLock::new(HashMap::new()),
            welcome_sent: Mutex::new(HashSet::new()),
            contact_cooldown: Mutex::new(HashMap::new()),
            tasks_started: AtomicBool::new(false),
        })
    }

    pub async fn is_muted(&self, channel_id: &str) -> bool {
        if let Some(v) = self.mute_cache.read().get(channel_id) { return *v; }
        let v = self.db.is_muted(channel_id).await.unwrap_or(false);
        self.mute_cache.write().insert(channel_id.to_string(), v);
        v
    }

    pub async fn mute(&self, channel_id: &str, guild_id: &str) -> anyhow::Result<()> {
        self.db.mute(channel_id, guild_id).await?;
        self.mute_cache.write().insert(channel_id.to_string(), true);
        Ok(())
    }

    pub async fn unmute(&self, channel_id: &str) -> anyhow::Result<()> {
        self.db.unmute(channel_id).await?;
        self.mute_cache.write().insert(channel_id.to_string(), false);
        Ok(())
    }

    pub async fn opt_out_set_by(&self, guild_id: &str, user_id: &str) -> Option<String> {
        let key = (guild_id.to_string(), user_id.to_string());
        if let Some(v) = self.opt_out_cache.read().get(&key) { return v.clone(); }
        let v = self.db.opt_out_set_by(guild_id, user_id).await.unwrap_or(None);
        self.opt_out_cache.write().insert(key, v.clone());
        v
    }

    pub async fn set_opt_out(&self, guild_id: &str, user_id: &str, set_by: &str) -> anyhow::Result<()> {
        self.db.opt_out(guild_id, user_id, set_by).await?;
        self.opt_out_cache.write().insert((guild_id.into(), user_id.into()), Some(set_by.into()));
        Ok(())
    }

    pub async fn clear_opt_out(&self, guild_id: &str, user_id: &str) -> anyhow::Result<()> {
        self.db.opt_in(guild_id, user_id).await?;
        self.opt_out_cache.write().insert((guild_id.into(), user_id.into()), None);
        Ok(())
    }

    pub async fn channel_enabled(&self, guild_id: &str, kind: ChannelType) -> bool {
        let key = i16::from(u8::from(kind));
        if !default_channel_types().contains_key(&key) { return false; }
        if !self.channel_cache.read().contains_key(guild_id) {
            let overrides = self.db.channel_overrides(guild_id).await.unwrap_or_default();
            self.channel_cache.write().insert(guild_id.into(), overrides.into_iter().collect());
        }
        self.channel_cache.read().get(guild_id).and_then(|m| m.get(&key)).copied()
            .unwrap_or_else(|| *default_channel_types().get(&key).unwrap_or(&false))
    }

    pub async fn channel_settings(&self, guild_id: &str) -> HashMap<i16, bool> {
        let mut result = default_channel_types();
        for (k,v) in self.db.channel_overrides(guild_id).await.unwrap_or_default() { result.insert(k,v); }
        self.channel_cache.write().insert(guild_id.into(), result.iter().filter_map(|(k,v)| {
            if default_channel_types().get(k) == Some(v) { None } else { Some((*k,*v)) }
        }).collect());
        result
    }

    pub async fn toggle_channel_type(&self, guild_id: &str, kind: i16) -> anyhow::Result<bool> {
        let defaults = default_channel_types();
        let default = *defaults.get(&kind).unwrap_or(&false);
        let current = self.channel_settings(guild_id).await.get(&kind).copied().unwrap_or(default);
        let next = !current;
        let override_value = if next == default { None } else { Some(next) };
        self.db.set_channel_override(guild_id, kind, override_value).await?;
        self.channel_cache.write().remove(guild_id);
        Ok(next)
    }

    pub fn mark_welcome(&self, guild_id: &str) -> bool { self.welcome_sent.lock().insert(guild_id.to_string()) }
    pub fn clear_welcome(&self, guild_id: &str) { self.welcome_sent.lock().remove(guild_id); }

    pub fn start_tasks_once(&self) -> bool {
        !self.tasks_started.swap(true, Ordering::SeqCst)
    }

    pub fn contact_allowed(&self, user_id: &str) -> Result<(), u64> {
        const COOLDOWN: u64 = 300;
        let mut map = self.contact_cooldown.lock();
        if let Some(last) = map.get(user_id) {
            let elapsed = last.elapsed().as_secs();
            if elapsed < COOLDOWN { return Err(COOLDOWN - elapsed); }
        }
        map.insert(user_id.to_string(), Instant::now());
        Ok(())
    }
}

pub fn default_channel_types() -> HashMap<i16,bool> {
    HashMap::from([
        (i16::from(u8::from(ChannelType::Text)), true),
        (i16::from(u8::from(ChannelType::Voice)), true),
        (i16::from(u8::from(ChannelType::Stage)), true),
        (i16::from(u8::from(ChannelType::News)), false),
        (i16::from(u8::from(ChannelType::Forum)), false),
        (i16::from(u8::from(ChannelType::NewsThread)), true),
        (i16::from(u8::from(ChannelType::PublicThread)), true),
        (i16::from(u8::from(ChannelType::PrivateThread)), true),
    ])
}

pub fn channel_type_label(kind: i16) -> &'static str {
    if kind == i16::from(u8::from(ChannelType::Text)) { "テキストチャンネル" }
    else if kind == i16::from(u8::from(ChannelType::Voice)) { "ボイスチャンネル" }
    else if kind == i16::from(u8::from(ChannelType::Stage)) { "ステージチャンネル" }
    else if kind == i16::from(u8::from(ChannelType::News)) { "アナウンスチャンネル" }
    else if kind == i16::from(u8::from(ChannelType::Forum)) { "フォーラムチャンネル" }
    else if kind == i16::from(u8::from(ChannelType::NewsThread)) { "ニューススレッド" }
    else if kind == i16::from(u8::from(ChannelType::PublicThread)) { "公開スレッド" }
    else if kind == i16::from(u8::from(ChannelType::PrivateThread)) { "プライベートスレッド" }
    else { "不明" }
}
