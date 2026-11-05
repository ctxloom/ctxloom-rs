//! Ported from Go's `internal/shared/confload/confload_test.go` (860336c4e).
//! Go's `map[string]any` values keep types; this reader keeps text, so the
//! assertions are on the text and the shape (a comma separated value is a
//! list).

use super::*;

fn parse(text: &str) -> Value {
    if text.is_empty() {
        return Value::Map(vec![]);
    }
    angzarr_yaml::parse(text).unwrap_or_else(|e| panic!("{e}"))
}

fn scalar(v: &str) -> Value {
    Value::Scalar(v.to_string())
}

fn at<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(v, |here, key| here.get(key))
}

/// A fake product schema: the dotted paths it knows (`*` is any label).
fn known(paths: &'static [&'static str]) -> impl Fn(&[String]) -> bool {
    move |path: &[String]| {
        paths.iter().any(|p| {
            let parts: Vec<&str> = p.split('.').collect();
            parts.len() == path.len() && parts.iter().zip(path).all(|(a, b)| *a == "*" || a == b)
        })
    }
}

fn no_check(_: &[String], _: &Value) -> Result<(), String> {
    Ok(())
}

fn apply_with(
    base: &str,
    env: &[(&str, &str)],
    sets: &[&str],
    paths: &'static [&'static str],
) -> Applied {
    let environ: Vec<(String, String)> = env
        .iter()
        .map(|(k, v)| (format!("{ENV_PREFIX}{k}"), v.to_string()))
        .collect();
    let sets: Vec<String> = sets.iter().map(ToString::to_string).collect();
    let (overrides, problems) = read(&environ, &sets);
    assert!(problems.is_empty(), "{problems:?}");
    apply(&parse(base), &overrides, &known(paths), &no_check)
}

// confload_test.go: TestPrecedence_FullChain_HomeProjectEnvCLI (the overlay
// half: the env beats the files, --config-set beats the env)
#[test]
fn test_the_flag_beats_the_environment_which_beats_the_files() {
    let out = apply_with(
        "c: project\nd: project\nb: project\n",
        &[("C", "env"), ("D", "env")],
        &["d=cli"],
        &[],
    );
    assert_eq!(at(&out.value, "b"), Some(&scalar("project")));
    assert_eq!(at(&out.value, "c"), Some(&scalar("env")));
    assert_eq!(at(&out.value, "d"), Some(&scalar("cli")));
}

// TestEnvOverlay_ResolvesCaseInsensitivelyToExistingKey
#[test]
fn test_an_environment_name_resolves_case_insensitively_adopting_the_existing_casing() {
    let out = apply_with(
        "agents:\n  MyCoder:\n    engine: claude-code\n    Runtime: host\n",
        &[("AGENTS_MYCODER_RUNTIME", "container")],
        &[],
        &[],
    );
    assert_eq!(
        at(&out.value, "agents.MyCoder.Runtime"),
        Some(&scalar("container"))
    );
    assert_eq!(
        at(&out.value, "agents.MyCoder.engine"),
        Some(&scalar("claude-code"))
    );
    assert!(at(&out.value, "agents.mycoder").is_none());
}

// TestEnvOverlay_AmbiguousCaseIsError
#[test]
fn test_two_keys_that_differ_only_in_case_are_an_error_naming_both() {
    let out = apply_with(
        "agents:\n  MyCoder: {}\n  mycoder: {}\n",
        &[("AGENTS_MYCODER_RUNTIME", "container")],
        &[],
        &[],
    );
    assert_eq!(out.errors.len(), 1, "{:?}", out.errors);
    let said = &out.errors[0];
    assert!(
        said.contains("CTXLOOM_CONFIG_AGENTS_MYCODER_RUNTIME"),
        "{said}"
    );
    assert!(
        said.contains("MyCoder") && said.contains("mycoder"),
        "{said}"
    );
    assert!(at(&out.value, "agents.MyCoder.runtime").is_none());
    assert!(at(&out.value, "agents.mycoder.runtime").is_none());
}

