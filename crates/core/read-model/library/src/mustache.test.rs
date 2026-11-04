//! Ported from Go's `internal/core/composite/substitute_test.go` (860336c4e).

use super::*;

fn run(content: &str, vars: &[(&str, &str)]) -> (String, Vec<String>) {
    let vars: BTreeMap<String, String> = vars
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let mut warnings = Vec::new();
    let out = substitute(content, &vars, &mut |w| warnings.push(w));
    (out, warnings)
}

// TestSubstituteVariables_Basic
#[test]
fn test_basic() {
    let (out, _) = run(
        "Hello {{name}}, welcome to {{place}}!",
        &[("name", "World"), ("place", "ctxloom")],
    );
    assert_eq!(out, "Hello World, welcome to ctxloom!");
}

// TestSubstituteVariables_MissingVariable
#[test]
fn test_a_missing_variable_is_reported() {
    let (_, warnings) = run("Hello {{name}}!", &[]);
    assert!(!warnings.is_empty());
    assert!(warnings[0].contains("undefined variable"), "{warnings:?}");
}

// TestSubstituteVariables_EmptyContent and _NoVariables
#[test]
fn test_empty_content_and_content_without_variables() {
    assert_eq!(run("", &[("name", "test")]).0, "");
    assert_eq!(run("No variables here", &[]).0, "No variables here");
}

// TestSubstituteVariables_InvalidTemplate: an unclosed section is a parse
// error; the content is returned as written, with a warning.
#[test]
fn test_an_unparsable_template_is_served_as_written_and_reported() {
    let content = "{{#section}}content without closing";
    let (out, warnings) = run(content, &[]);
    assert_eq!(out, content);
    assert!(!warnings.is_empty());
    assert!(warnings[0].contains("failed to parse"), "{warnings:?}");
}

// TestSubstituteVariables_SectionTag
#[test]
fn test_a_section_names_an_undefined_variable() {
    let (_, warnings) = run("{{#show}}visible{{/show}}", &[]);
    assert!(warnings[0].contains("undefined variable"));
    assert!(warnings[0].contains("show"));
}

// TestSubstituteVariables_RawVariable: {{{name}}} is not escaped
#[test]
fn test_a_raw_variable_is_not_escaped() {
    let (out, _) = run("Raw: {{{name}}}", &[("name", "<b>bold</b>")]);
    assert!(out.contains("<b>bold</b>"), "{out}");
    let (out, _) = run("Raw: {{&name}}", &[("name", "<b>bold</b>")]);
    assert!(out.contains("<b>bold</b>"), "{out}");
}

// html/template's escaper, which cbroglie/mustache uses for {{name}}
#[test]
fn test_a_plain_variable_is_html_escaped() {
    let (out, _) = run("{{name}}", &[("name", "A&B <c> \"d\" 'e'")]);
    assert_eq!(out, "A&amp;B &lt;c&gt; &#34;d&#34; &#39;e&#39;");
}

// TestSubstituteVariables_NestedSectionWithVariables
#[test]
fn test_a_nested_undefined_section_is_reported_first() {
    let (_, warnings) = run("{{#outer}}Hello {{inner_name}}!{{/outer}}", &[]);
    assert!(!warnings.is_empty());
    assert!(warnings[0].contains("outer"), "{warnings:?}");
}

// TestSubstituteVariables_InvertedSection and _DeeplyNestedSections
#[test]
fn test_inverted_and_deeply_nested_sections_are_reported() {
    let (_, warnings) = run("{{^missing}}default content{{/missing}}", &[]);
    assert!(warnings[0].contains("missing"));
    let (_, warnings) = run(
        "{{#level1}}A{{#level2}}B{{deep_var}}{{/level2}}{{/level1}}",
        &[],
    );
    assert!(warnings[0].contains("level1"), "{warnings:?}");
}

// TestSubstituteVariables_UndefinedPlainVariableRendersVerbatim
#[test]
fn test_an_undefined_plain_variable_renders_verbatim_and_is_reported() {
    let content = "Run `just build`:\n```just\nbuild:\n    go build {{ARGS}}\n```\nSee {{TOP}} and {{justfile_directory()}}.";
    let (out, warnings) = run(content, &[]);
    assert_eq!(
        out, content,
        "rendered output must be byte-identical to the input when nothing resolves"
    );
    assert!(warnings.len() >= 3);
    let joined = warnings[..3].join("");
    for name in ["ARGS", "TOP", "justfile_directory()"] {
        assert!(joined.contains(name), "{name} in {joined}");
    }
}

