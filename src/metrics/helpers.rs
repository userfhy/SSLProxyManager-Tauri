#[inline]
pub fn normalize_request_path_for_top(path: &str) -> String {
    let p = path.trim();
    if p.is_empty() {
        return "(empty)".to_string();
    }
    let without_query = p.split('?').next().unwrap_or(p);
    if without_query.is_empty() {
        return "(empty)".to_string();
    }

    let mut out = String::with_capacity(without_query.len());
    for seg in without_query.split('/') {
        if seg.is_empty() {
            continue;
        }
        out.push('/');
        if seg.len() <= 20 && seg.chars().all(|c| c.is_ascii_digit()) {
            out.push_str(":id");
        } else {
            out.push_str(seg);
        }
    }

    if out.is_empty() {
        "/".to_string()
    } else {
        out
    }
}

#[inline]
pub fn normalize_upstream_for_top(upstream: &str) -> String {
    let s = upstream.trim();
    if s.is_empty() {
        return "(empty)".to_string();
    }
    let s = s
        .strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))
        .unwrap_or(s);
    let s = s.strip_prefix("www.").unwrap_or(s);
    let host = s.split('/').next().unwrap_or(s);
    host.to_string()
}
