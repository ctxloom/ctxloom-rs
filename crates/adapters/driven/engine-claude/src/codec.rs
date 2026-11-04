//! Claude's hook wire in the port's terms (Go's `hookcodec.go`): the JSON
//! claude writes to a hook's stdin, decoded, and the JSON it reads back from
//! stdout, encoded.

use surface_ports::{HookCodec, HookEvent, HookResponse, Json, SurfaceError};

use crate::NAME;

/// Claude's hook wire.
#[derive(Default)]
pub struct Codec;

/// The unified event for a native one; a name not listed is carried verbatim.
fn unified(native: &str) -> String {
    match native {
        "PreToolUse" => "pre_tool",
        "PostToolUse" => "post_tool",
        "SessionStart" => "session_start",
        "SessionEnd" => "session_end",
        "Stop" => "turn_end",
        "UserPromptSubmit" => "turn_start",
        "PermissionRequest" => "permission_ask",
        other => other,
    }
    .to_string()
}

/// The native event whose `hookSpecificOutput` carries context to the model,
/// by the unified events registered on them.
fn context_event(event: &str) -> Option<&'static str> {
    match event {
        "session_start" => Some("SessionStart"),
        "turn_start" => Some("UserPromptSubmit"),
        "post_tool" | "post_file_edit" => Some("PostToolUse"),
        _ => None,
    }
}

/// The events claude lets a hook block with a decision.
fn can_block(event: &str) -> bool {
    matches!(
        event,
        "turn_start" | "post_tool" | "post_file_edit" | "turn_end"
    )
}

fn text(doc: &Json, key: &str) -> String {
    doc.get(key)
        .and_then(Json::as_str)
        .unwrap_or_default()
        .to_string()
}

/// The skill a tool call ran: only claude's `Skill` tool counts, and its
/// input names the skill under `skill`.
fn skill_of(tool: &str, input: Option<&Json>) -> String {
    if tool != "Skill" {
        return String::new();
    }
    input
        .and_then(|i| i.get("skill"))
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
        let native = text(&doc, "hook_event_name");
        let tool = text(&doc, "tool_name");
        let input = doc.get("tool_input");
        let path = ["file_path", "notebook_path"]
            .iter()
            .find_map(|k| input.and_then(|i| i.get(k)).and_then(Json::as_str))
            .unwrap_or_default()
            .to_string();
        Ok(HookEvent {
            event: if native.is_empty() {
                event.to_string()
            } else {
                unified(&native)
            },
            native_session: text(&doc, "session_id"),
            transcript: text(&doc, "transcript_path"),
            source: text(&doc, "source"),
            prompt: text(&doc, "prompt"),
            skill: skill_of(&tool, input),
            tool,
            path,
        })
    }

    fn encode(&self, event: &str, response: &HookResponse) -> Result<String, SurfaceError> {
        let mut members: Vec<(&str, Json)> = Vec::new();
        if response.block {
            if !can_block(event) {
                return Err(SurfaceError::Unsupported {
                    engine: NAME.to_string(),
                    what: format!("it cannot block {event} from a hook"),
                });
            }
            members.push(("decision", Json::text("block")));
            if !response.reason.is_empty() {
                members.push(("reason", Json::text(response.reason.as_str())));
            }
        }
        if !response.context.is_empty() {
            let native = context_event(event).ok_or_else(|| SurfaceError::Unsupported {
                engine: NAME.to_string(),
                what: format!("it carries no hook context on {event}"),
            })?;
            members.push((
                "hookSpecificOutput",
                Json::object([
                    ("hookEventName", Json::text(native)),
                    ("additionalContext", Json::text(response.context.as_str())),
                ]),
            ));
        }
        if !response.notice.is_empty() {
            members.push(("systemMessage", Json::text(response.notice.as_str())));
        }
        // One line, as claude reads it.
        Ok(format!("{}\n", compact(&Json::object(members))))
    }

    fn invoked_skills(&self, transcript: &str) -> Vec<String> {
        let mut seen = Vec::new();
        for line in transcript.lines() {
            let Ok(entry) = Json::parse(line) else {
                continue;
            };
            let main_thread = !matches!(entry.get("isSidechain"), Some(Json::Bool(true)));
            if !main_thread || entry.get("type").and_then(Json::as_str) != Some("assistant") {
                continue;
            }
            let Some(content) = entry
                .get("message")
                .and_then(|m| m.get("content"))
                .and_then(Json::as_array)
            else {
                continue;
            };
            for part in content {
                if part.get("type").and_then(Json::as_str) != Some("tool_use") {
                    continue;
                }
                let skill = skill_of(&text(part, "name"), part.get("input"));
                if !skill.is_empty() && !seen.contains(&skill) {
                    seen.push(skill);
                }
            }
        }
        seen
    }
}

/// The document on one line.
fn compact(doc: &Json) -> String {
    match doc {
        Json::Array(items) => {
            let parts: Vec<String> = items.iter().map(compact).collect();
            format!("[{}]", parts.join(","))
        }
        Json::Object(members) => {
            let parts: Vec<String> = members
                .iter()
                .map(|(k, v)| format!("{}:{}", Json::text(k.as_str()).pretty(), compact(v)))
                .collect();
            format!("{{{}}}", parts.join(","))
        }
        other => other.pretty(),
    }
}

#[cfg(test)]
#[path = "codec.test.rs"]
mod tests;
