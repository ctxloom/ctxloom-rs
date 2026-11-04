//! What a file holds once its claims are in it, and what it holds once they
//! are out. Both work on the document: a JSON file is read and written back
//! (digits and member order kept, whitespace not), a text file has its claims
//! appended and cut out.

use surface_ports::{Claim, Json, JsonOp, SurfaceError};

/// A file's bytes with `claims` in it, over `base` (what the file held
/// before any of them). `left` collects values left standing.
pub(crate) fn render(
    file: &str,
    base: Option<&[u8]>,
    claims: &[&Claim],
    left: &mut Vec<String>,
) -> Result<Vec<u8>, SurfaceError> {
    let mut whole: Option<Vec<u8>> = None;
    let mut text: Option<String> = None;
    let mut json: Option<Json> = None;
    for claim in claims {
        match claim {
            Claim::Whole { bytes, .. } => whole = Some(bytes.clone()),
            Claim::Section { text: section, .. } => {
                let held = match text.take() {
                    Some(held) => held,
                    None => utf8(file, base.unwrap_or_default())?,
                };
                text = Some(append(&held, section));
            }
            Claim::Json { ops, .. } => {
                let mut doc = match json.take() {
                    Some(doc) => doc,
                    None => parse(file, base)?,
                };
                for op in ops {
                    apply(file, &mut doc, op, left)?;
                }
                json = Some(doc);
            }
        }
    }
    if let Some(bytes) = whole {
        return Ok(bytes);
    }
    if let Some(doc) = json {
        return Ok(format!("{}\n", doc.pretty()).into_bytes());
    }
    Ok(text.unwrap_or_default().into_bytes())
}

/// `current` with `claims` taken out of it, where they are found: what is
/// done to a file somebody changed after ctxloom wrote it. None where the file
/// would be left with nothing in it that ctxloom did not make.
pub(crate) fn unrender(
    file: &str,
    current: &[u8],
    claims: &[&Claim],
) -> Result<Vec<u8>, SurfaceError> {
    let mut text: Option<String> = None;
    let mut json: Option<Json> = None;
    for claim in claims.iter().rev() {
        match claim {
            Claim::Whole { .. } => return Ok(current.to_vec()),
            Claim::Section { text: section, .. } => {
                let held = match text.take() {
                    Some(held) => held,
                    None => utf8(file, current)?,
                };
                text = Some(cut(&held, section));
            }
            Claim::Json { ops, .. } => {
                let mut doc = match json.take() {
                    Some(doc) => doc,
                    None => parse(file, Some(current))?,
                };
                for op in ops.iter().rev() {
                    undo(&mut doc, op);
                }
                json = Some(doc);
            }
        }
    }
    if let Some(doc) = json {
        return Ok(format!("{}\n", doc.pretty()).into_bytes());
    }
    Ok(text.unwrap_or_default().into_bytes())
}

fn utf8(file: &str, bytes: &[u8]) -> Result<String, SurfaceError> {
    String::from_utf8(bytes.to_vec()).map_err(|_| SurfaceError::Refused {
        file: name(file),
        reason: "it is not text".to_string(),
    })
}

fn name(file: &str) -> String {
    file.rsplit('/').next().unwrap_or(file).to_string()
}

fn parse(file: &str, base: Option<&[u8]>) -> Result<Json, SurfaceError> {
    let Some(bytes) = base.filter(|b| !b.iter().all(u8::is_ascii_whitespace)) else {
        return Ok(Json::Object(Vec::new()));
    };
    let doc = Json::parse(&utf8(file, bytes)?).map_err(|e| SurfaceError::Refused {
        file: name(file),
        reason: format!(
            "ctxloom cannot read it ({e}), so it cannot tell its own entries from yours"
        ),
    })?;
    match doc {
        Json::Object(_) => Ok(doc),
        _ => Err(SurfaceError::Refused {
            file: name(file),
            reason: "it is not a JSON object".to_string(),
        }),
    }
}