// TestSubstituteVariables_SectionIdiomUnchangedByVerbatimFix
#[test]
fn test_the_section_presence_toggle_keeps_its_polarity() {
    let (out, _) = run(
        "{{#DEBUG}}debug on{{/DEBUG}}{{^DEBUG}}debug off{{/DEBUG}}",
        &[],
    );
    assert!(!out.contains("debug on"));
    assert!(out.contains("debug off"));
    assert!(
        !out.contains("{{DEBUG}}") && !out.contains("{{#DEBUG}}") && !out.contains("{{^DEBUG}}")
    );
}

// TestSubstituteVariables_NameUsedAsBothSectionAndPlainVariableStaysFalsy
#[test]
fn test_a_name_used_as_section_and_plain_variable_stays_falsy() {
    let (out, _) = run("{{#DEBUG}}on{{/DEBUG}} plain:{{DEBUG}}", &[]);
    assert!(!out.contains("on"));
    assert!(!out.contains("plain:{{DEBUG}}"));
    assert!(out.contains("plain:"));
}

// TestSubstituteVariables_SetDelimiterEscapesLiteralMustache
#[test]
fn test_set_delimiter_escapes_literal_mustache() {
    let content = "{{=<% %>=}}\nUse `{{ARGS}}` to forward recipe arguments; `{{justfile_directory()}}`\nis scoped to the local file, not a composing parent. See {{TOP}}.\n<%={{ }}=%>\nReal variable: {{name}}";
    let (out, warnings) = run(content, &[("name", "World")]);
    assert!(
        out.contains("Use `{{ARGS}}` to forward recipe arguments; `{{justfile_directory()}}`"),
        "{out}"
    );
    assert!(out.contains("is scoped to the local file, not a composing parent. See {{TOP}}."));
    assert!(out.contains("Real variable: World"));
    assert!(warnings.is_empty(), "{warnings:?}");
}

// TestSubstituteVariables_RealVariableOutsideEscapedBlockStillSubstitutes
#[test]
fn test_a_real_variable_outside_an_escaped_block_still_substitutes() {
    let content = "Before: {{name}}\n{{=<% %>=}}\nliteral {{name}} stays literal\n<%={{ }}=%>\nAfter: {{name}}";
    let (out, warnings) = run(content, &[("name", "World")]);
    assert!(out.contains("Before: World"));
    assert!(out.contains("literal {{name}} stays literal"));
    assert!(out.contains("After: World"));
    assert!(warnings.is_empty());
}

// TestSubstituteVariables_MultipleSetDelimiterBlocksInOneFragment
#[test]
fn test_the_escape_is_not_a_one_shot_toggle() {
    let content = "{{=<% %>=}}\nfirst literal {{ARGS}}\n<%={{ }}=%>\nmiddle real: {{name}}\n{{=<% %>=}}\nsecond literal {{TOP}}\n<%={{ }}=%>\nend";
    let (out, warnings) = run(content, &[("name", "World")]);
    assert!(out.contains("first literal {{ARGS}}"));
    assert!(out.contains("middle real: World"));
    assert!(out.contains("second literal {{TOP}}"));
    assert!(warnings.is_empty());
}

// TestSubstituteVariables_SetDelimiterEscapesEvenRealVariableNames
#[test]
fn test_the_escape_is_lexical_not_by_name() {
    let (out, warnings) = run(
        "{{=<% %>=}}\nliteral {{ARGS}}\n<%={{ }}=%>\nreal: {{ARGS}}",
        &[("ARGS", "real-value")],
    );
    assert!(out.contains("literal {{ARGS}}"));
    assert!(out.contains("real: real-value"));
    assert!(warnings.is_empty());
}

// TestSubstituteVariables_UnclosedSetDelimiterSwallowsRestOfFragment
#[test]
fn test_an_unclosed_set_delimiter_swallows_the_rest_silently() {
    let content =
        "Before: {{name}}\n{{=<% %>=}}\nliteral {{ARGS}}\nAfter (never re-escaped): {{name}}";
    let (out, warnings) = run(content, &[("name", "World")]);
    assert!(out.contains("Before: World"));
    assert!(out.contains("literal {{ARGS}}"));
    assert!(out.contains("After (never re-escaped): {{name}}"));
    assert!(!out.contains("After (never re-escaped): World"));
    assert!(warnings.is_empty());
}

