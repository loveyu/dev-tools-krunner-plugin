use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use devtools_core::Settings;

use crate::platform;

const CONFIG_DIRECTORY: &str = "devtools";
const CONFIG_FILE: &str = "settings.json";

/// 用户级配置与 KDE 自启动入口的持久化管理器。
pub struct SettingsStore {
    config_root: PathBuf,
}

impl SettingsStore {
    pub fn from_environment() -> io::Result<Self> {
        Ok(Self {
            config_root: platform::config_root_from_environment()?,
        })
    }

    #[cfg(test)]
    pub fn new(config_root: PathBuf) -> Self {
        Self { config_root }
    }

    pub fn load(&self) -> Settings {
        let path = self.settings_path();
        match fs::read_to_string(path) {
            Ok(contents) => serde_json::from_str(&contents).unwrap_or_else(|error| {
                eprintln!("devtools-workerd: invalid settings, using defaults: {error}");
                Settings::default()
            }),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Settings::default(),
            Err(error) => {
                eprintln!("devtools-workerd: failed to read settings: {error}");
                Settings::default()
            }
        }
    }

    pub fn apply(&self, previous: &Settings, next: &Settings, executable: &Path) -> io::Result<()> {
        self.sync_autostart(next.autostart, executable)?;
        if let Err(error) = self.save(next) {
            let _ = self.sync_autostart(previous.autostart, executable);
            return Err(error);
        }
        Ok(())
    }

    pub fn sync_autostart(&self, enabled: bool, executable: &Path) -> io::Result<()> {
        let path = self.autostart_path();
        if !enabled {
            return match fs::remove_file(path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error),
            };
        }

        let directory = path.parent().expect("自启动文件必须有父目录");
        fs::create_dir_all(directory)?;
        fs::write(path, platform::autostart_entry(executable))
    }

    fn save(&self, settings: &Settings) -> io::Result<()> {
        let path = self.settings_path();
        let directory = path.parent().expect("设置文件必须有父目录");
        fs::create_dir_all(directory)?;
        let contents = serde_json::to_string_pretty(&settings).expect("Settings 序列化不会失败");
        fs::write(path, format!("{contents}\n"))
    }

    fn settings_path(&self) -> PathBuf {
        self.config_root.join(CONFIG_DIRECTORY).join(CONFIG_FILE)
    }

    fn autostart_path(&self) -> PathBuf {
        platform::autostart_path(&self.config_root)
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use devtools_core::{LanguageMode, ThemeMode};

    use super::*;

    #[test]
    fn defaults_when_settings_file_is_missing() {
        let directory = tempdir().expect("应创建临时目录");
        let store = SettingsStore::new(directory.path().to_owned());

        assert_eq!(store.load(), Settings::default());
    }

    #[test]
    fn persists_settings_and_autostart_entry() {
        let directory = tempdir().expect("应创建临时目录");
        let store = SettingsStore::new(directory.path().to_owned());
        let settings = Settings {
            show_tray: false,
            autostart: true,
            theme: ThemeMode::Dark,
            language: LanguageMode::TraditionalChinese,
            ..Settings::default()
        };
        let executable = Path::new("/tmp/Dev Tools/devtools-workerd");

        store
            .apply(&Settings::default(), &settings, executable)
            .expect("设置应保存成功");

        assert_eq!(store.load(), settings);
        let entry = fs::read_to_string(store.autostart_path()).expect("应生成自启动文件");
        assert!(entry.contains(&executable.to_string_lossy().to_string()));
    }

    #[test]
    fn disabling_autostart_removes_entry() {
        let directory = tempdir().expect("应创建临时目录");
        let store = SettingsStore::new(directory.path().to_owned());
        let executable = Path::new("/tmp/devtools-workerd");
        store
            .sync_autostart(true, executable)
            .expect("应生成自启动文件");

        store
            .sync_autostart(false, executable)
            .expect("应删除自启动文件");

        assert!(!store.autostart_path().exists());
    }
}
