
use crate::{
    crypto::Crypto,
    models::{DbStats, RankEntry, Senryu, ServerStats},
};
use anyhow::{Context, Result};
use rand::Rng;
use sqlx::{
    postgres::{PgPool, PgPoolOptions},
    sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions},
    Row,
};
use std::{path::Path, str::FromStr};

#[derive(Clone)]
pub enum Database {
    Sqlite { pool: SqlitePool, path: String },
    Postgres { pool: PgPool },
}

impl Database {
    pub async fn connect(driver: &str, path: &str, dsn: &str) -> Result<Self> {
        match driver {
            "sqlite3" | "sqlite" => {
                if let Some(parent) = Path::new(path).parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }
                let opts = SqliteConnectOptions::from_str(&format!("sqlite://{path}"))?
                    .create_if_missing(true)
                    .foreign_keys(true);
                let pool = SqlitePoolOptions::new()
                    .max_connections(8)
                    .connect_with(opts)
                    .await
                    .context("failed to open SQLite database")?;
                let db = Self::Sqlite {
                    pool,
                    path: path.to_string(),
                };
                db.migrate().await?;
                Ok(db)
            }
            "postgres" | "postgresql" => {
                let pool = PgPoolOptions::new()
                    .max_connections(12)
                    .connect(dsn)
                    .await
                    .context("failed to connect PostgreSQL database")?;
                let db = Self::Postgres { pool };
                db.migrate().await?;
                Ok(db)
            }
            other => anyhow::bail!("unsupported database driver: {other}"),
        }
    }

    async fn migrate(&self) -> Result<()> {
        match self {
            Self::Sqlite { pool, .. } => {
                for sql in sqlite_schema() {
                    sqlx::query(*sql).execute(pool).await?;
                }
                let columns = sqlx::query("PRAGMA table_info(detection_opt_outs)").fetch_all(pool).await?;
                let has_set_by = columns.iter().any(|r| r.try_get::<String,_>("name").ok().as_deref() == Some("set_by"));
                if !has_set_by {
                    sqlx::query("ALTER TABLE detection_opt_outs ADD COLUMN set_by TEXT NOT NULL DEFAULT 'self'").execute(pool).await?;
                }
            }
            Self::Postgres { pool } => {
                for sql in postgres_schema() {
                    sqlx::query(*sql).execute(pool).await?;
                }
                sqlx::query("ALTER TABLE detection_opt_outs ADD COLUMN IF NOT EXISTS set_by TEXT NOT NULL DEFAULT 'self'").execute(pool).await?;
            }
        }
        Ok(())
    }

    pub fn sqlite_path(&self) -> Option<&str> {
        match self {
            Self::Sqlite { path, .. } => Some(path),
            _ => None,
        }
    }

    pub async fn create_senryu(
        &self,
        crypto: &Crypto,
        server_id: &str,
        author_id: &str,
        parts: &[String; 3],
        spoiler: bool,
    ) -> Result<Senryu> {
        let k = crypto.encrypt(&parts[0])?;
        let n = crypto.encrypt(&parts[1])?;
        let s = crypto.encrypt(&parts[2])?;
        let now = chrono::Utc::now().timestamp_millis();

        let id: i64 = match self {
            Self::Sqlite { pool, .. } => {
                let row = sqlx::query(
                    "INSERT INTO senryus(server_id,author_id,kamigo,nakasichi,simogo,spoiler,created_at)
                     VALUES($1,$2,$3,$4,$5,$6,CURRENT_TIMESTAMP) RETURNING id",
                )
                .bind(server_id)
                .bind(author_id)
                .bind(&k)
                .bind(&n)
                .bind(&s)
                .bind(spoiler)
                .fetch_one(pool)
                .await?;
                row.try_get("id")?
            }
            Self::Postgres { pool } => {
                let row = sqlx::query(
                    "INSERT INTO senryus(server_id,author_id,kamigo,nakasichi,simogo,spoiler,created_at)
                     VALUES($1,$2,$3,$4,$5,$6,CURRENT_TIMESTAMP) RETURNING id",
                )
                .bind(server_id)
                .bind(author_id)
                .bind(&k)
                .bind(&n)
                .bind(&s)
                .bind(spoiler)
                .fetch_one(pool)
                .await?;
                i64::from(row.try_get::<i32,_>("id")?)
            }
        };

        Ok(Senryu {
            id,
            server_id: server_id.to_string(),
            author_id: author_id.to_string(),
            kamigo: parts[0].clone(),
            nakashichi: parts[1].clone(),
            shimogo: parts[2].clone(),
            spoiler,
            created_at: now,
        })
    }

    pub async fn get_last_senryu(&self, crypto: &Crypto, server_id: &str) -> Result<Option<Senryu>> {
        match self {
            Self::Sqlite { pool, .. } => {
                let row = sqlx::query(
                    "SELECT id,server_id,author_id,kamigo,nakasichi,simogo,spoiler
                     FROM senryus WHERE server_id=$1 ORDER BY id DESC LIMIT 1",
                )
                .bind(server_id)
                .fetch_optional(pool)
                .await?;
                row.map(|r| decode_sqlite_row(r, crypto)).transpose()
            }
            Self::Postgres { pool } => {
                let row = sqlx::query(
                    "SELECT id,server_id,author_id,kamigo,nakasichi,simogo,spoiler
                     FROM senryus WHERE server_id=$1 ORDER BY id DESC LIMIT 1",
                )
                .bind(server_id)
                .fetch_optional(pool)
                .await?;
                row.map(|r| decode_pg_row(r, crypto)).transpose()
            }
        }
    }

    pub async fn get_senryu(&self, crypto: &Crypto, id: i64, server_id: &str) -> Result<Option<Senryu>> {
        match self {
            Self::Sqlite { pool, .. } => {
                let row = sqlx::query(
                    "SELECT id,server_id,author_id,kamigo,nakasichi,simogo,spoiler
                     FROM senryus WHERE id=$1 AND server_id=$2",
                )
                .bind(id)
                .bind(server_id)
                .fetch_optional(pool)
                .await?;
                row.map(|r| decode_sqlite_row(r, crypto)).transpose()
            }
            Self::Postgres { pool } => {
                let row = sqlx::query(
                    "SELECT id,server_id,author_id,kamigo,nakasichi,simogo,spoiler
                     FROM senryus WHERE id=$1 AND server_id=$2",
                )
                .bind(id)
                .bind(server_id)
                .fetch_optional(pool)
                .await?;
                row.map(|r| decode_pg_row(r, crypto)).transpose()
            }
        }
    }

    pub async fn get_author_page(
        &self,
        crypto: &Crypto,
        server_id: &str,
        author_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Senryu>> {
        match self {
            Self::Sqlite { pool, .. } => {
                let rows = sqlx::query(
                    "SELECT id,server_id,author_id,kamigo,nakasichi,simogo,spoiler
                     FROM senryus WHERE server_id=$1 AND author_id=$2
                     ORDER BY id DESC LIMIT $3 OFFSET $4",
                )
                .bind(server_id)
                .bind(author_id)
                .bind(limit)
                .bind(offset)
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(|r| decode_sqlite_row(r, crypto)).collect()
            }
            Self::Postgres { pool } => {
                let rows = sqlx::query(
                    "SELECT id,server_id,author_id,kamigo,nakasichi,simogo,spoiler
                     FROM senryus WHERE server_id=$1 AND author_id=$2
                     ORDER BY id DESC LIMIT $3 OFFSET $4",
                )
                .bind(server_id)
                .bind(author_id)
                .bind(limit)
                .bind(offset)
                .fetch_all(pool)
                .await?;
                rows.into_iter().map(|r| decode_pg_row(r, crypto)).collect()
            }
        }
    }

    pub async fn count_author(&self, server_id: &str, author_id: &str) -> Result<i64> {
        match self {
            Self::Sqlite { pool, .. } => {
                Ok(sqlx::query_scalar::<_, i64>(
                    "SELECT COUNT(*) FROM senryus WHERE server_id=$1 AND author_id=$2",
                )
                .bind(server_id)
                .bind(author_id)
                .fetch_one(pool)
                .await?)
            }
            Self::Postgres { pool } => {
                Ok(sqlx::query_scalar::<_, i64>(
                    "SELECT COUNT(*) FROM senryus WHERE server_id=$1 AND author_id=$2",
                )
                .bind(server_id)
                .bind(author_id)
                .fetch_one(pool)
                .await?)
            }
        }
    }

    pub async fn delete_senryu(&self, id: i64, server_id: &str) -> Result<bool> {
        let affected = match self {
            Self::Sqlite { pool, .. } => sqlx::query("DELETE FROM senryus WHERE id=$1 AND server_id=$2")
                .bind(id)
                .bind(server_id)
                .execute(pool)
                .await?
                .rows_affected(),
            Self::Postgres { pool } => sqlx::query("DELETE FROM senryus WHERE id=$1 AND server_id=$2")
                .bind(id)
                .bind(server_id)
                .execute(pool)
                .await?
                .rows_affected(),
        };
        Ok(affected > 0)
    }

    pub async fn random_three(&self, crypto: &Crypto, server_id: &str) -> Result<Vec<Senryu>> {
        let count = self.count_non_spoiler(server_id).await?;
        if count == 0 {
            return Ok(Vec::new());
        }

        let offsets: Vec<i64> = {
            let mut rng = rand::rng();
            (0..3).map(|_| rng.random_range(0..count)).collect()
        };
        let mut out = Vec::with_capacity(3);
        for offset in offsets {
            let item = match self {
                Self::Sqlite { pool, .. } => {
                    let row = sqlx::query(
                        "SELECT id,server_id,author_id,kamigo,nakasichi,simogo,spoiler
                         FROM senryus WHERE server_id=$1 AND spoiler=FALSE LIMIT 1 OFFSET $2",
                    )
                    .bind(server_id)
                    .bind(offset)
                    .fetch_one(pool)
                    .await?;
                    decode_sqlite_row(row, crypto)?
                }
                Self::Postgres { pool } => {
                    let row = sqlx::query(
                        "SELECT id,server_id,author_id,kamigo,nakasichi,simogo,spoiler
                         FROM senryus WHERE server_id=$1 AND spoiler=FALSE LIMIT 1 OFFSET $2",
                    )
                    .bind(server_id)
                    .bind(offset)
                    .fetch_one(pool)
                    .await?;
                    decode_pg_row(row, crypto)?
                }
            };
            out.push(item);
        }
        Ok(out)
    }

    async fn count_non_spoiler(&self, server_id: &str) -> Result<i64> {
        match self {
            Self::Sqlite { pool, .. } => Ok(sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM senryus WHERE server_id=$1 AND spoiler=FALSE",
            )
            .bind(server_id)
            .fetch_one(pool)
            .await?),
            Self::Postgres { pool } => Ok(sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM senryus WHERE server_id=$1 AND spoiler=FALSE",
            )
            .bind(server_id)
            .fetch_one(pool)
            .await?),
        }
    }

    pub async fn ranking(&self, server_id: &str) -> Result<Vec<RankEntry>> {
        let pairs: Vec<(String, i64)> = match self {
            Self::Sqlite { pool, .. } => {
                let rows = sqlx::query(
                    "SELECT author_id, COUNT(*) AS count FROM senryus
                     WHERE server_id=$1 GROUP BY author_id ORDER BY count DESC",
                )
                .bind(server_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter()
                    .map(|r| Ok((r.try_get("author_id")?, r.try_get("count")?)))
                    .collect::<Result<Vec<_>, sqlx::Error>>()?
            }
            Self::Postgres { pool } => {
                let rows = sqlx::query(
                    "SELECT author_id, COUNT(*) AS count FROM senryus
                     WHERE server_id=$1 GROUP BY author_id ORDER BY count DESC",
                )
                .bind(server_id)
                .fetch_all(pool)
                .await?;
                rows.into_iter()
                    .map(|r| Ok((r.try_get("author_id")?, r.try_get("count")?)))
                    .collect::<Result<Vec<_>, sqlx::Error>>()?
            }
        };

        let mut out = Vec::new();
        let mut previous_count = -1i64;
        let mut previous_rank = 0usize;
        for (idx, (author_id, count)) in pairs.into_iter().enumerate() {
            let rank = if count == previous_count {
                previous_rank
            } else {
                idx + 1
            };
            if rank > 5 {
                break;
            }
            out.push(RankEntry {
                count,
                author_id,
                rank,
            });
            previous_count = count;
            previous_rank = rank;
        }
        Ok(out)
    }

    pub async fn server_stats(&self, server_id: &str) -> Result<ServerStats> {
        match self {
            Self::Sqlite { pool, .. } => {
                let total = sqlx::query_scalar::<_, i64>(
                    "SELECT COUNT(*) FROM senryus WHERE server_id=$1",
                )
                .bind(server_id)
                .fetch_one(pool)
                .await?;
                let authors = sqlx::query_scalar::<_, i64>(
                    "SELECT COUNT(DISTINCT author_id) FROM senryus WHERE server_id=$1",
                )
                .bind(server_id)
                .fetch_one(pool)
                .await?;
                Ok(ServerStats { total_senryus: total, unique_authors: authors })
            }
            Self::Postgres { pool } => {
                let total = sqlx::query_scalar::<_, i64>(
                    "SELECT COUNT(*) FROM senryus WHERE server_id=$1",
                )
                .bind(server_id)
                .fetch_one(pool)
                .await?;
                let authors = sqlx::query_scalar::<_, i64>(
                    "SELECT COUNT(DISTINCT author_id) FROM senryus WHERE server_id=$1",
                )
                .bind(server_id)
                .fetch_one(pool)
                .await?;
                Ok(ServerStats { total_senryus: total, unique_authors: authors })
            }
        }
    }

    pub async fn db_stats(&self) -> Result<DbStats> {
        match self {
            Self::Sqlite { pool, .. } => Ok(DbStats {
                senryu_count: sqlx::query_scalar("SELECT COUNT(*) FROM senryus").fetch_one(pool).await?,
                muted_channel_count: sqlx::query_scalar("SELECT COUNT(*) FROM muted_channels").fetch_one(pool).await?,
                opt_out_count: sqlx::query_scalar("SELECT COUNT(*) FROM detection_opt_outs").fetch_one(pool).await?,
                connected: true,
            }),
            Self::Postgres { pool } => Ok(DbStats {
                senryu_count: sqlx::query_scalar("SELECT COUNT(*) FROM senryus").fetch_one(pool).await?,
                muted_channel_count: sqlx::query_scalar("SELECT COUNT(*) FROM muted_channels").fetch_one(pool).await?,
                opt_out_count: sqlx::query_scalar("SELECT COUNT(*) FROM detection_opt_outs").fetch_one(pool).await?,
                connected: true,
            }),
        }
    }

    pub async fn is_muted(&self, channel_id: &str) -> Result<bool> {
        match self {
            Self::Sqlite { pool, .. } => Ok(sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM muted_channels WHERE channel_id=$1",
            )
            .bind(channel_id)
            .fetch_one(pool)
            .await? > 0),
            Self::Postgres { pool } => Ok(sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM muted_channels WHERE channel_id=$1",
            )
            .bind(channel_id)
            .fetch_one(pool)
            .await? > 0),
        }
    }

    pub async fn mute(&self, channel_id: &str, guild_id: &str) -> Result<()> {
        match self {
            Self::Sqlite { pool, .. } => {
                sqlx::query(
                    "INSERT INTO muted_channels(channel_id,guild_id) VALUES($1,$2)
                     ON CONFLICT(channel_id) DO UPDATE SET guild_id=excluded.guild_id",
                )
                .bind(channel_id).bind(guild_id).execute(pool).await?;
            }
            Self::Postgres { pool } => {
                sqlx::query(
                    "INSERT INTO muted_channels(channel_id,guild_id) VALUES($1,$2)
                     ON CONFLICT(channel_id) DO UPDATE SET guild_id=excluded.guild_id",
                )
                .bind(channel_id).bind(guild_id).execute(pool).await?;
            }
        }
        Ok(())
    }

    pub async fn unmute(&self, channel_id: &str) -> Result<()> {
        match self {
            Self::Sqlite { pool, .. } => {
                sqlx::query("DELETE FROM muted_channels WHERE channel_id=$1")
                    .bind(channel_id).execute(pool).await?;
            }
            Self::Postgres { pool } => {
                sqlx::query("DELETE FROM muted_channels WHERE channel_id=$1")
                    .bind(channel_id).execute(pool).await?;
            }
        }
        Ok(())
    }

    pub async fn opt_out_set_by(&self, server_id: &str, user_id: &str) -> Result<Option<String>> {
        match self {
            Self::Sqlite { pool, .. } => Ok(sqlx::query_scalar::<_, String>(
                "SELECT set_by FROM detection_opt_outs WHERE server_id=$1 AND user_id=$2",
            )
            .bind(server_id).bind(user_id).fetch_optional(pool).await?),
            Self::Postgres { pool } => Ok(sqlx::query_scalar::<_, String>(
                "SELECT set_by FROM detection_opt_outs WHERE server_id=$1 AND user_id=$2",
            )
            .bind(server_id).bind(user_id).fetch_optional(pool).await?),
        }
    }

    pub async fn opt_out(&self, server_id: &str, user_id: &str, set_by: &str) -> Result<()> {
        match self {
            Self::Sqlite { pool, .. } => {
                sqlx::query(
                    "INSERT INTO detection_opt_outs(server_id,user_id,set_by) VALUES($1,$2,$3)
                     ON CONFLICT(server_id,user_id) DO UPDATE SET set_by=excluded.set_by",
                )
                .bind(server_id).bind(user_id).bind(set_by).execute(pool).await?;
            }
            Self::Postgres { pool } => {
                sqlx::query(
                    "INSERT INTO detection_opt_outs(server_id,user_id,set_by) VALUES($1,$2,$3)
                     ON CONFLICT(server_id,user_id) DO UPDATE SET set_by=excluded.set_by",
                )
                .bind(server_id).bind(user_id).bind(set_by).execute(pool).await?;
            }
        }
        Ok(())
    }

    pub async fn opt_in(&self, server_id: &str, user_id: &str) -> Result<()> {
        match self {
            Self::Sqlite { pool, .. } => {
                sqlx::query("DELETE FROM detection_opt_outs WHERE server_id=$1 AND user_id=$2")
                    .bind(server_id).bind(user_id).execute(pool).await?;
            }
            Self::Postgres { pool } => {
                sqlx::query("DELETE FROM detection_opt_outs WHERE server_id=$1 AND user_id=$2")
                    .bind(server_id).bind(user_id).execute(pool).await?;
            }
        }
        Ok(())
    }

    pub async fn list_opt_outs(&self, server_id: &str) -> Result<Vec<(String, String)>> {
        match self {
            Self::Sqlite { pool, .. } => {
                let rows = sqlx::query("SELECT user_id,set_by FROM detection_opt_outs WHERE server_id=$1 ORDER BY user_id")
                    .bind(server_id).fetch_all(pool).await?;
                rows.into_iter().map(|r| Ok((r.try_get("user_id")?, r.try_get("set_by")?))).collect::<Result<_, sqlx::Error>>().map_err(Into::into)
            }
            Self::Postgres { pool } => {
                let rows = sqlx::query("SELECT user_id,set_by FROM detection_opt_outs WHERE server_id=$1 ORDER BY user_id")
                    .bind(server_id).fetch_all(pool).await?;
                rows.into_iter().map(|r| Ok((r.try_get("user_id")?, r.try_get("set_by")?))).collect::<Result<_, sqlx::Error>>().map_err(Into::into)
            }
        }
    }

    pub async fn channel_overrides(&self, guild_id: &str) -> Result<Vec<(i16, bool)>> {
        match self {
            Self::Sqlite { pool, .. } => {
                let rows = sqlx::query("SELECT channel_type,enabled FROM guild_channel_type_settings WHERE guild_id=$1")
                    .bind(guild_id).fetch_all(pool).await?;
                rows.into_iter().map(|r| Ok((r.try_get::<i64,_>("channel_type")? as i16, r.try_get("enabled")?))).collect::<Result<_, sqlx::Error>>().map_err(Into::into)
            }
            Self::Postgres { pool } => {
                let rows = sqlx::query("SELECT channel_type,enabled FROM guild_channel_type_settings WHERE guild_id=$1")
                    .bind(guild_id).fetch_all(pool).await?;
                rows.into_iter().map(|r| Ok((r.try_get::<i32,_>("channel_type")? as i16, r.try_get("enabled")?))).collect::<Result<_, sqlx::Error>>().map_err(Into::into)
            }
        }
    }

    pub async fn set_channel_override(&self, guild_id: &str, channel_type: i16, enabled: Option<bool>) -> Result<()> {
        if let Some(enabled) = enabled {
            match self {
                Self::Sqlite { pool, .. } => {
                    sqlx::query(
                        "INSERT INTO guild_channel_type_settings(guild_id,channel_type,enabled) VALUES($1,$2,$3)
                         ON CONFLICT(guild_id,channel_type) DO UPDATE SET enabled=excluded.enabled",
                    )
                    .bind(guild_id).bind(channel_type as i64).bind(enabled).execute(pool).await?;
                }
                Self::Postgres { pool } => {
                    sqlx::query(
                        "INSERT INTO guild_channel_type_settings(guild_id,channel_type,enabled) VALUES($1,$2,$3)
                         ON CONFLICT(guild_id,channel_type) DO UPDATE SET enabled=excluded.enabled",
                    )
                    .bind(guild_id).bind(i32::from(channel_type)).bind(enabled).execute(pool).await?;
                }
            }
        } else {
            match self {
                Self::Sqlite { pool, .. } => {
                    sqlx::query("DELETE FROM guild_channel_type_settings WHERE guild_id=$1 AND channel_type=$2")
                        .bind(guild_id).bind(channel_type as i64).execute(pool).await?;
                }
                Self::Postgres { pool } => {
                    sqlx::query("DELETE FROM guild_channel_type_settings WHERE guild_id=$1 AND channel_type=$2")
                        .bind(guild_id).bind(i32::from(channel_type)).execute(pool).await?;
                }
            }
        }
        Ok(())
    }

    pub async fn get_metadata(&self, key: &str) -> Result<Option<String>> {
        match self {
            Self::Sqlite { pool, .. } => Ok(sqlx::query_scalar::<_, String>("SELECT value FROM metadata WHERE key=$1")
                .bind(key).fetch_optional(pool).await?),
            Self::Postgres { pool } => Ok(sqlx::query_scalar::<_, String>("SELECT value FROM metadata WHERE key=$1")
                .bind(key).fetch_optional(pool).await?),
        }
    }

    pub async fn set_metadata(&self, key: &str, value: &str) -> Result<()> {
        match self {
            Self::Sqlite { pool, .. } => {
                sqlx::query("INSERT INTO metadata(key,value) VALUES($1,$2) ON CONFLICT(key) DO UPDATE SET value=excluded.value")
                    .bind(key).bind(value).execute(pool).await?;
            }
            Self::Postgres { pool } => {
                sqlx::query("INSERT INTO metadata(key,value) VALUES($1,$2) ON CONFLICT(key) DO UPDATE SET value=excluded.value")
                    .bind(key).bind(value).execute(pool).await?;
            }
        }
        Ok(())
    }

    pub async fn clear_metadata(&self, key: &str) -> Result<()> {
        match self {
            Self::Sqlite { pool, .. } => { sqlx::query("DELETE FROM metadata WHERE key=$1").bind(key).execute(pool).await?; }
            Self::Postgres { pool } => { sqlx::query("DELETE FROM metadata WHERE key=$1").bind(key).execute(pool).await?; }
        }
        Ok(())
    }

    pub async fn delete_guild_data(&self, guild_id: &str) -> Result<(u64, u64, u64)> {
        match self {
            Self::Sqlite { pool, .. } => {
                let a = sqlx::query("DELETE FROM senryus WHERE server_id=$1").bind(guild_id).execute(pool).await?.rows_affected();
                let b = sqlx::query("DELETE FROM detection_opt_outs WHERE server_id=$1").bind(guild_id).execute(pool).await?.rows_affected();
                let c = sqlx::query("DELETE FROM guild_channel_type_settings WHERE guild_id=$1").bind(guild_id).execute(pool).await?.rows_affected();
                sqlx::query("DELETE FROM muted_channels WHERE guild_id=$1").bind(guild_id).execute(pool).await?;
                Ok((a,b,c))
            }
            Self::Postgres { pool } => {
                let a = sqlx::query("DELETE FROM senryus WHERE server_id=$1").bind(guild_id).execute(pool).await?.rows_affected();
                let b = sqlx::query("DELETE FROM detection_opt_outs WHERE server_id=$1").bind(guild_id).execute(pool).await?.rows_affected();
                let c = sqlx::query("DELETE FROM guild_channel_type_settings WHERE guild_id=$1").bind(guild_id).execute(pool).await?.rows_affected();
                sqlx::query("DELETE FROM muted_channels WHERE guild_id=$1").bind(guild_id).execute(pool).await?;
                Ok((a,b,c))
            }
        }
    }

    pub async fn encrypt_plaintext_rows(&self, crypto: &Crypto) -> Result<u64> {
        if !crypto.enabled() { return Ok(0); }
        let mut changed = 0u64;
        match self {
            Self::Sqlite { pool, .. } => {
                let rows = sqlx::query("SELECT id,kamigo,nakasichi,simogo FROM senryus").fetch_all(pool).await?;
                for row in rows {
                    let id: i64 = row.try_get("id")?;
                    let k: String = row.try_get("kamigo")?;
                    let n: String = row.try_get("nakasichi")?;
                    let s: String = row.try_get("simogo")?;
                    if crypto.is_encrypted(&k) && crypto.is_encrypted(&n) && crypto.is_encrypted(&s) { continue; }
                    let ek = if crypto.is_encrypted(&k) { k } else { crypto.encrypt(&k)? };
                    let en = if crypto.is_encrypted(&n) { n } else { crypto.encrypt(&n)? };
                    let es = if crypto.is_encrypted(&s) { s } else { crypto.encrypt(&s)? };
                    sqlx::query("UPDATE senryus SET kamigo=$1,nakasichi=$2,simogo=$3 WHERE id=$4")
                        .bind(ek).bind(en).bind(es).bind(id).execute(pool).await?;
                    changed += 1;
                }
            }
            Self::Postgres { pool } => {
                let rows = sqlx::query("SELECT id,kamigo,nakasichi,simogo FROM senryus").fetch_all(pool).await?;
                for row in rows {
                    let id = i64::from(row.try_get::<i32,_>("id")?);
                    let k: String = row.try_get("kamigo")?;
                    let n: String = row.try_get("nakasichi")?;
                    let s: String = row.try_get("simogo")?;
                    if crypto.is_encrypted(&k) && crypto.is_encrypted(&n) && crypto.is_encrypted(&s) { continue; }
                    let ek = if crypto.is_encrypted(&k) { k } else { crypto.encrypt(&k)? };
                    let en = if crypto.is_encrypted(&n) { n } else { crypto.encrypt(&n)? };
                    let es = if crypto.is_encrypted(&s) { s } else { crypto.encrypt(&s)? };
                    sqlx::query("UPDATE senryus SET kamigo=$1,nakasichi=$2,simogo=$3 WHERE id=$4")
                        .bind(ek).bind(en).bind(es).bind(id).execute(pool).await?;
                    changed += 1;
                }
            }
        }
        Ok(changed)
    }

    pub async fn checkpoint_sqlite(&self) -> Result<()> {
        if let Self::Sqlite { pool, .. } = self {
            sqlx::query("PRAGMA wal_checkpoint(FULL)").execute(pool).await?;
        }
        Ok(())
    }
}

