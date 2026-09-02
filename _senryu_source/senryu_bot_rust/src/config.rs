
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::{env, fs, path::PathBuf};

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    pub discord: DiscordConfig,
    pub database: DatabaseConfig,
    pub log: LogConfig,
    pub admin: AdminConfig,
    pub server: ServerConfig,
    pub backup: BackupConfig,
    pub encryption: EncryptionConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            discord: DiscordConfig::default(),
            database: DatabaseConfig::default(),
            log: LogConfig::default(),
            admin: AdminConfig::default(),
            server: ServerConfig::default(),
            backup: BackupConfig::default(),
            encryption: EncryptionConfig::default(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct DiscordConfig {
    #[serde(skip)]
    pub token: String,
    pub playing: String,
    pub welcome_enabled: bool,
}

impl Default for DiscordConfig {
    fn default() -> Self {
        Self {
            token: String::new(),
            playing: String::new(),
            welcome_enabled: true,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct DatabaseConfig {
    pub driver: String,
    pub path: String,
    pub dsn: String,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            driver: "sqlite3".into(),
            path: "data/senryu.db".into(),
            dsn: String::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct LogConfig {
    pub level: String,
    pub format: String,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: "info".into(),
            format: "text".into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Default)]
#[serde(default)]
pub struct AdminConfig {
    pub owner_ids: Vec<String>,
    pub guild_id: String,
    pub log_channel_id: String,
    pub report_channel_id: String,
    pub contact_channel_id: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            host: "127.0.0.1".into(),
            port: 9090,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct BackupConfig {
    pub enabled: bool,
    pub interval_hour: u64,
    pub path: String,
    pub max_backups: usize,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            interval_hour: 24,
            path: "data/backups".into(),
            max_backups: 7,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Default)]
#[serde(default)]
pub struct EncryptionConfig {
    pub key: String,
}

impl Config {
    pub fn load(token: String) -> Result<Self> {
        let explicit = env::var("SENRYU_BOT_CONFIG")
            .ok()
            .or_else(|| env::var("FINDSENRYU_CONFIG").ok());

        let candidates = explicit
            .map(|p| vec![PathBuf::from(p)])
            .unwrap_or_else(|| vec![PathBuf::from("senryu_bot.toml"), PathBuf::from("config.toml")]);

        let mut conf = Config::default();
        for path in candidates {
            if path.exists() {
                let raw = fs::read_to_string(&path)
                    .with_context(|| format!("failed to read {}", path.display()))?;
                conf = toml::from_str(&raw)
                    .with_context(|| format!("failed to parse {}", path.display()))?;
                break;
            }
        }

        conf.apply_env();
        conf.discord.token = token.trim().to_string();

        if conf.discord.token.is_empty() {
            bail!("Discord Bot token is empty");
        }

        match conf.database.driver.as_str() {
            "sqlite3" | "sqlite" => {
                if conf.database.path.trim().is_empty() {
                    conf.database.path = "data/senryu.db".into();
                }
            }
            "postgres" | "postgresql" => {
                if conf.database.dsn.trim().is_empty() {
                    bail!("database.driver=postgres requires database.dsn");
                }
            }
            other => bail!("unsupported database driver: {other}"),
        }

        if conf.backup.max_backups == 0 {
            conf.backup.max_backups = 1;
        }
        if conf.backup.interval_hour == 0 {
            conf.backup.interval_hour = 24;
        }

        Ok(conf)
    }

    fn apply_env(&mut self) {
        let get = |suffix: &str| {
            env::var(format!("SENRYU_BOT_{suffix}"))
                .ok()
                .or_else(|| env::var(format!("FINDSENRYU_{suffix}")).ok())
        };

        if let Some(v) = get("DISCORD_PLAYING") {
            self.discord.playing = v;
        }
        if let Some(v) = get("DISCORD_WELCOME_ENABLED") {
            self.discord.welcome_enabled = parse_bool(&v, self.discord.welcome_enabled);
        }
        if let Some(v) = get("DATABASE_DRIVER") {
            self.database.driver = v;
        }
        if let Some(v) = get("DATABASE_PATH") {
            self.database.path = v;
        }
        if let Some(v) = get("DATABASE_DSN") {
            self.database.dsn = v;
        }
        if let Some(v) = get("LOG_LEVEL") {
            self.log.level = v;
        }
        if let Some(v) = get("LOG_FORMAT") {
            self.log.format = v;
        }
        if let Some(v) = get("ADMIN_OWNER_IDS") {
            self.admin.owner_ids = v
                .split(',')
                .map(str::trim)
                .filter(|x| !x.is_empty())
                .map(str::to_string)
                .collect();
        }
        if let Some(v) = get("ADMIN_GUILD_ID") {
            self.admin.guild_id = v;
        }
        if let Some(v) = get("ADMIN_LOG_CHANNEL_ID") {
            self.admin.log_channel_id = v;
        }
        if let Some(v) = get("ADMIN_REPORT_CHANNEL_ID") {
            self.admin.report_channel_id = v;
        }
        if let Some(v) = get("ADMIN_CONTACT_CHANNEL_ID") {
            self.admin.contact_channel_id = v;
        }
        if let Some(v) = get("SERVER_ENABLED") {
            self.server.enabled = parse_bool(&v, self.server.enabled);
        }
        if let Some(v) = get("SERVER_HOST") {
            self.server.host = v;
        }
        if let Some(v) = get("SERVER_PORT") {
            if let Ok(port) = v.parse() {
                self.server.port = port;
            }
        }
        if let Some(v) = get("BACKUP_ENABLED") {
            self.backup.enabled = parse_bool(&v, self.backup.enabled);
        }
        if let Some(v) = get("BACKUP_INTERVAL_HOUR") {
            if let Ok(n) = v.parse() {
                self.backup.interval_hour = n;
            }
        }
        if let Some(v) = get("BACKUP_PATH") {
            self.backup.path = v;
        }
        if let Some(v) = get("BACKUP_MAX_BACKUPS") {
            if let Ok(n) = v.parse() {
                self.backup.max_backups = n;
            }
        }
        if let Some(v) = get("ENCRYPTION_KEY") {
            self.encryption.key = v;
        }
    }

    pub fn is_owner(&self, id: u64) -> bool {
        let id = id.to_string();
        self.admin.owner_ids.iter().any(|x| x == &id)
    }
}

fn parse_bool(v: &str, default: bool) -> bool {
    match v.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => true,
        "0" | "false" | "no" | "off" => false,
        _ => default,
    }
}
