use std::{fmt::Display, sync::Arc};

use actix::Message;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub trait IntoAppError<R> {
    fn into_app_err<'a>(
        self,
        info: impl Into<Arc<str>>,
        kind: AppErrorKind,
        extra_details: &'a [&'a str],
    ) -> R;
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct AppError {
    kind: AppErrorKind,
    info: Arc<str>,
    detailed_info: Arc<str>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../app/src/api-types/")]
pub enum AppErrorKind {
    Queue,
    Api,
    LocalData,
    Database,
    Download,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../app/src/api-types/")]
struct UserError {
    kind: AppErrorKind,
    info: Arc<str>,
}

impl Clone for AppError {
    fn clone(&self) -> Self {
        Self {
            kind: self.kind.clone(),
            info: Arc::clone(&self.info),
            detailed_info: Arc::clone(&self.detailed_info),
        }
    }
}

impl Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let width: usize = 80;

        let kind = self.kind.to_string();
        let header = format!(
            "\n--{kind}{sep}",
            sep = "-".repeat(width.saturating_sub(kind.len() + 2))
        );
        let body = format!(
            "\nINFO: {info}\n\n{details}",
            info = self.info,
            details = self.detailed_info
        );
        let footer = "-".repeat(width);

        let msg = format!("{header}{body}\n{footer}");
        write!(f, "{msg}")
    }
}

impl Display for AppErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            Self::Api => "API ERROR",
            Self::Queue => "QUEUE ERROR",
            Self::Database => "DATABASE ERROR",
            Self::Download => "DOWNLOAD ERROR",
            Self::LocalData => "LOCAL DATA ERROR",
        };

        write!(f, "{str}")
    }
}

impl<E: Display> IntoAppError<AppError> for E {
    fn into_app_err<'a>(
        self,
        info: impl Into<Arc<str>>,
        kind: AppErrorKind,
        extra_details: &'a [&'a str],
    ) -> AppError {
        let app_err = AppError {
            kind,
            info: info.into(),
            detailed_info: AppError::format_detailed_info(self, extra_details),
        };

        log::error!("{app_err}");
        app_err
    }
}

impl<T, E> IntoAppError<Result<T, AppError>> for Result<T, E>
where
    E: IntoAppError<AppError>,
{
    fn into_app_err<'a>(
        self,
        info: impl Into<Arc<str>>,
        kind: AppErrorKind,
        extra_details: &'a [&'a str],
    ) -> Result<T, AppError> {
        self.map_err(|err| err.into_app_err(info, kind, extra_details))
    }
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let user_err = self.user_err();
        user_err.serialize(serializer)
    }
}

impl AppError {
    pub fn new(kind: AppErrorKind, info: impl Into<Arc<str>>, extra_details: &[&str]) -> Self {
        let app_err = Self {
            kind,
            info: info.into(),
            detailed_info: AppError::format_detailed_info("", extra_details),
        };

        log::error!("{app_err}");
        app_err
    }

    fn format_detailed_info<D: Display>(err: D, extra_details: &[&str]) -> Arc<str> {
        format!(
            "DETAILS:\n{extra}{err}",
            err = if err.to_string().is_empty() {
                "".to_owned()
            } else {
                format!("\n\nERROR: {err}")
            },
            extra = extra_details.join("\n")
        )
        .into()
    }

    fn user_err(&self) -> UserError {
        UserError {
            kind: self.kind.clone(),
            info: Arc::clone(&self.info),
        }
    }
}
