use crate::config::{self, AppConfig};
use crate::error;
use std::path::{Path, PathBuf};

fn init_log(log_file_path: String) -> Result<log4rs::Handle, Box<dyn std::error::Error>> {
    use log::LevelFilter;
    use log4rs::{
        append::{
            console::{ConsoleAppender, Target},
            file::FileAppender,
        },
        config::{Appender, Config, Root},
        encode::pattern::PatternEncoder,
        filter::threshold::ThresholdFilter,
    };

    let stderr = ConsoleAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{d}: {l} - {m}\n")))
        .target(Target::Stderr)
        .build();

    let log_file_path = PathBuf::from(log_file_path);

    let log_file = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{d}: {l} - {m}\n")))
        .append(false)
        .build(log_file_path)?;

    let config = Config::builder()
        .appender(
            Appender::builder()
                .filter(Box::new(ThresholdFilter::new(log::LevelFilter::Debug)))
                .build("log", Box::new(log_file)),
        )
        .appender(
            Appender::builder()
                .filter(Box::new(ThresholdFilter::new(log::LevelFilter::Debug)))
                .build("stderr", Box::new(stderr)),
        )
        .build(
            Root::builder()
                .appender("log")
                .appender("stderr")
                .build(LevelFilter::max()),
        )?;

    let res = log4rs::init_config(config)?;
    Ok(res)
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum NormalizePathError {
    #[error("path '{path}' goes thru root ('/')")]
    GoesThruRoot { path: String },
}

fn normalize_path(path: PathBuf, relative_to: PathBuf) -> Result<PathBuf, NormalizePathError> {
    assert!(relative_to.has_root());
    let mut res = relative_to;
    for (_, part) in path.components().enumerate() {
        use std::path::Component;
        match part {
            Component::Prefix(_) => panic!(),
            Component::RootDir => {}
            Component::CurDir => {}
            Component::ParentDir => {
                if !res.pop() {
                    return Err(NormalizePathError::GoesThruRoot {
                        path: path.as_os_str().to_string_lossy().to_string(),
                    });
                }
            }
            Component::Normal(s) => res.push(s),
        }
    }
    Ok(res)
}

fn expand_path(path: PathBuf, relative_to: PathBuf) -> Result<PathBuf, NormalizePathError> {
    if path.is_absolute() {
        Ok(path)
    } else {
        normalize_path(path, relative_to)
    }
}

fn normalize_path_in_config(
    path: &str,
    field: &str,
    is_dir: bool,
    relative_to: PathBuf,
) -> Result<String, error::ConfigFileError> {
    match expand_path(PathBuf::from(path), relative_to) {
        Ok(path_buf) => {
            let mut path = path_buf.to_str().unwrap().to_string();
            if is_dir && !path.ends_with('/') {
                path += "/";
            }
            log::debug!("{}: {}", field, path);
            Ok(path)
        }
        Err(e) => Err(error::ConfigFileError::NormalizePath {
            parameter: field.to_string(),
            path: path.to_string(),
            error: e.to_string(),
        }),
    }
}

#[derive(Debug, Clone)]
pub struct ConfigParameter {
    pub name: &'static str,
    pub description: &'static str,
}

impl ConfigParameter {
    fn empty_paramter_error(&self) -> error::ConfigFileError {
        error::ConfigFileError::EmptyParameter {
            name: self.name.to_string(),
            description: self.description.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConfigParameters {
    tor: ConfigParameter,
    torrc: ConfigParameter,
    data_dirs: ConfigParameter,
}

static CONFIG_PARAMETERS: ConfigParameters = ConfigParameters {
    tor: ConfigParameter {
        name: "tor",
        description: "path to tor binary",
    },
    torrc: ConfigParameter {
        name: "torrc",
        description: "path to torrc (tor config) file",
    },
    data_dirs: ConfigParameter {
        name: "data_dirs",
        description: "path to tor work data root directory",
    },
};

fn check_config(config: &AppConfig) -> Result<(), error::ConfigFileError> {
    if config.tor.path.is_empty() {
        return Err(CONFIG_PARAMETERS.tor.empty_paramter_error());
    }
    if config.tor.torrc.is_empty() {
        return Err(CONFIG_PARAMETERS.torrc.empty_paramter_error());
    }
    if config.tor.data_dirs.path.is_empty() {
        return Err(CONFIG_PARAMETERS.data_dirs.empty_paramter_error());
    }
    Ok(())
}

fn init_config(config: &mut AppConfig, relative_to: PathBuf) -> Result<(), error::ConfigFileError> {
    check_config(config)?;
    config.tor.full_path =
        normalize_path_in_config(&config.tor.path, "tor.path", false, relative_to.clone())?;
    config.tor.torrc_full_path =
        normalize_path_in_config(&config.tor.torrc, "tor.torrc", false, relative_to.clone())?;
    config.tor.data_dirs.full_path =
        normalize_path_in_config(&config.tor.data_dirs.path, "data_dirs.path", true, relative_to.clone())?;
    Ok(())
}

fn remove_dir_contents<P: AsRef<Path>>(path: P) -> std::io::Result<()> {
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            remove_dir_contents(&path)?;
            std::fs::remove_dir(path)?;
        } else {
            std::fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn get_relative_to(config_file_path: PathBuf) -> PathBuf {
    let mut relative_to: PathBuf;
    let exe = std::env::current_exe().unwrap();
    if config_file_path.parent().unwrap() == exe.parent().unwrap() {
        // config near exe
        log::debug!("exe: {}", exe.to_str().unwrap());
        relative_to = exe;
        relative_to.pop();
    } else {
        // config in <project>/configs/
        relative_to = config_file_path;
        relative_to.pop();
        relative_to.pop();
    }
    relative_to
}

pub fn init() -> Result<AppConfig, Box<dyn std::error::Error>> {
    let (mut config, config_file_path) = config::load_config()?;
    let relative_to = get_relative_to(config_file_path);
    if config.log.r#use {
        let mut exe = std::env::current_exe().unwrap();
        exe.set_extension("log");
        let log_file_name = exe.file_name().unwrap().to_str().unwrap().to_string();
        let mut log_file_path =
            normalize_path_in_config(&config.log.path, "log.path", true, relative_to.clone())?;
        log_file_path = log_file_path + &log_file_name;
        let _ = init_log(log_file_path)?;
    }
    log::debug!("init...");
    log::debug!("relative_to: {}", relative_to.to_str().unwrap());
    init_config(&mut config, relative_to)?;

    let data_dirs_path = config.tor.data_dirs.full_path.clone();
    if std::fs::metadata(&data_dirs_path).is_err() {
        log::debug!("tor data dir ('{}') not exists; creating...", &data_dirs_path);
        std::fs::create_dir(&data_dirs_path).map_err(|e| error::CreateDataDirError {
            path: data_dirs_path.clone(),
            error: e.to_string()
        })?;
    }
    if config.tor.data_dirs.clear {
        log::debug!("clear data dirs ('{}')...", &data_dirs_path);
        match remove_dir_contents(&data_dirs_path) {
            Ok(_) => {
                log::debug!("clear done.");
            }
            Err(e) => {
                log::debug!("clear failed: '{}'", e.to_string());
                return Err(error::ClearDataDirError {
                    path: data_dirs_path.clone(),
                    error: e.to_string(),
                }
                .into());
            }
        }
    }
    log::debug!("init done.");
    Ok(config)
}

#[cfg(test)]
mod tests {
    use crate::init::{normalize_path, NormalizePathError};
    use std::path::PathBuf;

    fn ok_with_path(res: Result<PathBuf, NormalizePathError>, path: &str) -> bool {
        match res {
            Ok(path_buf) => {
                let res = path_buf.as_os_str().to_str().unwrap();
                if res == path {
                    true
                } else {
                    println!("{}", res);
                    false
                }
            }
            Err(e) => {
                println!("{}", e);
                false
            }
        }
    }

    fn pb(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn check_normalize_path() {
        assert!(ok_with_path(
            normalize_path(pb(".."), pb("/foo/bar/baz")),
            "/foo/bar"
        ));
        assert!(ok_with_path(
            normalize_path(pb(".."), pb("/foo/bar/baz/")),
            "/foo/bar"
        ));
        assert!(ok_with_path(
            normalize_path(pb("/../../"), pb("/foo/bar/baz/")),
            "/foo"
        ));
        assert!(ok_with_path(
            normalize_path(pb("./x"), pb("/foo/bar/baz/")),
            "/foo/bar/baz/x"
        ));
        assert!(ok_with_path(
            normalize_path(pb("../x"), pb("/foo/bar/baz/")),
            "/foo/bar/x"
        ));
        assert!(matches!(
            normalize_path(pb("../../../.."), pb("/foo/bar/baz/")),
            Err(NormalizePathError::GoesThruRoot { .. })
        ));
    }
}
