use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeCode {
    Ok,
    NotReady,
    Conflict,
    Unsupported,
    Misconfigured,
    InvalidInput,
    NotFound,
    Internal,
}

#[derive(Debug, Clone, Serialize)]
pub struct OperationOutcome<T>
where
    T: Serialize,
{
    pub ok: bool,
    pub code: OutcomeCode,
    pub message: String,
    pub timestamp: String,
    pub data: T,
}

impl<T> OperationOutcome<T>
where
    T: Serialize,
{
    pub fn success(message: impl Into<String>, data: T) -> Self {
        Self {
            ok: true,
            code: OutcomeCode::Ok,
            message: message.into(),
            timestamp: utc_timestamp(),
            data,
        }
    }

    pub fn failure(code: OutcomeCode, message: impl Into<String>, data: T) -> Self {
        Self {
            ok: false,
            code,
            message: message.into(),
            timestamp: utc_timestamp(),
            data,
        }
    }
}

pub fn utc_timestamp() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .expect("rfc3339 timestamp")
}