fn decode_sqlite_row(row: sqlx::sqlite::SqliteRow, crypto: &Crypto) -> Result<Senryu> {
    Ok(Senryu {
        id: row.try_get("id")?,
        server_id: row.try_get("server_id")?,
        author_id: row.try_get("author_id")?,
        kamigo: crypto.decrypt_if_needed(row.try_get::<String,_>("kamigo")?.as_str())?,
        nakashichi: crypto.decrypt_if_needed(row.try_get::<String,_>("nakasichi")?.as_str())?,
        shimogo: crypto.decrypt_if_needed(row.try_get::<String,_>("simogo")?.as_str())?,
        spoiler: row.try_get("spoiler")?,
        created_at: 0,
    })
}

fn decode_pg_row(row: sqlx::postgres::PgRow, crypto: &Crypto) -> Result<Senryu> {
    Ok(Senryu {
        id: i64::from(row.try_get::<i32,_>("id")?),
        server_id: row.try_get("server_id")?,
        author_id: row.try_get("author_id")?,
        kamigo: crypto.decrypt_if_needed(row.try_get::<String,_>("kamigo")?.as_str())?,
        nakashichi: crypto.decrypt_if_needed(row.try_get::<String,_>("nakasichi")?.as_str())?,
        shimogo: crypto.decrypt_if_needed(row.try_get::<String,_>("simogo")?.as_str())?,
        spoiler: row.try_get("spoiler")?,
        created_at: 0,
    })
}

