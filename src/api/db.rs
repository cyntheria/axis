use rusqlite::{params, Connection, Result};
use std::path::Path;
use crate::api::plugin::PluginMetadata;

pub struct PluginDatabase {
    conn: Connection,
}

impl PluginDatabase {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;
        let db = Self { conn };
        db.init()?;
        Ok(db)
    }

    fn init(&self) -> Result<()> {
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS plugins (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                version TEXT NOT NULL,
                author TEXT,
                description TEXT,
                path TEXT NOT NULL,
                enabled INTEGER DEFAULT 1
            )",
            [],
        )?;
        Ok(())
    }

    pub fn register_plugin(&self, meta: &PluginMetadata, path: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO plugins (name, version, author, description, path)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![meta.name, meta.version, meta.author, meta.description, path],
        )?;
        Ok(())
    }

    pub fn list_plugins(&self) -> Result<Vec<(PluginMetadata, String, bool)>> {
        let mut stmt = self.conn.prepare("SELECT name, version, author, description, path, enabled FROM plugins")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                PluginMetadata {
                    name: row.get(0)?,
                    version: row.get(1)?,
                    author: row.get(2)?,
                    description: row.get(3)?,
                },
                row.get::<_, String>(4)?,
                row.get::<_, i32>(5)? != 0,
            ))
        })?;

        let mut plugins = Vec::new();
        for row in rows {
            plugins.push(row?);
        }
        Ok(plugins)
    }

    pub fn set_plugin_enabled(&self, name: &str, enabled: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE plugins SET enabled = ?1 WHERE name = ?2",
            params![if enabled { 1 } else { 0 }, name],
        )?;
        Ok(())
    }

    pub fn remove_plugin(&self, name: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM plugins WHERE name = ?1",
            params![name],
        )?;
        Ok(())
    }
}
