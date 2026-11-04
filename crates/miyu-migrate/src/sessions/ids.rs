//! 编号和时刻（细则第一节第 5 条、事件外壳的 `at`）。

/// 新会话编号：UUIDv7 的写法。时间段是 `millis`（旧会话的创建时刻），随机段取
/// `blake3("miyu-legacy\0" + db + "\0" + old)` 的前 74 位：同样的输入每次得到同一个编号，重跑导入回 `present`。
/// 用旧时刻，是因为新版照编号倒序认新旧。
pub fn session_id(millis: u64, db: &str, old: &str) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"miyu-legacy\0");
    hasher.update(db.as_bytes());
    hasher.update(b"\0");
    hasher.update(old.as_bytes());
    let hash = hasher.finalize();
    let bytes = hash.as_bytes();
    let random = u128::from_be_bytes(bytes[..16].try_into().unwrap_or([0; 16])) >> (128 - 74);
    let rand_a = (random >> 62) & 0xfff;
    let rand_b = random & ((1u128 << 62) - 1);
    let value = (u128::from(millis & 0xffff_ffff_ffff) << 80)
        | (0x7u128 << 76)
        | (rand_a << 64)
        | (0b10u128 << 62)
        | rand_b;
    let hex = format!("{value:032x}");
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}

/// 旧版的时刻（RFC 3339，可能带纳秒、`+00:00`）。读不懂的是 `None`。
pub fn parse(text: &str) -> Option<jiff::Timestamp> {
    text.trim().parse::<jiff::Timestamp>().ok()
}

/// 事件外壳的 `at`：UTC、毫秒、24 个字。
pub fn at(time: jiff::Timestamp) -> String {
    time.strftime("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

/// Unix 毫秒换成 `at` 的写法；出了范围的是 `None`。
pub fn at_millis(millis: u64) -> Option<String> {
    let millis = i64::try_from(millis).ok()?;
    jiff::Timestamp::from_millisecond(millis).ok().map(at)
}
