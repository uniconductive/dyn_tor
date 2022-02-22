#[derive(thiserror::Error, Debug, Clone)]
pub enum TorSpawnError {
    #[error("tor binary not found: '{path}'")]
    NotFound { path: String },
    #[error("other spawn error of '{path}' : '{error}'")]
    Other { path: String, error: String },
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum ConfigFileError {
    #[error("can't normalize config parameter '{parameter}' = '{path}', error: '{error}'")]
    NormalizePath {
        parameter: String,
        path: String,
        error: String,
    },
    #[error("parameter '{name}' ({description}) can not be empty")]
    EmptyParameter { name: String, description: String },
}

#[derive(thiserror::Error, Debug, Clone)]
#[error("can't clear tor data directory '{path}': '{error}'")]
pub struct ClearDataDirError {
    pub path: String,
    pub error: String,
}

#[derive(thiserror::Error, Debug, Clone)]
#[error("can't create tor data directory '{path}': '{error}'")]
pub struct CreateDataDirError {
    pub path: String,
    pub error: String,
}
