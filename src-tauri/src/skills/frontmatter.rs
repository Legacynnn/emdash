//! Tiny YAML-frontmatter parser for SKILL.md files. We only support
//! string scalar values (with optional single/double quoting); anything
//! more elaborate is rejected silently in line with the original
//! Electron parser at `src/shared/skills/validation.ts`.

use crate::skills::model::SkillFrontmatter;

pub fn parse(content: &str) -> (SkillFrontmatter, String) {
    let trimmed = content.trim_start_matches('\u{feff}');
    let Some(rest) = trimmed.strip_prefix("---\n").or_else(|| trimmed.strip_prefix("---\r\n"))
    else {
        return (
            SkillFrontmatter {
                name: String::new(),
                description: String::new(),
                ..SkillFrontmatter::default()
            },
            content.to_string(),
        );
    };

    // Find the closing fence.
    let close_idx = match find_close_fence(rest) {
        Some(idx) => idx,
        None => {
            return (
                SkillFrontmatter::default(),
                content.to_string(),
            )
        }
    };
    let yaml_block = &rest[..close_idx];
    let body_start = close_idx + close_fence_len(rest, close_idx);
    let body = rest.get(body_start..).unwrap_or("").to_string();

    let mut fm = SkillFrontmatter::default();
    for line in yaml_block.split('\n') {
        let Some(colon) = line.find(':') else {
            continue;
        };
        let key = line[..colon].trim();
        let mut value = line[colon + 1..].trim().to_string();
        let was_double = value.starts_with('"') && value.ends_with('"') && value.len() >= 2;
        let was_single = value.starts_with('\'') && value.ends_with('\'') && value.len() >= 2;
        if was_double {
            value = value[1..value.len() - 1]
                .replace("\\\"", "\"")
                .replace("\\\\", "\\");
        } else if was_single {
            value = value[1..value.len() - 1].replace("''", "'");
        }
        if key.is_empty() {
            continue;
        }
        match key {
            "name" => fm.name = value,
            "description" => fm.description = value,
            "license" => fm.license = Some(value),
            "compatibility" => fm.compatibility = Some(value),
            "allowed-tools" => fm.allowed_tools = Some(value),
            _ => {}
        }
    }
    (fm, body)
}

fn find_close_fence(rest: &str) -> Option<usize> {
    let mut start = 0;
    while let Some(idx) = rest[start..].find("---") {
        let abs = start + idx;
        let prev_nl = if abs == 0 {
            true
        } else {
            rest[..abs].ends_with('\n')
        };
        let after = &rest[abs + 3..];
        let next_ok = after.is_empty()
            || after.starts_with('\n')
            || after.starts_with("\r\n");
        if prev_nl && next_ok {
            return Some(abs);
        }
        start = abs + 3;
    }
    None
}

fn close_fence_len(rest: &str, close_idx: usize) -> usize {
    let tail = &rest[close_idx..];
    if tail.starts_with("---\r\n") {
        5
    } else if tail.starts_with("---\n") {
        4
    } else {
        3
    }
}

pub fn generate(name: &str, description: &str, body: Option<&str>) -> String {
    let body = body
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("# {name}\n\n{description}\n"));
    format!(
        "---\nname: \"{}\"\ndescription: \"{}\"\n---\n\n{body}\n",
        escape_yaml(name),
        escape_yaml(description)
    )
}

pub fn is_valid_skill_name(name: &str) -> bool {
    let len = name.chars().count();
    if !(1..=64).contains(&len) {
        return false;
    }
    if name.contains("--") {
        return false;
    }
    let mut chars = name.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return false;
    }
    let mut last = first;
    for c in chars {
        if !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
            return false;
        }
        last = c;
    }
    if last == '-' {
        return false;
    }
    true
}

fn escape_yaml(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_frontmatter() {
        let (fm, body) = parse("---\nname: foo\ndescription: bar\n---\nbody text\n");
        assert_eq!(fm.name, "foo");
        assert_eq!(fm.description, "bar");
        assert_eq!(body, "body text\n");
    }

    #[test]
    fn parses_quoted_values() {
        let (fm, _) = parse("---\nname: \"quoted name\"\ndescription: 'single'\n---\n");
        assert_eq!(fm.name, "quoted name");
        assert_eq!(fm.description, "single");
    }

    #[test]
    fn returns_empty_frontmatter_without_fence() {
        let (fm, body) = parse("just body");
        assert_eq!(fm.name, "");
        assert_eq!(body, "just body");
    }

    #[test]
    fn generate_round_trip() {
        let out = generate("my-skill", "Does things", None);
        let (fm, body) = parse(&out);
        assert_eq!(fm.name, "my-skill");
        assert_eq!(fm.description, "Does things");
        assert!(body.contains("# my-skill"));
    }

    #[test]
    fn validates_skill_name() {
        assert!(is_valid_skill_name("my-skill"));
        assert!(is_valid_skill_name("a"));
        assert!(is_valid_skill_name("abc123"));
        assert!(!is_valid_skill_name("My-Skill"));
        assert!(!is_valid_skill_name("-skill"));
        assert!(!is_valid_skill_name("skill-"));
        assert!(!is_valid_skill_name("a--b"));
        assert!(!is_valid_skill_name(""));
    }
}