// TestEnvOverlay_UnsetSchemaKeyCreatedWithoutWarning
#[test]
fn test_a_schema_key_that_is_unset_is_created_without_a_warning() {
    let out = apply_with("", &[("DEFAULT_AGENT", "mycoder")], &[], &["default_agent"]);
    assert_eq!(at(&out.value, "default_agent"), Some(&scalar("mycoder")));
    assert!(out.warnings.is_empty());
}

// TestEnvOverlay_UnknownKeyWarnsAndCreates: the fallback keeps the token's
// ORIGINAL (upper) case
#[test]
fn test_an_unknown_environment_key_warns_and_is_created_as_written() {
    let out = apply_with("", &[("BOGUS", "x")], &[], &["default_agent"]);
    assert_eq!(at(&out.value, "BOGUS"), Some(&scalar("x")));
    assert_eq!(out.warnings.len(), 1);
}

// TestEnvOverlay_BootstrapVarsExcluded
#[test]
fn test_a_bootstrap_variable_is_no_override() {
    let environ = vec![
        ("CTXLOOM_ROOT".to_string(), "/some/path".to_string()),
        (format!("{ENV_PREFIX}DEFAULT_AGENT"), "mycoder".to_string()),
    ];
    let (overrides, problems) = read(&environ, &[]);
    assert!(problems.is_empty());
    assert_eq!(overrides.env.len(), 1);
    assert_eq!(overrides.env[0].0, "DEFAULT_AGENT");
}

// TestEnvOverlay_CoercesBoolIntAndList and TestEnvOverlay_ZeroAndOneStayIntegers
#[test]
fn test_values_are_texts_and_a_comma_separated_one_is_a_list() {
    assert_eq!(coerce("false"), scalar("false"));
    assert_eq!(coerce("3"), scalar("3"));
    assert_eq!(coerce("0"), scalar("0"));
    assert_eq!(coerce("1"), scalar("1"));
    assert_eq!(
        coerce("a,b, c"),
        Value::Seq(vec![scalar("a"), scalar("b"), scalar("c")])
    );
}

// TestEnvOverlay_ExplicitFalseBeatsInheritedTrue
#[test]
fn test_an_explicit_false_beats_an_inherited_true() {
    let out = apply_with("enabled: true\n", &[("ENABLED", "false")], &[], &[]);
    assert_eq!(at(&out.value, "enabled"), Some(&scalar("false")));
}

// TestConfigSetFlag_OverridesConfigKey
#[test]
fn test_config_set_overrides_a_key_in_the_files() {
    let out = apply_with(
        "llm:\n  default: claude\n",
        &[],
        &["llm.default=gemini"],
        &["llm"],
    );
    assert_eq!(at(&out.value, "llm.default"), Some(&scalar("gemini")));
}

// TestConfigSetFlag_CreatesCaseSensitiveKeyAsTyped: a schema that recognizes
// agents.<label>.runtime lets --config-set mint a label in the case typed
#[test]
fn test_config_set_creates_a_key_in_the_case_it_was_typed() {
    let out = apply_with(
        "",
        &[],
        &["agents.MyCoder.runtime=container"],
        &["agents.*.runtime"],
    );
    assert_eq!(
        at(&out.value, "agents.MyCoder.runtime"),
        Some(&scalar("container"))
    );
    assert!(at(&out.value, "agents.mycoder").is_none());
}

// TestConfigSetFlag_ResolvesCaseInsensitivelyToExistingKey
#[test]
fn test_config_set_resolves_case_insensitively_to_an_existing_key() {
    let out = apply_with(
        "agents:\n  MyCoder:\n    engine: claude-code\n",
        &[],
        &["agents.mycoder.runtime=container"],
        &[],
    );
    assert_eq!(
        at(&out.value, "agents.MyCoder.runtime"),
        Some(&scalar("container"))
    );
    assert_eq!(
        at(&out.value, "agents.MyCoder.engine"),
        Some(&scalar("claude-code"))
    );
}

