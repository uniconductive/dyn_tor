use serde::{Deserialize, Serialize};
use std::error::Error;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum LogLevelConfig {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LogConfig {
    pub r#use: bool,
    pub path: String,
    pub level: LogLevelConfig,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            r#use: false,
            path: "./".to_owned(),
            level: LogLevelConfig::Info,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TorDataDirsConfig {
    pub path: String,
    pub clear: bool,
    #[serde(skip_serializing, skip_deserializing)]
    pub full_path: String,
}


#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TorConfig {
    pub path: String,
    pub torrc: String,
    pub data_dirs: TorDataDirsConfig,
    pub start_port: u16,
    pub port_count: u16,
    #[serde(skip_serializing, skip_deserializing)]
    pub full_path: String,
    #[serde(skip_serializing, skip_deserializing)]
    pub torrc_full_path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppConfig {
    pub tor: TorConfig,
    pub listen_addr: String,
    #[serde(default)]
    pub log: LogConfig,
    // #[serde(skip_serializing, skip_deserializing)]
    // pub tor_full_path: String,
    // #[serde(skip_serializing, skip_deserializing)]
    // pub torrc_full_path: String,
    // #[serde(skip_serializing, skip_deserializing)]
    // pub data_dirs_full_path: String,
}

pub fn get_config_file_path(force_near_binary: bool) -> Result<(PathBuf, bool), Box<dyn Error>> {
    let mut path = std::env::current_exe()?;
    if !path.set_extension("config") {
        panic!("!file_name.set_extension('config')")
    }
    if force_near_binary {
        Ok((path, false))
    } else if path.exists() && path.is_file() {
        Ok((path, true))
    } else {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
        if manifest_dir.is_empty() {
            Ok((path, false))
        } else {
            let file_name = path.file_name().unwrap().to_str().unwrap().to_string();
            let mut res = PathBuf::from(manifest_dir);
            res.push("configs");
            res.push(&file_name);
            Ok((res, false))
        }
    }
}

pub fn load_config() -> Result<(AppConfig, PathBuf), Box<dyn Error>> {
    let (file_path, checked) = get_config_file_path(false)?;
    let file_path_str = file_path.to_str().unwrap().to_string();
    if checked || (file_path.exists() && file_path.is_file()) {
        println!("config file: '{}'", file_path_str);
        let data = std::fs::read(file_path.clone())
            .expect(&("Unable to read config file: ".to_owned() + &file_path_str));
        let res: AppConfig = serde_json::from_slice(&data)
            .expect(&("Unable to parse config file: ".to_owned() + &file_path_str));
        Ok((res, file_path))
    } else {
        panic!("config file '{}' does not exists", &file_path_str)
    }
}

/*
pub fn save_config(app_config: &AppConfig) -> Result<(), Box<dyn Error>> {
    let (file_path, _) = get_config_file_path(false)?;
    let res = serde_json::to_vec_pretty(app_config)?;
    std::fs::write(file_path, &res)?;
    Ok(())
}

pub fn save_default_config() -> Result<(), Box<dyn Error>> {
    save_config(&AppConfig {
        tor: TorConfig {
            path: "./tor/bin/tor".to_string(),
            torrc: "./tor/torrc".to_string(),
            data_dirs: TorDataDirsConfig {
                path: "./tor/data_dirs".to_string(),
                clear: true,
                full_path: "".to_string(),
            },
            start_port: 8600,
            port_count: 20,
            full_path: "".to_string(),
            torrc_full_path: "".to_string()
        },
        listen_addr: "127.0.0.1:9051".to_string(),
        log: Default::default(),
    })
}
*/