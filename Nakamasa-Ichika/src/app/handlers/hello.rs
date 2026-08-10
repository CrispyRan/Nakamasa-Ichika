use crate::core::t;
use salvo::http::Version;
use salvo::prelude::*;
use std::collections::HashMap;
#[handler]
pub async fn hello(req: &mut Request, depot: &mut Depot) -> String {
    let version_str = match req.version() {
        Version::HTTP_10 => "HTTP/1.0",
        Version::HTTP_11 => "HTTP/1.1",
        Version::HTTP_2 => "HTTP/2",
        Version::HTTP_3 => "HTTP/3",
        _ => "Unknown",
    };
    println!("协议: {}", version_str);
    let lang = depot
        .get::<String>("language")
        .cloned()
        .unwrap_or("en".to_string());
    let mut args = HashMap::new();
    args.insert("name", lang.as_str());

    t(depot, "hello", Some(args))
}
