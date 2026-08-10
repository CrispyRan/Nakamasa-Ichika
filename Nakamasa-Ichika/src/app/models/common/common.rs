// 通用结构体
use serde::{Deserialize, Serialize};

/// 设备信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct DeviceInfo {
    pub udid: String,
    pub time: i64,
}
