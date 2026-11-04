//! Skills as claude's Agent Skill packages, under the Agent Skills standard
//! with Anthropic's reserved words (Go's `kit.AgentSkillRules`).

use surface_ports::{Claim, Planned, Skill};

const DIR: &str = ".claude/skills";
const NAME_MAX: usize = 64;
const DESCRIPTION_MAX: usize = 1024;
const RESERVED: [&str; 2] = ["anthropic", "claude"];

/// The first constraint the skill's frontmatter breaks, as a sentence.
fn violation(skill: &Skill) -> Option<String> {
    let name = &skill.name;
    if name.is_empty() {
        return Some("SKILL.md frontmatter is missing the required `name` field".to_string());
    }
    if name.len() > NAME_MAX {
        return Some(format!(
            "name {name:?} is {} chars, exceeds the {NAME_MAX} char limit",
            name.len()
        ));
    }
    let word = |w: &str| {
        !w.is_empty()
            && w.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
    };
    if !name.split('-').all(word) {
        return Some(format!(
            "name {name:?} must be lowercase alphanumerics and hyphens only"
        ));
    }
    if let Some(reserved) = RESERVED.iter().find(|r| name.contains(*r)) {
        return Some(format!(
            "name {name:?} must not contain the reserved word {reserved:?}"
        ));
    }
    if skill.description.is_empty() {
        return Some(
            "SKILL.md frontmatter is missing the required `description` field".to_string(),
        );
    }
    if skill.description.len() > DESCRIPTION_MAX {
        return Some(format!(
            "description is {} chars, exceeds the {DESCRIPTION_MAX} char limit",
            skill.description.len()
        ));
    }
    None
}

/// Whether a package file's path stays inside its package.
fn inside(path: &str) -> bool {
    ctxloom_ports::is_plain_relative(path) && !path.contains('\\')
}

pub(crate) fn plan(skills: &[Skill], planned: &mut Planned) {
    for skill in skills {
        if let Some(why) = violation(skill) {
            planned.skipped.push(format!(
                "refusing skill {:?} for claude-code: {why}",
                skill.name
            ));
            continue;
        }
        for file in skill.files.iter().filter(|f| inside(&f.path)) {
            planned.claims.push(Claim::Whole {
                path: format!("{DIR}/{}/{}", skill.name, file.path),
                bytes: file.bytes.clone(),
                executable: file.executable,
            });
        }
    }
}

#[cfg(test)]
#[path = "skills.test.rs"]
mod tests;