fn sqlite_schema() -> &'static [&'static str] {
    &[
        "PRAGMA journal_mode=WAL",
        "CREATE TABLE IF NOT EXISTS senryus(
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            server_id TEXT NOT NULL,
            author_id TEXT NOT NULL,
            kamigo TEXT NOT NULL,
            nakasichi TEXT NOT NULL,
            simogo TEXT NOT NULL,
            spoiler BOOLEAN NOT NULL DEFAULT FALSE,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
        )",
        "CREATE INDEX IF NOT EXISTS idx_senryus_server_id ON senryus(server_id)",
        "CREATE INDEX IF NOT EXISTS idx_senryus_author_id ON senryus(author_id)",
        "CREATE INDEX IF NOT EXISTS idx_senryus_server_spoiler ON senryus(server_id,spoiler)",
        "CREATE TABLE IF NOT EXISTS muted_channels(
            channel_id TEXT PRIMARY KEY,
            guild_id TEXT NOT NULL
        )",
        "CREATE INDEX IF NOT EXISTS idx_muted_channels_guild_id ON muted_channels(guild_id)",
        "CREATE TABLE IF NOT EXISTS guild_channel_type_settings(
            guild_id TEXT NOT NULL,
            channel_type INTEGER NOT NULL,
            enabled BOOLEAN NOT NULL,
            PRIMARY KEY(guild_id,channel_type)
        )",
        "CREATE TABLE IF NOT EXISTS detection_opt_outs(
            server_id TEXT NOT NULL,
            user_id TEXT NOT NULL,
            set_by TEXT NOT NULL DEFAULT 'self',
            PRIMARY KEY(server_id,user_id)
        )",
        "CREATE TABLE IF NOT EXISTS metadata(
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )",
    ]
}

