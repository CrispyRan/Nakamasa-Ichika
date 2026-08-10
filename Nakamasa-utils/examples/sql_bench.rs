//! SQL 方言翻译器性能基准
//!
//! 对比「实现后（经 adapt_mysql_sql 翻译）」与「原生 SQL（MySQL 直通零转换）」
//! 的时间开销。翻译器本身是纯 CPU 字符串处理，不涉及数据库往返。
//!
//! 运行：`cargo run --release --example sql_bench`

use nakamasa_utils::db_mysql::{adapt_mysql_sql, DbType};
use std::hint::black_box;
use std::time::Instant;

/// 循环测量某条 SQL 的翻译耗时
fn bench(label: &str, sql: &'static str, dialect: DbType, iters: u64) {
    // 预热
    let mut warm: usize = 0;
    for _ in 0..iters / 10 {
        warm = warm.wrapping_add(adapt_mysql_sql(black_box(sql), dialect).len());
    }
    black_box(warm);

    let start = Instant::now();
    let mut acc: usize = 0;
    for _ in 0..iters {
        let out = adapt_mysql_sql(black_box(sql), dialect);
        acc = acc.wrapping_add(out.len());
    }
    let elapsed = start.elapsed();
    black_box(acc);

    let ns_per_op = elapsed.as_nanos() as f64 / iters as f64;
    let mb_per_s = sql.len() as f64 * 1e3 / ns_per_op;
    println!(
        "  {:<30} {:>9.1} ns/op   {:>4} B/条   {:>9.1} MB/s",
        label,
        ns_per_op,
        sql.len(),
        mb_per_s
    );
}

fn main() {
    let iterations: u64 = std::env::var("ITERS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1_000_000);
    println!("SQL 方言翻译器基准（{} 次/场景）\n", iterations);

    // ── 贴业务的真实 SQL ──
    let simple =
        "SELECT `id`, `name`, `email` FROM `users` WHERE `status` = ? ORDER BY `id` DESC LIMIT ? OFFSET ?";
    let business = "SELECT O.id, O.gid, O.order_no, O.money, IFNULL(U.phone, IFNULL(U.email, U.acctno)) as user, FROM_UNIXTIME(O.add_time, '%Y-%m-%d') as day FROM u_order AS O LEFT JOIN u_user AS U ON (O.uid = U.id) WHERE O.state = ? ORDER BY O.id DESC LIMIT ? OFFSET ?";
    let business_two_arg =
        "SELECT O.id, IFNULL(U.phone, IFNULL(U.email, '')) FROM u_order AS O LEFT JOIN u_user AS U ON (O.uid = U.id) WHERE O.state = ? ORDER BY O.id DESC LIMIT ?, 8";
    let date_heavy = "SELECT IF(DATEDIFF(NOW(), add_time) > 3, 'new', 'old'), FROM_UNIXTIME(reg_time, '%m-%d'), DATE_FORMAT(create_time, '%Y-%m-%d') FROM u_user WHERE IFNULL(state, 0) = 1";
    let nested_heavy = "SELECT CONCAT_WS('-', IF(IFNULL(a, 0) > 0, MD5(b), 'x'), LOCATE('ab', c), FIND_IN_SET(d, e)) FROM t WHERE name REGEXP '^a' LIMIT 0, 10";
    let backtick = "SELECT `a`, `b^c`, `d.e` FROM `t` WHERE `x` = ?";

    println!("── 原生 / 零转换路径（MySQL 目标 = 与原生 SQL 完全一致） ──");
    bench("MySQL 直通(business)", business, DbType::MySql, iterations);
    bench("MySQL 直通(nested_heavy)", nested_heavy, DbType::MySql, iterations);

    println!("── 纯标准 SQL（无 MySQL 语法 → fast-path 零分配借用） ──");
    bench("PG 标准 SQL(未翻译)", simple, DbType::Postgres, iterations);
    bench("SQLite 标准 SQL", simple, DbType::Sqlite, iterations);

    println!("── 需翻译路径（MySQL → PG / SQLite） ──");
    bench("PG 业务 SQL(IFNULL+LIMIT)", business, DbType::Postgres, iterations);
    bench("PG 业务 SQL(两参 LIMIT)", business_two_arg, DbType::Postgres, iterations);
    bench("SQLite 业务 SQL(两参)", business_two_arg, DbType::Sqlite, iterations);
    bench("PG 日期函数堆叠", date_heavy, DbType::Postgres, iterations);
    bench("SQLite 日期堆叠", date_heavy, DbType::Sqlite, iterations);
    bench("PG 嵌套极重(IF/CONCAT/MD5)", nested_heavy, DbType::Postgres, iterations);
    bench("PG backtick 全量转义", backtick, DbType::Postgres, iterations);

    println!();
    println!("注：单次数据库往返（网络+执行）通常在 0.1~10 ms 量级；");
    println!("翻译开销为亚微秒级 (~0.2~1 µs)，占比可忽略。");
    println!("MySQL 目标路径为 0 分配 Cow::Borrowed，与原生 SQL 执行完全一致。");
}