// TestConfigSetFlag_AmbiguousCaseIsError
#[test]
fn test_config_set_with_an_ambiguous_case_is_an_error_naming_the_flag_and_both() {
    let out = apply_with(
        "agents:\n  MyCoder: {}\n  mycoder: {}\n",
        &[],
        &["agents.mycoder.runtime=container"],
        &[],
    );
    assert_eq!(out.errors.len(), 1);
    assert!(out.errors[0].contains("agents.mycoder.runtime"));
    assert!(out.errors[0].contains("MyCoder") && out.errors[0].contains("mycoder"));
}

// TestConfigSetFlag_MalformedIsError
#[test]
fn test_a_malformed_config_set_is_reported_naming_it() {
    let sets = vec!["no-equals-sign-here".to_string(), "=empty-path".to_string()];
    let (overrides, problems) = read(&[], &sets);
    assert!(overrides.flags.is_empty());
    assert_eq!(problems.len(), 2, "{problems:?}");
    assert!(
        problems[0].contains("no-equals-sign-here")
            && problems[0].contains("expected <path>=<value>")
    );
    assert!(problems[1].contains("=empty-path") && problems[1].contains("empty path"));
}

// TestConfigSetFlag_CoercesBoolIntAndList
#[test]
fn test_config_set_values_coerce_like_the_environments() {
    let out = apply_with(
        "",
        &[],
        &["enabled=false", "count=3", "tags=a,b,c"],
        &["enabled", "count", "tags"],
    );
    assert_eq!(at(&out.value, "enabled"), Some(&scalar("false")));
    assert_eq!(at(&out.value, "count"), Some(&scalar("3")));
    assert_eq!(
        at(&out.value, "tags"),
        Some(&Value::Seq(vec![scalar("a"), scalar("b"), scalar("c")]))
    );
}

// TestConfigSetFlag_UnknownKeyWarnsAndCreates (and Go's wording)
#[test]
fn test_an_unknown_config_set_key_warns_and_is_applied() {
    let out = apply_with("", &[], &["bogus=x"], &["default_agent"]);
    assert_eq!(at(&out.value, "bogus"), Some(&scalar("x")));
    assert_eq!(
        out.warnings,
        ["--config-set bogus does not match any known config key (resolved as bogus); setting it anyway"]
    );
}

// TestResolvePath_SchemaMatchPrefersTheTypedCase: an env name's underscores
// are ambiguous; the schema decides (LLM_DEFAULTS_PRIMARY is llm.defaults.primary)
#[test]
fn test_the_schema_resolves_an_environment_names_underscores() {
    let out = apply_with(
        "",
        &[("LLM_DEFAULTS_PRIMARY", "claude-fast")],
        &[],
        &["llm.defaults.primary", "llm.defaults"],
    );
    assert_eq!(
        at(&out.value, "llm.defaults.primary"),
        Some(&scalar("claude-fast"))
    );
    assert!(out.warnings.is_empty(), "{:?}", out.warnings);
}

// overlay.go resolveRaw: a value the schema refuses at its path is an error
// naming the source (and, as in Go, still applied: the load reports it)
#[test]
fn test_a_value_the_schema_refuses_is_an_error_naming_its_source() {
    let environ = vec![(format!("{ENV_PREFIX}RUNTIME"), "laptop".to_string())];
    let (overrides, _) = read(&environ, &[]);
    let refuse = |path: &[String], _: &Value| -> Result<(), String> {
        if path == ["runtime"] {
            Err("value must be one of \"host\"".to_string())
        } else {
            Ok(())
        }
    };
    let out = apply(
        &parse("runtime: host\n"),
        &overrides,
        &known(&["runtime"]),
        &refuse,
    );
    assert_eq!(out.errors.len(), 1, "{:?}", out.errors);
    assert!(
        out.errors[0].contains("CTXLOOM_CONFIG_RUNTIME"),
        "{}",
        out.errors[0]
    );
    assert!(out.errors[0].contains("runtime"), "{}", out.errors[0]);
    assert_eq!(at(&out.value, "runtime"), Some(&scalar("laptop")));
}

fn toks(of: &[&str]) -> Vec<String> {
    of.iter().map(ToString::to_string).collect()
}

