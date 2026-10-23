use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use mh_core::ErrorCode;
use serde::Serialize;
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
