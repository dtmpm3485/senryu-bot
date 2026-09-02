use crate::{config::BackupConfig, db::Database};
use anyhow::{Context, Result};
use chrono::Local;
use std::{path::{Path, PathBuf}, sync::Arc, time::Duration};
use tokio::{fs, time};

#[derive(Clone)]
pub struct BackupManager {
    cfg: BackupConfig,
    db: Database,
}

impl BackupManager {
    pub fn new(cfg: BackupConfig, db: Database) -> Self { Self { cfg, db } }

    pub async fn create_backup(&self) -> Result<PathBuf> {
        let source = self.db.sqlite_path().context("backups are available only for SQLite")?;
        self.db.checkpoint_sqlite().await?;
        fs::create_dir_all(&self.cfg.path).await?;
        let name = format!("senryu-{}.db", Local::now().format("%Y%m%d-%H%M%S"));
        let dest = Path::new(&self.cfg.path).join(name);
        fs::copy(source, &dest).await.with_context(|| format!("failed to copy SQLite DB to {}", dest.display()))?;
        self.rotate().await?;
        Ok(dest)
    }

    pub async fn list(&self) -> Result<Vec<PathBuf>> {
        let mut out = Vec::new();
        let mut rd = match fs::read_dir(&self.cfg.path).await {
            Ok(v) => v,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
            Err(e) => return Err(e.into()),
        };
        while let Some(entry) = rd.next_entry().await? {
            let p = entry.path();
            if p.extension().and_then(|x| x.to_str()) == Some("db") { out.push(p); }
        }
        out.sort_by(|a,b| b.file_name().cmp(&a.file_name()));
        Ok(out)
    }

    async fn rotate(&self) -> Result<()> {
        let list = self.list().await?;
        for path in list.into_iter().skip(self.cfg.max_backups) {
            let _ = fs::remove_file(path).await;
        }
        Ok(())
    }

    pub fn spawn(self: Arc<Self>) {
        if !self.cfg.enabled || self.db.sqlite_path().is_none() { return; }
        tokio::spawn(async move {
            let mut tick = time::interval(Duration::from_secs(self.cfg.interval_hour.saturating_mul(3600).max(3600)));
            tick.tick().await;
            loop {
                tick.tick().await;
                match self.create_backup().await {
                    Ok(path) => tracing::info!(backup=%path.display(), "automatic backup created"),
                    Err(err) => tracing::error!(error=%err, "automatic backup failed"),
                }
            }
        });
    }
}
