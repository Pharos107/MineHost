use std::collections::HashMap;

use axum::Json;
use axum::extract::{FromRequest, FromRequestParts, Path, Request};
use axum::http::StatusCode;
use axum::http::request::Parts;
use axum::response::{IntoResponse, Response};
use mh_app::AppError;
use mh_core::{ErrorCode, ServerId};
use serde::Serialize;
use serde::de::DeserializeOwned;
use utoipa::ToSchema;

/// Thân response khi có lỗi. Frontend dịch theo `code`; backend không gửi câu chữ đã dịch.
#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorBody {
    /// Một giá trị của `ErrorCode`, ví dụ `NOT_FOUND`.
    pub code: String,
    /// Tham số để ghép vào bản dịch, ví dụ `{ "port": 25565 }`.
    #[schema(value_type = Object)]
    pub params: serde_json::Value,
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: ErrorCode,
    params: serde_json::Value,
}

impl ApiError {
    pub fn new(status: StatusCode, code: ErrorCode) -> Self {
        Self {
            status,
            code,
            params: serde_json::Value::Object(Default::default()),
        }
    }

    pub fn with_params(mut self, params: serde_json::Value) -> Self {
        self.params = params;
        self
    }

    /// Lỗi không lường trước. Chi tiết chỉ ghi vào log, không gửi cho client.
    pub fn internal(detail: impl std::fmt::Display) -> Self {
        tracing::error!(%detail, "lỗi nội bộ khi xử lý request");
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, ErrorCode::Internal)
    }

    /// Dữ liệu gửi lên sai; `field` cho UI biết ô nào cần sửa.
    pub fn invalid(field: &str) -> Self {
        Self::new(StatusCode::BAD_REQUEST, ErrorCode::InvalidInput)
            .with_params(serde_json::json!({ "field": field }))
    }
}

/// HTTP status ứng với từng mã lỗi.
///
/// `UNSUPPORTED_BY_VERSION` là 409: thao tác không làm được với bản Minecraft này,
/// cùng nhóm với các từ chối vì trạng thái, không phải lỗi của controller.
fn status_for(code: ErrorCode) -> StatusCode {
    match code {
        ErrorCode::NotFound | ErrorCode::VersionNotFound => StatusCode::NOT_FOUND,
        ErrorCode::InvalidInput | ErrorCode::PathOutsideRoot => StatusCode::BAD_REQUEST,
        ErrorCode::Forbidden => StatusCode::FORBIDDEN,
        ErrorCode::ServerBusy
        | ErrorCode::EulaNotAccepted
        | ErrorCode::PortInUse
        | ErrorCode::JobCancelled
        | ErrorCode::ServerCrashed
        | ErrorCode::UnsupportedByVersion
        | ErrorCode::BackupProtected => StatusCode::CONFLICT,
        ErrorCode::NetworkUnavailable | ErrorCode::DownloadFailed | ErrorCode::ChecksumMismatch => {
            StatusCode::BAD_GATEWAY
        }
        ErrorCode::NotEnoughDisk => StatusCode::INSUFFICIENT_STORAGE,
        ErrorCode::ConfigInvalid | ErrorCode::BackupCorrupt => StatusCode::UNPROCESSABLE_ENTITY,
        ErrorCode::FileTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
        // File nhị phân không đọc được như văn bản: đúng kiểu 415, không phải 400.
        ErrorCode::FileNotText => StatusCode::UNSUPPORTED_MEDIA_TYPE,
        // `If-Match` lệch là precondition HTTP (RFC 9110). 409 dành cho tên đã có.
        ErrorCode::FileChanged => StatusCode::PRECONDITION_FAILED,
        ErrorCode::FileExists => StatusCode::CONFLICT,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

impl From<AppError> for ApiError {
    fn from(err: AppError) -> Self {
        let status = status_for(err.code);
        if status.is_server_error() {
            tracing::error!(error = %err, "lỗi khi xử lý request");
        } else {
            tracing::debug!(error = %err, "request bị từ chối");
        }
        Self::new(status, err.code).with_params(err.params)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ErrorBody {
            code: self.code.as_str().to_owned(),
            params: self.params,
        };
        (self.status, Json(body)).into_response()
    }
}

pub async fn api_not_found() -> ApiError {
    ApiError::new(StatusCode::NOT_FOUND, ErrorCode::NotFound)
}

/// Như `Json`, nhưng body sai trả `ErrorBody` với `INVALID_INPUT` thay vì chữ của axum.
#[derive(Debug)]
pub struct ApiJson<T>(pub T);

impl<S: Send + Sync, T: DeserializeOwned> FromRequest<S> for ApiJson<T> {
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(req, state).await {
            Ok(Json(value)) => Ok(Self(value)),
            Err(rejection) => {
                tracing::debug!(%rejection, "body JSON không hợp lệ");
                // 413 là giới hạn body mặc định của axum (2 MiB). Giữ mã này để khỏi
                // lẫn với JSON sai cú pháp. Route upload nâng limit riêng.
                let status = match rejection.status() {
                    StatusCode::PAYLOAD_TOO_LARGE => StatusCode::PAYLOAD_TOO_LARGE,
                    StatusCode::UNSUPPORTED_MEDIA_TYPE => StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    _ => StatusCode::BAD_REQUEST,
                };
                Err(ApiError::new(status, ErrorCode::InvalidInput))
            }
        }
    }
}

/// `{id}` trong đường dẫn, đã kiểm tra là ULID trước khi được ghép vào đường dẫn trên đĩa.
#[derive(Debug, Clone, Copy)]
pub struct IdPath(pub ServerId);

impl<S: Send + Sync> FromRequestParts<S> for IdPath {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // Đọc theo tên, vì có route còn tham số khác sau `{id}` (ví dụ `{action}`).
        let Path(params) = Path::<HashMap<String, String>>::from_request_parts(parts, state)
            .await
            .map_err(|_| ApiError::invalid("id"))?;
        let raw = params.get("id").ok_or_else(|| ApiError::invalid("id"))?;
        ServerId::parse(raw)
            .map(Self)
            .map_err(|_| ApiError::invalid("id"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_known_code_has_a_sensible_status() {
        assert_eq!(status_for(ErrorCode::EulaNotAccepted), StatusCode::CONFLICT);
        assert_eq!(
            status_for(ErrorCode::UnsupportedByVersion),
            StatusCode::CONFLICT
        );
        assert_eq!(status_for(ErrorCode::Forbidden), StatusCode::FORBIDDEN);
        assert_eq!(
            status_for(ErrorCode::Internal),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            status_for(ErrorCode::VersionNotFound),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            status_for(ErrorCode::FileTooLarge),
            StatusCode::PAYLOAD_TOO_LARGE
        );
        assert_eq!(
            status_for(ErrorCode::FileNotText),
            StatusCode::UNSUPPORTED_MEDIA_TYPE
        );
        assert_eq!(
            status_for(ErrorCode::FileChanged),
            StatusCode::PRECONDITION_FAILED
        );
        assert_eq!(status_for(ErrorCode::FileExists), StatusCode::CONFLICT);
        assert_eq!(
            status_for(ErrorCode::BackupCorrupt),
            StatusCode::UNPROCESSABLE_ENTITY
        );
        assert_eq!(status_for(ErrorCode::BackupProtected), StatusCode::CONFLICT);
    }
}
