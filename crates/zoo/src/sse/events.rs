use types::sse::SseEvent;

pub fn format_sse(event: &SseEvent) -> String {
    let json = serde_json::to_string(event).unwrap_or_default();
    format!("data: {json}\n\n")
}