/// `section` after `held`, a blank line between, ending in a newline.
fn append(held: &str, section: &str) -> String {
    let mut out = held.to_string();
    if !out.is_empty() {
        out.push_str(if out.ends_with('\n') { "\n" } else { "\n\n" });
    }
    out.push_str(section);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// `held` without the section [`append`] put in it (and the blank line it
/// put before it).
fn cut(held: &str, section: &str) -> String {
    let mut wanted = section.to_string();
    if !wanted.ends_with('\n') {
        wanted.push('\n');
    }
    let Some(at) = held.rfind(&wanted) else {
        return held.to_string();
    };
    let before = &held[..at];
    let after = &held[at + wanted.len()..];
    let before = before
        .strip_suffix("\n\n")
        .map_or_else(|| before.strip_suffix('\n').unwrap_or(before), |b| b);
    // A blank line and then the user's own text again: keep the newline the
    // user's text ended with.
    if before.is_empty() {
        after.to_string()
    } else if after.is_empty() {
        before.to_string()
    } else {
        format!("{before}\n{after}")
    }
}

fn walk<'a>(doc: &'a mut Json, path: &[String], create: bool) -> Option<&'a mut Json> {
    let mut here = doc;
    for key in path {
        if here.get(key).is_none() {
            if !create {
                return None;
            }
            if !here.set(key, Json::Object(Vec::new())) {
                return None;
            }
        }
        here = here.get_mut(key)?;
    }
    Some(here)
}

fn apply(
    file: &str,
    doc: &mut Json,
    op: &JsonOp,
    left: &mut Vec<String>,
) -> Result<(), SurfaceError> {
    let blocked = |path: &[String]| SurfaceError::Refused {
        file: name(file),
        reason: format!(
            "{} is not an object, so ctxloom cannot put its entry there",
            path.join(".")
        ),
    };
    match op {
        JsonOp::Set { path, value } => {
            let (last, parents) = path.split_last().ok_or_else(|| blocked(path))?;
            let parent = walk(doc, parents, true).ok_or_else(|| blocked(parents))?;
            if !parent.set(last, value.clone()) {
                return Err(blocked(parents));
            }
        }
        JsonOp::SetCommand { path, value } => {
            let (last, parents) = path.split_last().ok_or_else(|| blocked(path))?;
            let parent = walk(doc, parents, true).ok_or_else(|| blocked(parents))?;
            let ours = value.get("command").and_then(Json::as_str);
            match parent.get(last) {
                None => {
                    parent.set(last, value.clone());
                }
                Some(held) => match held.get("command") {
                    Some(Json::String(command)) if Some(command.as_str()) == ours => {
                        parent.set(last, value.clone());
                    }
                    Some(Json::String(command)) => left.push(format!(
                        "{}: {last} is yours ({command}); remove it to get ctxloom's",
                        name(file)
                    )),
                    _ => {
                        return Err(SurfaceError::Refused {
                            file: name(file),
                            reason: format!(
                                "its {last} has a command ctxloom cannot read, and replacing it would lose it"
                            ),
                        })
                    }
                },
            }
        }
        JsonOp::Push { path, value } => {
            let (last, parents) = path.split_last().ok_or_else(|| blocked(path))?;
            let parent = walk(doc, parents, true).ok_or_else(|| blocked(parents))?;
            if parent.get(last).is_none() {
                parent.set(last, Json::Array(Vec::new()));
            }
            match parent.get_mut(last) {
                Some(Json::Array(items)) => {
                    if !items.contains(value) {
                        items.push(value.clone());
                    }
                }
                _ => return Err(blocked(path)),
            }
        }
        JsonOp::HookEntry {
            event,
            matcher,
            value,
        } => {
            let hooks = walk(doc, &["hooks".to_string()], true)
                .ok_or_else(|| blocked(&["hooks".to_string()]))?;
            if hooks.get(event).is_none() {
                hooks.set(event, Json::Array(Vec::new()));
            }
            let Some(Json::Array(groups)) = hooks.get_mut(event) else {
                return Err(blocked(&["hooks".to_string(), event.clone()]));
            };
            let has =
                |group: &Json| group.get("matcher").and_then(Json::as_str) == matcher.as_deref();
            let at = groups.iter().position(has);
            match at {
                Some(at) => {
                    let group = &mut groups[at];
                    if group.get("hooks").is_none() {
                        group.set("hooks", Json::Array(Vec::new()));
                    }
                    match group.get_mut("hooks") {
                        Some(Json::Array(entries)) => {
                            if !entries.contains(value) {
                                entries.push(value.clone());
                            }
                        }
                        _ => return Err(blocked(&["hooks".to_string(), event.clone()])),
                    }
                }
                None => {
                    let mut members = Vec::new();
                    if let Some(matcher) = matcher {
                        members.push(("matcher", Json::text(matcher.as_str())));
                    }
                    members.push(("hooks", Json::Array(vec![value.clone()])));
                    groups.push(Json::object(members));
                }
            }
        }
    }
    Ok(())
}

