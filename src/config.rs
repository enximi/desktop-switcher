use std::{fs, io, path::PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

const CONFIG_FILE_NAME: &str = "config.toml";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FeatureSettings {
    pub edge_wheel_switching_enabled: bool,
    pub right_button_gestures_enabled: bool,
}

impl Default for FeatureSettings {
    fn default() -> Self {
        Self {
            edge_wheel_switching_enabled: true,
            right_button_gestures_enabled: true,
        }
    }
}

pub fn load_or_create() -> Result<FeatureSettings> {
    let path = config_path()?;

    if !path.exists() {
        let settings = FeatureSettings::default();
        save(&settings)?;
        return Ok(settings);
    }

    ::config::Config::builder()
        .add_source(::config::File::from(path).required(true))
        .build()
        .and_then(::config::Config::try_deserialize)
        .map_err(ConfigError::Read)
}

pub fn save(settings: &FeatureSettings) -> Result<()> {
    let content = toml::to_string_pretty(settings).map_err(ConfigError::Serialize)?;
    fs::write(config_path()?, content).map_err(ConfigError::Io)
}

fn config_path() -> Result<PathBuf> {
    let executable = std::env::current_exe().map_err(ConfigError::Io)?;
    let directory = executable
        .parent()
        .ok_or_else(|| ConfigError::ExecutablePathMissingParent(executable.clone()))?;

    Ok(directory.join(CONFIG_FILE_NAME))
}

pub type Result<T> = std::result::Result<T, ConfigError>;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("配置文件读写失败: {0}")]
    Io(io::Error),

    #[error("配置文件读取失败: {0}")]
    Read(::config::ConfigError),

    #[error("配置文件序列化失败: {0}")]
    Serialize(toml::ser::Error),

    #[error("无法确定可执行文件所在目录: {}", .0.display())]
    ExecutablePathMissingParent(PathBuf),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_feature_settings_from_toml() {
        let settings = load_from_toml(
            "\
edge_wheel_switching_enabled = false
right_button_gestures_enabled = true
",
        )
        .unwrap();

        assert_eq!(
            settings,
            FeatureSettings {
                edge_wheel_switching_enabled: false,
                right_button_gestures_enabled: true,
            }
        );
    }

    #[test]
    fn rejects_unknown_keys() {
        let error = load_from_toml(
            "\
edge_wheel_switching_enabled = true
right_button_gestures_enabled = true
extra = true
",
        )
        .unwrap_err();

        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn rejects_missing_keys() {
        let error = load_from_toml("edge_wheel_switching_enabled = true\n").unwrap_err();

        assert!(error.to_string().contains("right_button_gestures_enabled"));
    }

    fn load_from_toml(
        content: &str,
    ) -> std::result::Result<FeatureSettings, ::config::ConfigError> {
        ::config::Config::builder()
            .add_source(::config::File::from_str(
                content,
                ::config::FileFormat::Toml,
            ))
            .build()?
            .try_deserialize()
    }
}
