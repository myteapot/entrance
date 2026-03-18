use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use refinery::Report;
use rusqlite::Connection as RusqliteConnection;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

mod core_migrations {
    use refinery::embed_migrations;

    embed_migrations!("migrations/core");
}

#[derive(Clone, Debug)]
pub struct DataStore {
    db_path: PathBuf,
    pool: SqlitePool,
}

impl DataStore {
    pub async fn connect(path: impl AsRef<Path>) -> Result<Self> {
        let db_path = path.as_ref().to_path_buf();

        if let Some(parent) = db_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("failed to create database dir {}", parent.display()))?;
        }

        let options = sqlite_file_options(&db_path);
        migrate_with_path(&db_path)?;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .context("failed to create sqlite pool")?;

        Ok(Self { db_path, pool })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    pub async fn migrate(&self) -> Result<Report> {
        migrate_with_path(self.db_path())
    }

    pub fn plugin_table_name(plugin_name: &str, table_name: &str) -> Result<String> {
        validate_identifier(plugin_name, "plugin name")?;
        validate_identifier(table_name, "table name")?;
        Ok(format!("plugin_{plugin_name}_{table_name}"))
    }
}

fn sqlite_file_options(path: &Path) -> SqliteConnectOptions {
    SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .foreign_keys(true)
}

fn migrate_with_path(path: &Path) -> Result<Report> {
    let mut connection = RusqliteConnection::open(path).with_context(|| {
        format!(
            "failed to open sqlite connection for migration {}",
            path.display()
        )
    })?;
    let report = core_migrations::migrations::runner()
        .run(&mut connection)
        .context("failed to run core migrations")?;
    Ok(report)
}

fn validate_identifier(value: &str, label: &str) -> Result<()> {
    if value.is_empty() {
        bail!("{label} cannot be empty");
    }

    if !value
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
    {
        bail!("{label} must use lowercase ASCII letters, digits, or underscores");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::Row;
    use tempfile::tempdir;

    #[tokio::test]
    async fn migrates_core_tables() -> Result<()> {
        let dir = tempdir()?;
        let store = DataStore::connect(dir.path().join("entrance.db")).await?;
        let rows = sqlx::query(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name LIKE 'core_%'",
        )
        .fetch_all(store.pool())
        .await?;

        let table_names = rows
            .iter()
            .map(|row| row.get::<String, _>("name"))
            .collect::<Vec<_>>();

        assert!(table_names.iter().any(|name| name == "core_plugins"));
        assert!(table_names.iter().any(|name| name == "core_hotkeys"));
        assert!(table_names.iter().any(|name| name == "core_event_log"));

        Ok(())
    }

    #[test]
    fn formats_plugin_table_names() -> Result<()> {
        assert_eq!(
            DataStore::plugin_table_name("vault", "tokens")?,
            "plugin_vault_tokens"
        );
        assert!(DataStore::plugin_table_name("Vault", "tokens").is_err());
        Ok(())
    }
}