fn postgres_schema() -> &'static [&'static str] {
    &[
        "CREATE TABLE IF NOT EXISTS senryus(
            id SERIAL PRIMARY KEY,
            server_id TEXT NOT NULL,
            author_id TEXT NOT NULL,
            kamigo TEXT NOT NULL,
            nakasichi TEXT NOT NULL,
            simogo TEXT NOT NULL,
            spoiler BOOLEAN NOT NULL DEFAULT FALSE,
            created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
        )",
        "CREATE INDEX IF NOT EXISTS idx_senryus_server_id ON senryus(server_id)",
        "CREATE INDEX IF NOT EXISTS idx_senryus_author_id ON senryus(author_id)",
        "CREATE INDEX IF NOT EXISTS idx_senryus_server_spoiler ON senryus(server_id,spoiler)",
        "CREATE TABLE IF NOT EXISTS muted_channels(
            channel_id TEXT PRIMARY KEY,
            guild_id TEXT NOT NULL
        )",
        "CREATE INDEX IF NOT EXISTS idx_muted_channels_guild_id ON muted_channels(guild_id)",
        "CREATE TABLE IF NOT EXISTS guild_channel_type_settings(
            guild_id TEXT NOT NULL,
            channel_type INTEGER NOT NULL,
            enabled BOOLEAN NOT NULL,
            PRIMARY KEY(guild_id,channel_type)
        )",
        "CREATE TABLE IF NOT EXISTS detection_opt_outs(
            server_id TEXT NOT NULL,
            user_id TEXT NOT NULL,
            set_by TEXT NOT NULL DEFAULT 'self',
            PRIMARY KEY(server_id,user_id)
        )",
        "CREATE TABLE IF NOT EXISTS metadata(
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )",
    ]
}