// confload.go: the ways to cut an env var's words into runs, most runs first
#[test]
fn test_partitions_cut_the_tokens_most_runs_first() {
    let t = |a: &[&[&str]]| -> Vec<Vec<String>> { a.iter().map(|g| toks(g)).collect() };
    assert_eq!(
        partitions(&toks(&["a", "b", "c"])),
        [
            t(&[&["a"], &["b"], &["c"]]),
            t(&[&["a"], &["b", "c"]]),
            t(&[&["a", "b"], &["c"]]),
            t(&[&["a", "b", "c"]]),
        ]
    );
    assert_eq!(partitions(&toks(&["a"])), [t(&[&["a"]])]);
    assert!(partitions(&[]).is_empty());
}

// confload.go: a name of more than sixteen words is not cut at all
#[test]
fn test_partitions_stop_at_sixteen_words() {
    let words = |n: usize| -> Vec<String> { (0..n).map(|i| i.to_string()).collect() };
    assert_eq!(partitions(&words(16)).len(), 1 << 15);
    assert!(partitions(&words(17)).is_empty());
}

// ---------------------------------------------------------------- characterization
//
// Pinned before the intake mechanics may move to the shared repository (A0):
// what `read` takes from the environment and the flags, in what order, and
// the exact words of what it refuses or warns of.

fn pairs(of: &[(&str, &str)]) -> Vec<(String, String)> {
    of.iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[test]
fn test_the_prefix_is_the_ctxloom_config_underscore_in_capitals() {
    assert_eq!(ENV_PREFIX, "CTXLOOM_CONFIG_");
}

#[test]
fn test_only_variables_that_begin_with_the_exact_prefix_are_overrides() {
    let (found, problems) = read(
        &pairs(&[
            ("ctxloom_config_LOWER", "x"),
            ("XCTXLOOM_CONFIG_PRE", "x"),
            ("CTXLOOM_CONFIG", "x"),
            ("CTXLOOM_CONFIGX", "x"),
            ("CTXLOOM_CONFIG_", "no suffix"),
            ("CTXLOOM_CONFIG_REAL", "x"),
            ("PATH", "/bin"),
        ]),
        &[],
    );
    assert!(problems.is_empty());
    let names: Vec<&str> = found.env.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["REAL"]);
}

#[test]
fn test_overrides_come_sorted_by_name_whatever_the_order_given() {
    let (found, _) = read(
        &pairs(&[
            ("CTXLOOM_CONFIG_B", "2"),
            ("CTXLOOM_CONFIG_A", "1"),
            ("CTXLOOM_CONFIG_C", "3"),
        ]),
        &["z=1".to_string(), "m=2".to_string(), "a=3".to_string()],
    );
    let env: Vec<&str> = found.env.iter().map(|(n, _)| n.as_str()).collect();
    let flags: Vec<&str> = found.flags.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!((env, flags), (vec!["A", "B", "C"], vec!["a", "m", "z"]));
}

#[test]
fn test_a_later_config_set_of_the_same_path_replaces_the_earlier() {
    let sets = vec!["k=1".to_string(), "other=x".to_string(), "k=2".to_string()];
    let (found, problems) = read(&[], &sets);
    assert!(problems.is_empty());
    assert_eq!(
        found.flags,
        [
            ("k".to_string(), scalar("2")),
            ("other".to_string(), scalar("x"))
        ]
    );
}

#[test]
fn test_a_config_set_splits_at_the_first_equals_only() {
    let (found, _) = read(&[], &["a.b=c=d".to_string(), "e=".to_string()]);
    assert_eq!(
        found.flags,
        [
            ("a.b".to_string(), scalar("c=d")),
            ("e".to_string(), scalar(""))
        ]
    );
}

