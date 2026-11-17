//! Bắt panic khi actor xử lý một sự kiện.
//!
//! Actor panic thì task của nó dừng, nhưng process Java vẫn chạy: không còn ai gửi `stop`, nút Tắt
//! trả lỗi, và lúc thoát MineHost Java bị kill không kịp lưu world. Bắt panic quanh từng sự kiện thì
//! chỉ mất sự kiện đó, actor vẫn quản server tiếp.

use std::any::Any;
use std::future::Future;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::pin::Pin;
use std::task::{Context, Poll};

type Panic = Box<dyn Any + Send>;

/// Chạy `fut` tới xong; panic trong lúc poll thành `Err`.
pub(crate) fn catch_panic<'a, T: 'a>(
    fut: impl Future<Output = T> + Send + 'a,
) -> impl Future<Output = Result<T, Panic>> + Send + 'a {
    CatchPanic {
        inner: Box::pin(fut),
    }
}

struct CatchPanic<'a, T> {
    inner: Pin<Box<dyn Future<Output = T> + Send + 'a>>,
}

impl<T> Future for CatchPanic<'_, T> {
    type Output = Result<T, Panic>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Trạng thái actor có thể dở dang sau panic; vẫn tốt hơn để Java chạy mà không ai quản.
        match catch_unwind(AssertUnwindSafe(|| self.inner.as_mut().poll(cx))) {
            Ok(Poll::Ready(value)) => Poll::Ready(Ok(value)),
            Ok(Poll::Pending) => Poll::Pending,
            Err(panic) => Poll::Ready(Err(panic)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn panic_becomes_err_and_value_passes_through() {
        assert_eq!(catch_panic(async { 7 }).await.ok(), Some(7));
        let caught = catch_panic(async {
            tokio::task::yield_now().await;
            panic!("hỏng giữa chừng");
        })
        .await;
        assert!(caught.is_err());
    }
}
