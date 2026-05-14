use crate::models::{Credentials, Preferences, PreferencesForUpdate, Torrent, TorrentFile};
use anyhow::{Context, Result};
use qbit_rs::model::Credential;
use qbit_rs::Qbit;
use std::{collections::HashMap};

#[derive(Debug, serde::Deserialize)]
pub struct SyncData {
    pub rid: u64,
    #[serde(default)]
    pub full_update: bool,
    #[serde(default)]
    pub torrents: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub torrents_removed: Vec<String>,
}

#[derive(Clone)]
pub struct QBitApi {
    client: Option<Qbit>,
}

impl QBitApi {
    pub fn new(host: &str) -> Self {
        let credential = Credential::new("", "");
        let client = if host.starts_with("http://") || host.starts_with("https://") {
            Some(Qbit::new(host, credential))
        } else {
            None
        };
        return Self {
            client,
        };
    }

    pub fn with_credentials(credentials: &Credentials) -> Self {
        let host = credentials.host.as_str();
        let credential = Credential::new(&credentials.username, &credentials.password);
        let client = if host.starts_with("http://") || host.starts_with("https://") {
            Option::Some(Qbit::new(host, credential))
        } else {
            Option::None
        };
        return Self {
            client,
        };
    }

    pub async fn login(&self) -> Result<()> {
        if self.client.is_none() {
            return Result::Err(anyhow::anyhow!("Client not initialized"));
        }
        let client = self.client.as_ref().unwrap();
        client.login(false).await.context("Login failed")?;
        Ok(())
    }

    pub async fn get_torrents(&self) -> Result<Vec<Torrent>> {
        if self.client.is_none() {
            return Err(anyhow::anyhow!("Client not initialized"));
        }
        let client = self.client.as_ref().unwrap();
        let torrents = client.get_torrent_list(Default::default()).await?;
        return Ok(torrents.into_iter().map(|t| t.into()).collect());
    }

    pub async fn get_torrent_files(&self, hash: &str) -> Result<Vec<TorrentFile>> {
        if self.client.is_none() {
            return Err(anyhow::anyhow!("Client not initialized"));
        }
        let client = self.client.as_ref().unwrap();
        match client.get_torrent_contents(hash, Option::None).await {
            Ok(files) => Ok(files.into_iter().map(|f| f.into()).collect()),
            Err(e) => return Err(anyhow::Error::new(e)),
        }
    }

    pub async fn get_preferences(&self) -> Result<Preferences> {
        if self.client.is_none() {
            return Result::Err(anyhow::anyhow!("Client not initialized"));
        }
        let client = self.client.as_ref().unwrap();
        let qbit_preference = client.get_preferences().await?;
        let preferences = Preferences {
            script_enabled: qbit_preference.autorun_enabled.unwrap_or_default(),
            script: qbit_preference.autorun_program.unwrap_or_default(),
            log_dir: qbit_preference.file_log_path.clone(),
        };
        Ok(preferences)
    }

    pub async fn set_preferences(&self, preferences: &PreferencesForUpdate) -> Result<()> {
        if self.client.is_none() {
            return Result::Err(anyhow::anyhow!("Client not initialized"));
        }
        let client = self.client.as_ref().unwrap();
        let mut qbit_preference = client.get_preferences().await?;
        if let Some(enabled) = preferences.script_enabled {
            qbit_preference.autorun_enabled = Some(enabled);
        }
        if let Some(program) = &preferences.script {
            qbit_preference.autorun_program = Some(program.clone());
        }
        client.set_preferences(qbit_preference).await?;
        Ok(())
    }

    pub async fn test_connection(&self) -> Result<String> {
        if self.client.is_none() {
            return Err(anyhow::anyhow!("Client not initialized"));
        }
        let client = self.client.as_ref().unwrap();
        return match client
            .get_version()
            .await
            .map_err(|e| anyhow::Error::new(e))
        {
            Ok(v) => {
                if v.trim() == "" {
                    Err(anyhow::anyhow!("Failed to parse qBittorrent version"))
                } else {
                    Ok(v)
                }
            }
            Err(e) => Err(e),
        };
    }

    pub async fn sync(&self, rid: u64) -> Result<SyncData> {
        if self.client.is_none() {
            return Result::Err(anyhow::anyhow!("Client not initialized"));
        }
        let client = self.client.as_ref().unwrap();
        let data = client.sync(Some(rid as i64)).await?;
        let rid_value = data.rid;
        Ok(SyncData {
            rid: rid_value as u64,
            full_update: data.full_update.unwrap_or(false),
            torrents: data
                .torrents
                .map(|t| {
                    t.into_iter()
                        .map(|(k, v)| (k, serde_json::to_value(v).unwrap_or_default()))
                        .collect()
                })
                .unwrap_or_default(),
            torrents_removed: data.torrents_removed.unwrap_or_default(),
        })
    }

    pub async fn add_tags(&self, torrent_hashes: &[String], tags: &[String]) -> Result<()> {
        if self.client.is_none() {
            return Result::Err(anyhow::anyhow!("Client not initialized"));
        }
        let client = self.client.as_ref().unwrap();
        client.add_torrent_tags(torrent_hashes, tags).await?;
        Ok(())
    }

    pub async fn remove_tags(&self, torrent_hashes: &[String], tags: &[String]) -> Result<()> {
        if self.client.is_none() {
            return Result::Err(anyhow::anyhow!("Client not initialized"));
        }
        let client = self.client.as_ref().unwrap();
        client
            .remove_torrent_tags(torrent_hashes, Some(tags))
            .await?;
        Ok(())
    }
}
