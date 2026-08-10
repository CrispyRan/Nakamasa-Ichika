//! GeoIP 地域查询模块
//!
//! 基于 MaxMind GeoLite2 数据库提供 IP 地址地理位置查询功能。
//!
//! # 功能
//! - 查询 IP 所属国家、省份、城市
//! - 返回经纬度坐标
//! - 支持多语言（默认中文）
//!
//! # Example
//! ```rust,ignore
//! use nakamasa_utils::geoip::{GeoIpReader, GeoLocation};
//!
//! let reader = GeoIpReader::new("GeoLite2-City.mmdb")?;
//! let location = reader.lookup("8.8.8.8")?;
//! println!("国家: {}, 城市: {}", location.country, location.city);
//! ```

use maxminddb::geoip2::{Asn, City};
use maxminddb::Reader;
use std::net::IpAddr;
use std::path::Path;
use std::sync::Arc;

/// IP 地域信息
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct GeoLocation {
    /// 国家名称
    pub country: String,
    /// 省份/州名称
    pub province: String,
    /// 城市名称
    pub city: String,
    /// 纬度
    pub latitude: Option<f64>,
    /// 经度
    pub longitude: Option<f64>,
    /// 国家 ISO 代码 (如 CN, US)
    pub country_code: String,
    /// 自治系统编号 (ASN)
    pub asn: Option<u32>,
    /// 运营商/组织名称
    pub isp: Option<String>,
}

impl GeoLocation {
    /// 创建空的地理位置信息
    pub fn empty() -> Self {
        GeoLocation {
            country: String::new(),
            province: String::new(),
            city: String::new(),
            latitude: None,
            longitude: None,
            country_code: String::new(),
            asn: None,
            isp: None,
        }
    }

    /// 检查是否有有效的地理位置信息
    pub fn is_valid(&self) -> bool {
        !self.country.is_empty() || !self.city.is_empty()
    }

    /// 格式化为简短字符串 (如 "中国 北京")
    pub fn to_short_string(&self) -> String {
        let mut parts = Vec::with_capacity(3);
        if !self.country.is_empty() {
            parts.push(self.country.as_str());
        }
        if !self.province.is_empty() && self.province != self.country {
            parts.push(self.province.as_str());
        }
        if !self.city.is_empty() && self.city != self.province {
            parts.push(self.city.as_str());
        }
        parts.join(" ")
    }
}

/// GeoIP 查询器
///
/// 封装 MaxMind City 和 ASN 数据库读取器，提供 IP 地理位置 + 运营商查询功能。
/// 内部使用 `Arc` 包装，支持多线程共享。
pub struct GeoIpReader {
    city: Arc<Reader<Vec<u8>>>,
    asn: Option<Arc<Reader<Vec<u8>>>>,
}

impl GeoIpReader {
    /// 从文件路径创建 GeoIP 查询器（仅 City 数据库）
    ///
    /// # Arguments
    /// * `path` - GeoLite2-City.mmdb 文件路径
    ///
    /// # Errors
    /// 如果文件不存在或格式无效，返回错误
    ///
    /// # Example
    /// ```rust,ignore
    /// let reader = GeoIpReader::new("GeoLite2-City.mmdb")?;
    /// ```
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, GeoIpError> {
        let reader =
            Reader::open_readfile(path).map_err(|e| GeoIpError::OpenFailed(e.to_string()))?;
        Ok(GeoIpReader {
            city: Arc::new(reader),
            asn: None,
        })
    }

