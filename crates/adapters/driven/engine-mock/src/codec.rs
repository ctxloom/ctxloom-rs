//! The mock's hook wire: ctxloom's own (Go's `kit.NativeHookCodec`). The
//! payload is `{event, session, transcript, source, prompt, tool,
//! tool_input}`; the answer is `{context, notice, block, reason}`.

use surface_ports::{HookCodec, HookEvent, HookResponse, Json, SurfaceError};

/// The mock's hook wire.
pub struct Codec;

const NAME: &str = "mock";

fn text(doc: &Json, key: &str) -> String {
    doc.get(key)
        .and_then(Json::as_str)
        .unwrap_or_default()
        .to_string()
}

impl HookCodec for Codec {
    fn decode(&self, event: &str, payload: &str) -> Result<HookEvent, SurfaceError> {
        let doc = Json::parse(payload).map_err(|detail| SurfaceError::Payload {
            engine: NAME.to_string(),
            detail,
        })?;
        let named = text(&doc, "event");
        Ok(HookEvent {
            event: if named.is_empty() {
                event.to_string()
            } else {
                named
            },
            native_session: text(&doc, "session"),
            transcript: text(&doc, "transcript"),
            source: text(&doc, "source"),
            prompt: text(&doc, "prompt"),
            tool: text(&doc, "tool"),
            skill: String::new(),
            path: String::new(),
        })
    }

    fn encode(&self, _event: &str, response: &HookResponse) -> Result<String, SurfaceError> {
        let mut members = Vec::new();
        if !response.context.is_empty() {
            members.push(("context", Json::text(response.context.as_str())));
        }
        if !response.notice.is_empty() {
            members.push(("notice", Json::text(response.notice.as_str())));
        }
        if response.block {
            members.push(("block", Json::Bool(true)));
        }
        if !response.reason.is_empty() {
            members.push(("reason", Json::text(response.reason.as_str())));
        }
        let parts: Vec<String> = members
            .iter()
            .map(|(k, v)| format!("{}:{}", Json::text(*k).pretty(), v.pretty()))
            .collect();
        Ok(format!("{{{}}}\n", parts.join(",")))
    }

    fn invoked_skills(&self, _transcript: &str) -> Vec<String> {
        Vec::new()
    }
}

#[cfg(test)]
#[path = "codec.test.rs"]
mod tests;
