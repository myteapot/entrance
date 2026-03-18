use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;
use toml::map::Map;
use toml::Value;

#[derive(Debug)]
pub struct ConfigStore {
    path: PathBuf,
    document: RwLock<Map<String, Value>>,
}

impl ConfigStore {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let document = read_table(&path)?;

        Ok(Self {
            path,
            document: RwLock::new(document),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn reload(&self) -> Result<()> {
        let next_document = read_table(&self.path)?;
        *self.write_document()? = next_document;
        Ok(())
    }

    pub fn get_section<T>(&self, section: &str) -> Result<Option<T>>
    where
        T: DeserializeOwned,
    {
        let document = self.read_document()?;
        let Some(value) = document.get(section) else {
            return Ok(None);
        };

        let decoded = value
            .clone()
            .try_into()
            .with_context(|| format!("failed to decode config section `{section}`"))?;

        Ok(Some(decoded))
    }

    pub fn set_section<T>(&self, section: &str, value: &T) -> Result<()>
    where
        T: Serialize,
    {
        let section_value =
            Value::try_from(value).with_context(|| format!("failed to encode `{section}`"))?;
        let mut document = self.write_document()?;
        document.insert(section.to_owned(), section_value);
        drop(document);
        self.save()
    }

    pub fn snapshot(&self) -> Result<Map<String, Value>> {
        Ok(self.read_document()?.clone())
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create config dir {}", parent.display()))?;
        }

        let serialized = {
            let document = self.read_document()?;
            toml::to_string_pretty(&Value::Table(document.clone()))
                .context("failed to serialize config")?
        };

        fs::write(&self.path, serialized)
            .with_context(|| format!("failed to write config file {}", self.path.display()))?;

        Ok(())
    }

    fn read_document(&self) -> Result<RwLockReadGuard<'_, Map<String, Value>>> {
        self.document
            .read()
            .map_err(|_| anyhow::anyhow!("config store lock poisoned"))
    }

    fn write_document(&self) -> Result<RwLockWriteGuard<'_, Map<String, Value>>> {
        self.document
            .write()
            .map_err(|_| anyhow::anyhow!("config store lock poisoned"))
    }
}

fn read_table(path: &Path) -> Result<Map<String, Value>> {
    if !path.exists() {
        return Ok(Map::new());
    }

    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read config file {}", path.display()))?;

    if raw.trim().is_empty() {
        return Ok(Map::new());
    }

    let parsed = raw
        .parse::<Value>()
        .with_context(|| format!("failed to parse config file {}", path.display()))?;

    match parsed {
        Value::Table(table) => Ok(table),
        _ => Err(anyhow::anyhow!(
            "config file {} must contain a TOML table",
            path.display()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use tempfile::tempdir;

    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
    struct CoreConfig {
        theme: String,
    }

    #[test]
    fn reads_and_writes_sections() -> Result<()> {
        let dir = tempdir()?;
        let path = dir.path().join("entrance.toml");
        fs::write(&path, "[core]\ntheme = \"dark\"\n")?;

        let store = ConfigStore::load(&path)?;
        let core: CoreConfig = store.get_section("core")?.expect("core section must exist");
        assert_eq!(
            core,
            CoreConfig {
                theme: "dark".to_owned()
            }
        );

        store.set_section(
            "core",
            &CoreConfig {
                theme: "light".to_owned(),
            },
        )?;

        let reloaded = ConfigStore::load(&path)?;
        let core: CoreConfig = reloaded
            .get_section("core")?
            .expect("core section must exist after save");
        assert_eq!(
            core,
            CoreConfig {
                theme: "light".to_owned()
            }
        );

        Ok(())
    }
}