    /// 从文件路径创建 GeoIP 查询器（同时加载 City + ASN 数据库）
    ///
    /// # Arguments
    /// * `city_path` - GeoLite2-City.mmdb 文件路径
    /// * `asn_path` - GeoLite2-ASN.mmdb 文件路径
    ///
    /// # Errors
    /// 任一文件不存在或格式无效，返回错误
    ///
    /// # Example
    /// ```rust,ignore
    /// let reader = GeoIpReader::new_with_asn("GeoLite2-City.mmdb", "GeoLite2-ASN.mmdb")?;
    /// ```
    pub fn new_with_asn<P: AsRef<Path>>(
        city_path: P,
        asn_path: P,
    ) -> Result<Self, GeoIpError> {
        let city =
            Reader::open_readfile(city_path).map_err(|e| GeoIpError::OpenFailed(e.to_string()))?;
        let asn =
            Reader::open_readfile(asn_path).map_err(|e| GeoIpError::OpenFailed(e.to_string()))?;
        Ok(GeoIpReader {
            city: Arc::new(city),
            asn: Some(Arc::new(asn)),
        })
    }

    /// 查询 IP 地址的地理位置
    ///
    /// # Arguments
    /// * `ip_str` - IP 地址字符串 (如 "8.8.8.8" 或 "2001:db8::1")
    ///
    /// # Returns
    /// 返回 `GeoLocation` 结构体，包含国家、省份、城市、经纬度
    ///
    /// # Example
    /// ```rust,ignore
    /// let location = reader.lookup("8.8.8.8")?;
    /// println!("国家: {}, 城市: {}", location.country, location.city);
    /// ```
    pub fn lookup(&self, ip_str: &str) -> Result<GeoLocation, GeoIpError> {
        let ip: IpAddr = ip_str
            .parse()
            .map_err(|e: std::net::AddrParseError| GeoIpError::InvalidIp(e.to_string()))?;

        let city: City = self
            .city
            .lookup(ip)
            .map_err(|e| GeoIpError::LookupFailed(e.to_string()))?
            .ok_or(GeoIpError::NoData)?;

        Ok(self.extract_location(&city))
    }

    /// 查询 IP 地址的地理位置和 ASN/运营商信息
    ///
    /// # Arguments
    /// * `ip_str` - IP 地址字符串 (如 "8.8.8.8" 或 "2001:db8::1")
    ///
    /// # Returns
    /// 返回 `GeoLocation` 结构体，额外包含 ASN 编号和运营商名称
    /// ASN 查询失败不影响地理位置数据返回
    ///
    /// # Example
    /// ```rust,ignore
    /// let location = reader.lookup_with_asn("8.8.8.8")?;
    /// println!("运营商: {:?}, ASN: {:?}", location.isp, location.asn);
    /// ```
    pub fn lookup_with_asn(&self, ip_str: &str) -> Result<GeoLocation, GeoIpError> {
        let ip: IpAddr = ip_str
            .parse()
            .map_err(|e: std::net::AddrParseError| GeoIpError::InvalidIp(e.to_string()))?;

        let city: City = self
            .city
            .lookup(ip)
            .map_err(|e| GeoIpError::LookupFailed(e.to_string()))?
            .ok_or(GeoIpError::NoData)?;

        let mut loc = self.extract_location(&city);

        if let Some(ref asn_reader) = self.asn {
            if let Ok(Some(asn_data)) = asn_reader.lookup::<Asn>(ip) {
                loc.asn = asn_data.autonomous_system_number;
                loc.isp = asn_data
                    .autonomous_system_organization
                    .map(|s| s.to_string());
            }
        }

        Ok(loc)
    }

