// src/core/handler_ext.rs
use crate::core::AppState;
use salvo::prelude::*;
use std::sync::Arc;

pub trait HandlerExt {
    fn app_state(&self) -> Result<Arc<AppState>, StatusError>;
}

impl HandlerExt for Depot {
    fn app_state(&self) -> Result<Arc<AppState>, StatusError> {
        // 1. obtain 返回的是 Result<&Arc<AppState>, NotFoundError>
        // 2. map(|s| Arc::clone(s)) 将 &Arc 转换为 Arc (仅仅是引用计数+1，开销很小)
        // 3. map_err 将 Depot 的查找错误转换为 HTTP StatusError
        self.get_typed::<Arc<AppState>>()
            .map(Arc::clone)
            .map_err(|_| StatusError::internal_server_error())
    }
}