// cbroglie/mustache parse.go (the mustache spec's standalone tags): a section,
// comment or delimiter tag alone on its line takes the whole line with it;
// a variable never does; a tag with text beside it keeps the padding.
#[test]
fn test_a_tag_alone_on_its_line_takes_the_line_with_it() {
    for (template, want) in [
        ("a\n{{#x}}\nin\n{{/x}}\nb\n", "a\nin\nb\n"),
        ("  {{#x}}\n  in\n  {{/x}}\n", "  in\n"),
        ("l1\n  \t{{#x}}\t \nq\n{{/x}}", "l1\nq\n"),
        ("{{#x}}\r\nin\r\n{{/x}}\r\n", "in\r\n"),
        ("{{#x}}a\n  {{/x}}\r\nz", "a\nz"),
        ("{{#x}}a\n  {{/x}}\rz", "a\n  \rz"),
        ("{{#x}}\ra{{/x}}", "\ra"),
        ("{{#x}}\n{{/x}}", ""),
        ("{{#x}}a\n{{/x}}  ", "a\n  "),
        ("a\n  {{! c }}\nb", "a\nb"),
        ("a\n  {{! c}}", "a\n"),
        ("{{=<% %>=}}\n<%x%> {{x}}\n", "1 {{x}}\n"),
        ("  {{x}}\n", "  1\n"),
        ("a {{#x}} b {{/x}}\n", "a  b \n"),
        ("{{! c}} z\n", " z\n"),
        ("  {{! c}} z\n", "   z\n"),
    ] {
        let (out, warnings) = run(template, &[("x", "1")]);
        assert_eq!(out, want, "{template:?}");
        assert!(warnings.is_empty(), "{template:?}: {warnings:?}");
    }
}

// cbroglie/mustache parse.go: a template that does not parse names the line
#[test]
fn test_a_parse_failure_names_its_line() {
    for (template, why) in [
        ("a\nb\n{{#x}}\n", "Line 4: Section 'x' has no closing tag"),
        ("a\nb\n{{#x}} q\n", "Line 3: Section 'x' has no closing tag"),
        ("a\n{{/x}}", "Line 2: unmatched close tag"),
        ("a\n{{#y}}\n{{/x}}", "Line 3: Interleaved closing tag: x"),
        ("a\n{{> p}}", "Line 2: partials are not supported: p"),
        (
            "line1\nline2\n{{> p}}",
            "Line 3: partials are not supported: p",
        ),
        ("x\n\n{{ab", "Line 3: unmatched open tag"),
        ("x\n{{ }}", "Line 2: empty tag"),
        ("{{a\nb}} {{", "Line 2: unmatched open tag"),
        ("{{#x}}\n{{y\n}}\n{{/x}}\n{{", "Line 5: unmatched open tag"),
        (
            "ab {{x}}\n\ncd {{! c\n}} \n{{",
            "Line 5: unmatched open tag",
        ),
        (
            "{{#x}}a\n  {{/x}}\n{{#y}}",
            "Line 3: Section 'y' has no closing tag",
        ),
    ] {
        let (out, warnings) = run(template, &[("x", "1")]);
        assert_eq!(out, template, "served as written");
        assert_eq!(
            warnings,
            [format!("failed to parse template: {why}")],
            "{template:?}"
        );
    }
}

// cbroglie/mustache: the raw spellings, a section over a string, an inverted
// section, and the delimiter forms that are not delimiters
#[test]
fn test_the_tag_spellings() {
    let x = [("x", "1")];
    assert_eq!(run("{{{x}}} {{&x}} {{x}}", &x).0, "1 1 1");
    assert_eq!(run("{{^x}}no{{/x}}{{^y}}yes{{/y}}", &x).0, "yes");
    assert_eq!(run("{{#x}}{{.}}{{/x}}", &x).0, "1");
    assert_eq!(run("{{= <% %> =}}x", &x).0, "x");
    for odd in ["{{=<% %>}}", "{{=a=}}"] {
        let (out, warnings) = run(odd, &x);
        assert_eq!(warnings, [format!("undefined variable: {odd}")], "{odd}");
        assert!(out.starts_with("{{=") || out.contains("&lt;"), "{out}");
    }
}

// cbroglie/mustache parse.go: a standalone tag ended by CRLF counts its line
#[test]
fn test_a_crlf_standalone_tag_counts_its_line_and_a_bare_equals_is_a_variable() {
    let (_, warnings) = run("{{! c}}\r\nq\r\n{{", &[]);
    assert_eq!(
        warnings,
        ["failed to parse template: Line 3: unmatched open tag"]
    );
    let (out, warnings) = run("{{=}} {{==}}", &[]);
    assert_eq!(out, "{{=}} {{==}}");
    assert_eq!(
        warnings,
        ["undefined variable: {{=}}", "undefined variable: {{==}}"]
    );
}