    /// 从 City 结构中提取地理位置信息
    fn extract_location(&self, city: &City) -> GeoLocation {
        GeoLocation {
            // 国家名称 (中文优先)
            country: city
                .country
                .as_ref()
                .and_then(|c| c.names.as_ref())
                .and_then(|n| n.get("zh-CN").map(|s| s.to_string()))
                .or_else(|| {
                    city.country
                        .as_ref()
                        .and_then(|c| c.names.as_ref())
                        .and_then(|n| n.get("en").map(|s| s.to_string()))
                })
                .unwrap_or_default(),

            // 省份/州名称 (中文优先)
            province: city
                .subdivisions
                .as_ref()
                .and_then(|s| s.first())
                .and_then(|s| s.names.as_ref())
                .and_then(|n| n.get("zh-CN").map(|s| s.to_string()))
                .or_else(|| {
                    city.subdivisions
                        .as_ref()
                        .and_then(|s| s.first())
                        .and_then(|s| s.names.as_ref())
                        .and_then(|n| n.get("en").map(|s| s.to_string()))
                })
                .unwrap_or_default(),

            // 城市名称 (中文优先)
            city: city
                .city
                .as_ref()
                .and_then(|c| c.names.as_ref())
                .and_then(|n| n.get("zh-CN").map(|s| s.to_string()))
                .or_else(|| {
                    city.city
                        .as_ref()
                        .and_then(|c| c.names.as_ref())
                        .and_then(|n| n.get("en").map(|s| s.to_string()))
                })
                .unwrap_or_default(),

            // 经纬度
            latitude: city.location.as_ref().and_then(|loc| loc.latitude),
            longitude: city.location.as_ref().and_then(|loc| loc.longitude),

            // 国家 ISO 代码
            country_code: city
                .country
                .as_ref()
                .and_then(|c| c.iso_code.as_ref().map(|s| s.to_string()))
                .unwrap_or_default(),

            asn: None,
            isp: None,
        }
    }

    /// 批量查询多个 IP 地址
    ///
    /// # Arguments
    /// * `ips` - IP 地址字符串切片
    ///
    /// # Returns
    /// 返回 IP 地址与地理位置的映射
    pub fn lookup_batch(&self, ips: &[&str]) -> Vec<(String, GeoLocation)> {
        ips.iter()
            .map(|ip| {
                let location = self.lookup(ip).unwrap_or_default();
                ((*ip).to_string(), location)
            })
            .collect()
    }

    pub fn clone_city_reader(&self) -> Arc<Reader<Vec<u8>>> {
        Arc::clone(&self.city)
    }

    /// 克隆内部 ASN Reader 的 Arc 引用
    pub fn clone_asn_reader(&self) -> Option<Arc<Reader<Vec<u8>>>> {
        self.asn.as_ref().map(Arc::clone)
    }
}

impl Clone for GeoIpReader {
    fn clone(&self) -> Self {
        GeoIpReader {
            city: Arc::clone(&self.city),
            asn: self.asn.as_ref().map(Arc::clone),
        }
    }
}

/// GeoIP 错误类型
#[derive(Debug)]
pub enum GeoIpError {
    /// 数据库文件打开失败
    OpenFailed(String),
    /// IP 地址格式无效
    InvalidIp(String),
    /// 查询失败
    LookupFailed(String),
    /// 无数据
    NoData,
}

impl std::fmt::Display for GeoIpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GeoIpError::OpenFailed(msg) => write!(f, "GeoIP 数据库打开失败: {}", msg),
            GeoIpError::InvalidIp(msg) => write!(f, "IP 地址格式无效: {}", msg),
            GeoIpError::LookupFailed(msg) => write!(f, "IP 查询失败: {}", msg),
            GeoIpError::NoData => write!(f, "无地理位置数据"),
        }
    }
}

impl std::error::Error for GeoIpError {}

/// 测试函数 - 验证 GeoIP 功能
pub fn position() -> Result<(), Box<dyn std::error::Error>> {
    let reader = GeoIpReader::new("GeoLite2-City.mmdb")?;
    let location = reader.lookup("8.8.8.8")?;

    println!(
        "国家: {}, 省份: {}, 城市: {}, 经纬度: ({:?}, {:?})",
        location.country, location.province, location.city, location.latitude, location.longitude
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_geo_location_empty() {
        let loc = GeoLocation::empty();
        assert!(!loc.is_valid());
        assert!(loc.to_short_string().is_empty());
    }

    #[test]
    fn test_geo_location_short_string() {
        let loc = GeoLocation {
            country: "中国".to_string(),
            province: "北京".to_string(),
            city: "北京".to_string(),
            latitude: Some(39.9),
            longitude: Some(116.4),
            country_code: "CN".to_string(),
            asn: Some(15169),
            isp: Some("Google LLC".to_string()),
        };
        assert_eq!(loc.to_short_string(), "中国 北京");
    }
}