#[test]
fn test_the_problems_of_bad_config_sets_are_worded_exactly_and_the_rest_are_read() {
    let sets = vec![
        "no-equals".to_string(),
        "=blank".to_string(),
        "good=1".to_string(),
        "".to_string(),
    ];
    let (found, problems) = read(&[], &sets);
    assert_eq!(
        problems,
        [
            "--config-set \"no-equals\": expected <path>=<value>",
            "--config-set \"=blank\": empty path before \"=\"",
            "--config-set \"\": expected <path>=<value>",
        ]
    );
    assert_eq!(found.flags, [("good".to_string(), scalar("1"))]);
}

#[test]
fn test_the_environment_alone_gives_no_problems_whatever_it_holds() {
    let (found, problems) = read(&pairs(&[("CTXLOOM_CONFIG_A", "=,=")]), &[]);
    assert!(problems.is_empty());
    assert_eq!(found.env.len(), 1);
}

#[test]
fn test_a_list_value_keeps_its_blank_items_and_trims_the_others() {
    assert_eq!(
        coerce("a,,b"),
        Value::Seq(vec![scalar("a"), scalar(""), scalar("b")])
    );
    assert_eq!(coerce(","), Value::Seq(vec![scalar(""), scalar("")]));
    assert_eq!(
        coerce(" a , b "),
        Value::Seq(vec![scalar("a"), scalar("b")])
    );
    // a lone value keeps its spaces, and "empty" is a text
    assert_eq!(coerce(" a "), scalar(" a "));
    assert_eq!(coerce(""), scalar(""));
}

#[test]
fn test_a_number_or_a_bool_with_a_comma_in_it_is_still_a_list_of_texts() {
    // the number and bool checks are on the whole text: "1,2" is neither
    assert_eq!(coerce("1,2"), Value::Seq(vec![scalar("1"), scalar("2")]));
    assert_eq!(
        coerce("true,false"),
        Value::Seq(vec![scalar("true"), scalar("false")])
    );
    assert_eq!(coerce("1.5"), scalar("1.5"));
    assert_eq!(coerce("-3"), scalar("-3"));
}

#[test]
fn test_an_override_that_names_nothing_known_is_warned_of_in_these_words() {
    let out = apply_with("", &[("NOPE_AT_ALL", "x")], &["Odd.Path=1"], &[]);
    assert_eq!(
        out.warnings,
        [
            "CTXLOOM_CONFIG_NOPE_AT_ALL does not match any known config key (resolved as NOPE.AT.ALL); setting it anyway",
            "--config-set Odd.Path does not match any known config key (resolved as Odd.Path); setting it anyway",
        ]
    );
    assert!(out.errors.is_empty());
}

#[test]
fn test_an_ambiguous_environment_name_is_refused_in_these_words() {
    let out = apply_with("Ab: 1\nAB: 2\n", &[("AB", "3")], &[], &[]);
    assert_eq!(
        out.errors,
        ["CTXLOOM_CONFIG_AB: ambiguous override -- config has both \"AB\" and \"Ab\"; cannot tell which was meant (env/flag names lose case information)"]
    );
    assert_eq!(at(&out.value, "Ab"), Some(&scalar("1")), "left as it was");
}

#[test]
fn test_a_refused_value_is_still_set_and_the_error_says_who_set_it() {
    let refuse = |_: &[String], _: &Value| -> Verdict { Err("not allowed".to_string()) };
    let (overrides, _) = read(&pairs(&[("CTXLOOM_CONFIG_K", "v")]), &["f=1".to_string()]);
    let out = apply(&parse(""), &overrides, &known(&["k", "f"]), &refuse);
    assert_eq!(
        out.errors,
        [
            "CTXLOOM_CONFIG_K: env set \"k\" to a value the schema refuses: not allowed",
            "--config-set f: --config-set set \"f\" to a value the schema refuses: not allowed",
        ]
    );
    assert_eq!(at(&out.value, "k"), Some(&scalar("v")));
    assert_eq!(at(&out.value, "f"), Some(&scalar("1")));
}

#[test]
fn test_a_base_that_is_no_mapping_is_replaced_by_one() {
    let (overrides, _) = read(&[], &["a=1".to_string()]);
    let out = apply(&Value::Null, &overrides, &known(&["a"]), &no_check);
    assert_eq!(out.value, parse("a: 1\n"));
}
