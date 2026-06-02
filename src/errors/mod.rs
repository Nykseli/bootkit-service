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
    /// Generic error when nothing else is applicable
    Error(String),
    GrubParse(String),
    Io(String, Box<std::io::Error>),
    Sqlx(String, Box<sqlx::Error>),
    Zbus(String, Box<zbus::Error>),
    Serde(String, Box<serde_json::Error>),
    JoinError(String, Box<tokio::task::JoinError>),
    ThreadError(String, Box<dyn std::any::Any + Send + 'static>),
}

impl DErrorType {
    pub fn as_string(&self) -> String {
        match self {
            DErrorType::Error(msg) => format!("Error: {msg}"),
            DErrorType::GrubParse(msg) => {
                format!("Internal Parse: Failed to parse grub config: {msg}")
            }
            DErrorType::Io(msg, error) => format!("Internal IO error: {msg} ({error})"),
            DErrorType::Sqlx(msg, error) => format!("Interal database error: {msg} ({error})"),
            DErrorType::Zbus(msg, error) => format!("Internal zbus error: {msg} ({error})"),
            DErrorType::Serde(msg, error) => format!("Json handling error: {msg} ({error})"),
            DErrorType::JoinError(msg, error) => format!("Task runtime error: {msg} ({error})"),
            DErrorType::ThreadError(msg, error) => format!("Thread error: {msg} ({error:?})"),
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
    /// Origin where error happened
    ctx: DCtx,
    /// Additional places and messages where error was propagated, excluding the origin
    trace: Vec<(String, DCtx)>,
    error: DErrorType,
}

impl DError {
    pub fn new(ctx: DCtx, error: DErrorType) -> Self {
        Self {
            ctx,
            error,
            trace: Vec::new(),
        }
    }

    pub fn generic<M: Into<String>>(ctx: DCtx, message: M) -> Self {
        Self::new(ctx, DErrorType::Error(message.into()))
    }

    fn with_trace<M: Into<String>>(mut self, ctx: DCtx, message: M) -> Self {
        let message = message.into();
        self.trace.push((message, ctx));
        self
    }

    pub fn grub_parse_error<M: Into<String>>(ctx: DCtx, message: M) -> Self {
        Self::new(ctx, DErrorType::GrubParse(message.into()))
    }

    pub fn error(&self) -> &DErrorType {
        &self.error
    }
}

/// We know that DError propagation stops when it's dropped so it's the perfect
/// opportunity to log it
impl Drop for DError {
    fn drop(&mut self) {
        log::error!("Error at {}: {}", self.ctx, self.error());
        for (idx, (message, ctx)) in self.trace.iter().enumerate() {
            log::debug!("    trace [{}] {ctx}: {message}", idx + 1);
        }
    }
}

impl From<DError> for zbus::fdo::Error {
    fn from(value: DError) -> Self {
        Self::Failed(value.error().as_string())
    }
}

pub type DResult<T> = core::result::Result<T, DError>;

pub trait DResOption<T> {
    fn flat_ctx<M: Into<String>>(self, ctx: DCtx, msg: M) -> DResult<T>;
}

impl<T> DResOption<T> for DResult<Option<T>> {
    fn flat_ctx<M: Into<String>>(self, ctx: DCtx, msg: M) -> DResult<T> {
        match self {
            Ok(Some(value)) => Ok(value),
            Ok(None) => Err(DError::generic(ctx, msg)),
            Err(err) => Err(err.with_trace(ctx, msg)),
        }
    }
}

pub trait DRes<T> {
    fn ctx<M: Into<String>>(self, ctx: DCtx, msg: M) -> DResult<T>;
}

impl<T> DRes<T> for DResult<T> {
    fn ctx<M: Into<String>>(self, ctx: DCtx, msg: M) -> DResult<T> {
        match self {
            Ok(value) => Ok(value),
            Err(err) => Err(err.with_trace(ctx, msg)),
        }
    }
}

impl<T> DRes<T> for core::option::Option<T> {
    fn ctx<M: Into<String>>(self, ctx: DCtx, msg: M) -> DResult<T> {
        match self {
            Some(value) => Ok(value),
            None => Err(DError::generic(ctx, msg)),
        }
    }
}

impl<T> DRes<T> for std::io::Result<T> {
    fn ctx<M: Into<String>>(self, ctx: DCtx, msg: M) -> DResult<T> {
        match self {
            Ok(value) => Ok(value),
            Err(err) => Err(DError::new(ctx, DErrorType::Io(msg.into(), Box::new(err)))),
        }
    }
}

impl<T> DRes<T> for sqlx::Result<T> {
    fn ctx<M: Into<String>>(self, ctx: DCtx, msg: M) -> DResult<T> {
        match self {
            Ok(value) => Ok(value),
            Err(err) => Err(DError::new(
                ctx,
                DErrorType::Sqlx(msg.into(), Box::new(err)),
            )),
        }
    }
}

impl<T> DRes<T> for zbus::Result<T> {
    fn ctx<M: Into<String>>(self, ctx: DCtx, msg: M) -> DResult<T> {
        match self {
            Ok(value) => Ok(value),
            Err(err) => Err(DError::new(
                ctx,
                DErrorType::Zbus(msg.into(), Box::new(err)),
            )),
        }
    }
}

impl<T> DRes<T> for serde_json::Result<T> {
    fn ctx<M: Into<String>>(self, ctx: DCtx, msg: M) -> DResult<T> {
        match self {
            Ok(value) => Ok(value),
            Err(err) => Err(DError::new(
                ctx,
                DErrorType::Serde(msg.into(), Box::new(err)),
            )),
        }
    }
}

impl<T> DRes<T> for std::thread::Result<T> {
    fn ctx<M: Into<String>>(self, ctx: DCtx, msg: M) -> DResult<T> {
        match self {
            Ok(value) => Ok(value),
            Err(err) => Err(DError::new(
                ctx,
                DErrorType::ThreadError(msg.into(), Box::new(err)),
            )),
        }
    }
}

// Tokio spawn result
impl<T> DRes<T> for core::result::Result<T, tokio::task::JoinError> {
    fn ctx<M: Into<String>>(self, ctx: DCtx, msg: M) -> DResult<T> {
        match self {
            Ok(value) => Ok(value),
            Err(err) => Err(DError::new(
                ctx,
                DErrorType::JoinError(msg.into(), Box::new(err)),
            )),
        }
    }
}
