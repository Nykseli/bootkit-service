mod macros;

/// Error context that should be created with `dctx!()` macro
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DCtx(String);

impl DCtx {
    pub fn new(inner: String) -> Self {
        Self(inner)
    }
}

impl std::fmt::Display for DCtx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum DErrorType {
    GrubParse(String),
    Io(String, std::io::Error),
    Sqlx(String, sqlx::Error),
    Zbus(String, zbus::Error),
    Serde(String, serde_json::Error),
}

impl DErrorType {
    pub fn as_string(&self) -> String {
        match self {
            DErrorType::GrubParse(msg) => {
                format!("Internal Parse: Failed to parse grub config: {msg}")
            }
            DErrorType::Io(msg, error) => format!("Internal IO error: {msg} ({error})"),
            DErrorType::Sqlx(msg, error) => format!("Interal database error: {msg} ({error})"),
            DErrorType::Zbus(msg, error) => format!("Internal zbus error: {msg} ({error})"),
            DErrorType::Serde(msg, error) => format!("Json handling error: {msg} ({error})"),
        }
    }
}

impl std::fmt::Display for DErrorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_string())
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct DError {
    ctx: DCtx,
    error: DErrorType,
}

impl DError {
    fn new(ctx: DCtx, error: DErrorType) -> Self {
        log::debug!("Error at {ctx}: {error}");
        Self { ctx, error }
    }

    pub fn grub_parse_error<M: Into<String>>(ctx: DCtx, message: M) -> DResult<()> {
        Err(Self::new(ctx, DErrorType::GrubParse(message.into())))
    }

    pub fn error(&self) -> &DErrorType {
        &self.error
    }
}

pub type DResult<T> = core::result::Result<T, DError>;

pub trait DRes<T> {
    fn ctx<M: Into<String>>(self, ctx: DCtx, msg: M) -> DResult<T>;
}

impl<T> DRes<T> for std::io::Result<T> {
    fn ctx<M: Into<String>>(self, ctx: DCtx, msg: M) -> DResult<T> {
        match self {
            Ok(value) => Ok(value),
            Err(err) => Err(DError::new(ctx, DErrorType::Io(msg.into(), err))),
        }
    }
}

impl<T> DRes<T> for sqlx::Result<T> {
    fn ctx<M: Into<String>>(self, ctx: DCtx, msg: M) -> DResult<T> {
        match self {
            Ok(value) => Ok(value),
            Err(err) => Err(DError::new(ctx, DErrorType::Sqlx(msg.into(), err))),
        }
    }
}

impl<T> DRes<T> for zbus::Result<T> {
    fn ctx<M: Into<String>>(self, ctx: DCtx, msg: M) -> DResult<T> {
        match self {
            Ok(value) => Ok(value),
            Err(err) => Err(DError::new(ctx, DErrorType::Zbus(msg.into(), err))),
        }
    }
}

impl<T> DRes<T> for serde_json::Result<T> {
    fn ctx<M: Into<String>>(self, ctx: DCtx, msg: M) -> DResult<T> {
        match self {
            Ok(value) => Ok(value),
            Err(err) => Err(DError::new(ctx, DErrorType::Serde(msg.into(), err))),
        }
    }
}