/// Take `op` back out of `doc`, where it is found; containers it leaves empty
/// go with it.
fn undo(doc: &mut Json, op: &JsonOp) {
    match op {
        JsonOp::Set { path, value } | JsonOp::SetCommand { path, value } => {
            let Some((last, parents)) = path.split_last() else {
                return;
            };
            if let Some(parent) = walk(doc, parents, false) {
                if parent.get(last) == Some(value) {
                    parent.remove(last);
                }
            }
            prune(doc, parents);
        }
        JsonOp::Push { path, value } => {
            let Some((last, parents)) = path.split_last() else {
                return;
            };
            if let Some(parent) = walk(doc, parents, false) {
                if let Some(Json::Array(items)) = parent.get_mut(last) {
                    if let Some(at) = items.iter().position(|i| i == value) {
                        items.remove(at);
                    }
                    if items.is_empty() {
                        parent.remove(last);
                    }
                }
            }
            prune(doc, parents);
        }
        JsonOp::HookEntry {
            event,
            matcher,
            value,
        } => {
            if let Some(hooks) = doc.get_mut("hooks") {
                if let Some(Json::Array(groups)) = hooks.get_mut(event) {
                    take_hook_out(groups, matcher.as_deref(), value);
                    groups.retain(|group| {
                        let empty = matches!(group.get("hooks"), Some(Json::Array(e)) if e.is_empty());
                        let bare = matches!(group, Json::Object(m) if m.iter().all(|(k, _)| k == "hooks" || k == "matcher"));
                        !(empty && bare)
                    });
                    if groups.is_empty() {
                        hooks.remove(event);
                    }
                }
            }
            prune(doc, &["hooks".to_string()]);
        }
    }
}

/// The first entry equal to `value` in a group whose matcher is this, taken
/// out of it.
fn take_hook_out(groups: &mut [Json], matcher: Option<&str>, value: &Json) {
    for group in groups.iter_mut() {
        if group.get("matcher").and_then(Json::as_str) != matcher {
            continue;
        }
        if let Some(Json::Array(entries)) = group.get_mut("hooks") {
            if let Some(at) = entries.iter().position(|e| e == value) {
                entries.remove(at);
                return;
            }
        }
    }
}

/// Remove the objects along `path` that hold nothing, deepest first.
fn prune(doc: &mut Json, path: &[String]) {
    for depth in (1..=path.len()).rev() {
        let (last, parents) = path[..depth].split_last().expect("a non-empty path");
        let Some(parent) = walk(doc, parents, false) else {
            continue;
        };
        if matches!(parent.get(last), Some(Json::Object(m)) if m.is_empty()) {
            parent.remove(last);
        }
    }
}

#[cfg(test)]
#[path = "render.test.rs"]
mod tests;
