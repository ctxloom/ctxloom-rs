//! Priority, as Go's `priority.Compute` derives it: the formula bridge, a
//! task's tag values, the per-task scores, rank normalization and the
//! diagnostics. The formulas here are written for each test; what the
//! shipped default formulas rank is the application's tests', which read
//! the default schema itself.

use std::collections::BTreeMap;

use annals::next::Timestamp;
use annals_diagnostic::{Arg, Diagnostic};
use tasks::{Tag, DEFERRED, TO_DO};

use super::{
    placeholders, rank, rank_normalize, tag_values, Candidate, Facet, Formula, Formulas,
    PriorityError, Ranking, TargetCoverage, AGE_DAYS, AGE_FACTOR, EXPLOITED_IN_WILD, MAX,
};

/// 2026-07-20T12:00:00Z: every age here is measured from it.
const NOW: Timestamp = Timestamp {
    seconds: 1_784_548_800,
    nanos: 0,
};

/// The instant `days` before [`NOW`] (after it, for a negative `days`).
fn ago(days: i64) -> Timestamp {
    Timestamp {
        seconds: NOW.seconds - days * 86_400,
        nanos: 0,
    }
}

/// Each of `spelled`, parsed as tagma parses a tag.
fn tags(spelled: &[&str]) -> Vec<Tag> {
    spelled
        .iter()
        .map(|s| tagma_core::Tag::parse(s).expect("a valid tag").into())
        .collect()
}

/// A task to rank: its harp id, status, tags and age in days. Done and
/// Archived are the terminal statuses here, as the application's are.
struct Task {
    harp_id: &'static str,
    status: &'static str,
    tags: Vec<Tag>,
    created_at: Timestamp,
}

fn task(harp_id: &'static str, status: &'static str, spelled: &[&str], days: i64) -> Task {
    Task {
        harp_id,
        status,
        tags: tags(spelled),
        created_at: ago(days),
    }
}

fn candidates(all: &[Task]) -> Vec<Candidate<'_>> {
    all.iter()
        .map(|t| Candidate {
            harp_id: t.harp_id,
            status: t.status,
            terminal: t.status == "Done" || t.status == "Archived",
            tags: &t.tags,
            created_at: t.created_at,
        })
        .collect()
}

/// `formulas` by facet: each `(facet, target, formula)` declared.
fn formulas(declared: &[(Facet, &str, &str)]) -> Result<Formulas, PriorityError> {
    let mut priority_fns = BTreeMap::new();
    let mut decay_fns = BTreeMap::new();
    for (facet, target, formula) in declared {
        let fns = match facet {
            Facet::PriorityFn => &mut priority_fns,
            Facet::DecayFn => &mut decay_fns,
        };
        fns.insert(target.to_string(), formula.to_string());
    }
    Formulas::compile(&priority_fns, &decay_fns)
}

/// `all` ranked by the formulas `declared`, which must compile.
fn ranked(all: &[Task], declared: &[(Facet, &str, &str)]) -> Result<Ranking, PriorityError> {
    rank(
        &candidates(all),
        &formulas(declared).expect("formulas compile"),
        NOW,
    )
}

/// The score of the task `harp_id` in `ranking` of `all`.
fn raw(all: &[Task], ranking: &Ranking, harp_id: &str) -> f64 {
    score(all, ranking, harp_id).raw
}

fn score<'r>(all: &[Task], ranking: &'r Ranking, harp_id: &str) -> &'r super::Score {
    let at = all
        .iter()
        .position(|t| t.harp_id == harp_id)
        .expect("a task of the test's");
    &ranking.scores[at]
}

use Facet::{DecayFn, PriorityFn};

// ------------------------------------------------------------ usage

#[test]
fn test_rank_scores_each_task_by_priority_fn_over_its_tags() {
    let all = [
        task("high", TO_DO, &["widget:score=3"], 0),
        task("low", TO_DO, &["widget:score=1"], 0),
        task("mid", TO_DO, &["widget:score=2"], 0),
    ];
    let ranking = ranked(
        &all,
        &[(PriorityFn, "widget:score", "{{widget:score}} * 2")],
    )
    .unwrap();
    assert_eq!(raw(&all, &ranking, "high"), 6.0);
    assert_eq!(raw(&all, &ranking, "low"), 2.0);
    // Rank-normalized onto [0, MAX] by percentile within the population.
    assert_eq!(score(&all, &ranking, "high").priority, MAX);
    assert_eq!(score(&all, &ranking, "mid").priority, MAX * 2.0 / 3.0);
    assert_eq!(score(&all, &ranking, "low").priority, MAX / 3.0);
}

#[test]
fn test_decay_fn_gives_priority_fn_its_age_factor() {
    let all = [
        task("fresh", TO_DO, &[], 0),
        task("year-old", TO_DO, &[], 365),
    ];
    let ranking = ranked(
        &all,
        &[
            (PriorityFn, "widget:score", "{{age_factor}}"),
            (DecayFn, "widget:score", "1 + {{age_days}} / 365"),
        ],
    )
    .unwrap();
    assert!((raw(&all, &ranking, "fresh") - 1.0).abs() < 1e-9);
    assert!((raw(&all, &ranking, "year-old") - 2.0).abs() < 1e-9);
}

#[test]
fn test_age_counts_fractions_of_a_second() {
    // Created a day and a half-second before NOW: 86 399.5 seconds old.
    let mut all = [task("a", TO_DO, &[], 1)];
    all[0].created_at.nanos = 500_000_000;
    let ranking = ranked(
        &all,
        &[(PriorityFn, "widget:score", "{{age_days}} * 86400")],
    )
    .unwrap();
    assert!((raw(&all, &ranking, "a") - 86_399.5).abs() < 1e-6);
}

#[test]
fn test_priority_fn_reads_age_days_too() {
    let all = [task("a", TO_DO, &[], 10)];
    let ranking = ranked(&all, &[(PriorityFn, "widget:score", "{{age_days}} + 1")]).unwrap();
    assert!((raw(&all, &ranking, "a") - 11.0).abs() < 1e-9);
}

#[test]
fn test_a_whole_number_result_is_a_score() {
    let all = [task("a", TO_DO, &[], 0)];
    let ranking = ranked(&all, &[(PriorityFn, "widget:score", "2 ** 3")]).unwrap();
    assert_eq!(raw(&all, &ranking, "a"), 8.0);
}

#[test]
fn test_exploited_in_wild_forces_priority_to_max_but_not_raw() {
    let all = [
        task("boring", TO_DO, &["widget:score=2"], 0),
        task(
            "exploited",
            TO_DO,
            &["widget:score=1", EXPLOITED_IN_WILD],
            0,
        ),
    ];
    let ranking = ranked(&all, &[(PriorityFn, "widget:score", "{{widget:score}}")]).unwrap();
    let exploited = score(&all, &ranking, "exploited");
    assert_eq!(
        exploited.raw, 1.0,
        "the override is display priority's only"
    );
    assert_eq!(exploited.priority, MAX);
    assert!(exploited.overridden);
    assert!(!score(&all, &ranking, "boring").overridden);
}

#[test]
fn test_a_deferred_task_skips_decay() {
    let all = [
        task("parked", DEFERRED, &[], 100),
        task("active", TO_DO, &[], 100),
    ];
    let ranking = ranked(
        &all,
        &[
            (PriorityFn, "widget:score", "2 * {{age_factor}}"),
            (
                DecayFn,
                "widget:score",
                "1 + {{age_days}}/({{age_days}}+90)",
            ),
        ],
    )
    .unwrap();
    assert_eq!(raw(&all, &ranking, "parked"), 2.0, "age_factor pinned to 1");
    assert!((raw(&all, &ranking, "active") - 2.0 * (1.0 + 100.0 / 190.0)).abs() < 1e-9);
}

#[test]
fn test_terminal_tasks_stay_out_of_the_normalized_population() {
    let all = [
        task("finished", "Done", &["widget:score=9"], 0),
        task("shelved", "Archived", &["widget:score=9"], 0),
        task("only-live", TO_DO, &["widget:score=1"], 0),
    ];
    let ranking = ranked(&all, &[(PriorityFn, "widget:score", "{{widget:score}}")]).unwrap();
    assert_eq!(
        raw(&all, &ranking, "finished"),
        9.0,
        "raw is computed for every task"
    );
    assert_eq!(score(&all, &ranking, "finished").priority, 0.0);
    assert_eq!(score(&all, &ranking, "shelved").priority, 0.0);
    assert_eq!(score(&all, &ranking, "only-live").priority, MAX);
}

#[test]
fn test_exploited_in_wild_does_not_rank_a_terminal_task() {
    let all = [
        task("closed", "Done", &[EXPLOITED_IN_WILD], 0),
        task("open", TO_DO, &[], 0),
    ];
    let ranking = ranked(&all, &[(PriorityFn, "widget:score", "1")]).unwrap();
    let closed = score(&all, &ranking, "closed");
    assert_eq!(closed.priority, 0.0);
    assert!(!closed.overridden);
}

#[test]
fn test_scores_come_back_in_the_candidates_order() {
    let all = [
        task("b", TO_DO, &["widget:score=2"], 0),
        task("a", TO_DO, &["widget:score=1"], 0),
    ];
    let ranking = ranked(&all, &[(PriorityFn, "widget:score", "{{widget:score}}")]).unwrap();
    let raws: Vec<f64> = ranking.scores.iter().map(|s| s.raw).collect();
    assert_eq!(raws, [2.0, 1.0]);
}

// ------------------------------------------------------------ the bridge

#[test]
fn test_placeholders_are_found_as_gos_pattern_finds_them() {
    assert_eq!(
        placeholders("{{a}} + {{ b c }} {{}} {{{d}} {{e}x}} {{f"),
        ["a", "b c", "d"]
    );
    assert!(placeholders("none").is_empty());
}

#[test]
fn test_an_absent_tag_reads_as_zero() {
    let all = [task("a", TO_DO, &[], 0)];
    let ranking = ranked(
        &all,
        &[(PriorityFn, "widget:score", "{{triage:missing}} + 5")],
    )
    .unwrap();
    assert_eq!(raw(&all, &ranking, "a"), 5.0);
}

#[test]
fn test_precedence_and_operators_are_exprs() {
    let all = [task(
        "a",
        TO_DO,
        &["widget:a=2", "widget:b=3", "widget:c=4"],
        0,
    )];
    let cases = [
        ("({{widget:a}} + {{widget:b}}) * {{widget:c}}", 20.0),
        ("{{widget:a}} + {{widget:b}} * {{widget:c}}", 14.0),
        ("{{widget:b}} / {{widget:a}}", 1.5),
        ("2 ** 3 ** 2", 512.0),
        ("-2 ** 2", -4.0),
        ("{{widget:a}} > 1 ? {{widget:b}} : {{widget:c}}", 3.0),
        ("{{widget:a}} > 5 ? 1 : {{widget:a}} > 1 ? 2 : 3", 2.0),
        ("max({{widget:a}}, {{widget:c}})", 4.0),
    ];
    for (formula, want) in cases {
        let ranking = ranked(&all, &[(PriorityFn, "widget:score", formula)]).unwrap();
        assert_eq!(raw(&all, &ranking, "a"), want, "{formula}");
    }
}

#[test]
fn test_a_tag_value_is_data_never_formula_text() {
    // A quoted value can hold anything; arithmetic in it is never run.
    let all = [task("a", TO_DO, &[r#"triage:impact="9999*9999""#], 0)];
    let ranking = ranked(
        &all,
        &[(PriorityFn, "widget:score", "{{triage:impact}} * 2")],
    )
    .unwrap();
    assert_eq!(raw(&all, &ranking, "a"), 0.0);
}

#[test]
fn test_one_compiled_formula_scores_every_task_independently() {
    let all = [
        task("one", TO_DO, &["triage:impact=1"], 0),
        task("many", TO_DO, &["triage:impact=99"], 0),
        task("one-again", TO_DO, &["triage:impact=1"], 0),
    ];
    let ranking = ranked(&all, &[(PriorityFn, "widget:score", "{{triage:impact}}")]).unwrap();
    assert_eq!(raw(&all, &ranking, "one"), 1.0);
    assert_eq!(raw(&all, &ranking, "many"), 99.0);
    assert_eq!(raw(&all, &ranking, "one-again"), 1.0);
}

#[test]
fn test_a_formula_keeps_its_source() {
    let formula = Formula::compile(PriorityFn, "widget:score", "{{widget:score}} * 2").unwrap();
    assert_eq!(formula.source(), "{{widget:score}} * 2");
}

// ------------------------------------------------------------ tag values

#[test]
fn test_tag_values_a_modifier_is_one() {
    let values = tag_values(&tags(&["triage:regression"]));
    assert_eq!(values.get("triage:regression"), Some(&1.0));
    assert_eq!(values.get("triage:regression=*"), Some(&1.0));
}

#[test]
fn test_tag_values_a_number_is_its_value_and_its_composite_key() {
    let values = tag_values(&tags(&["triage:severity=3.5"]));
    assert_eq!(values.get("triage:severity"), Some(&3.5));
    assert_eq!(values.get("triage:severity=3.5"), Some(&1.0));
    assert_eq!(values.get("triage:severity=*"), Some(&1.0));
}

#[test]
fn test_tag_values_a_word_is_zero_with_its_composite_key() {
    let values = tag_values(&tags(&["triage:impact=capability"]));
    assert_eq!(values.get("triage:impact"), Some(&0.0));
    assert_eq!(values.get("triage:impact=capability"), Some(&1.0));
    assert_eq!(values.get("triage:impact=tooling"), None);
}

#[test]
fn test_tag_values_a_non_finite_number_is_zero() {
    for value in ["NaN", "Inf", "-Inf"] {
        let values = tag_values(&tags(&[&format!("triage:impact={value}")]));
        assert_eq!(values.get("triage:impact"), Some(&0.0), "{value}");
    }
}

#[test]
fn test_tag_values_an_absent_target_has_no_key() {
    let values = tag_values(&tags(&["triage:kind=defect"]));
    assert_eq!(values.get("triage:exposed=*"), None);
    assert_eq!(values.get("triage:exposed"), None);
}

#[test]
fn test_tag_values_a_target_holding_an_equals_sign_shadows_no_composite_key() {
    // `ns:"impact=3"=7` has the key `impact=3`; its bare entry would land
    // on the composite key `ns:impact=3`.
    let values = tag_values(&tags(&[r#"ns:"impact=3"=7"#]));
    assert_eq!(values.get("ns:impact=3"), None);
    assert!(values.is_empty());
}

#[test]
fn test_tag_values_an_untargeted_tag_counts_under_its_key() {
    let values = tag_values(&tags(&["urgent"]));
    assert_eq!(values.get("urgent"), Some(&1.0));
}

#[test]
fn test_composite_keys_drive_formulas() {
    let all = [
        task("big", TO_DO, &["widget:kind=big"], 0),
        task("small", TO_DO, &["widget:kind=small"], 0),
        task("bare", TO_DO, &["widget:flag"], 0),
    ];
    let ranking = ranked(
        &all,
        &[(
            PriorityFn,
            "widget:score",
            "{{widget:kind=big}} * 10 + {{widget:flag=*}}",
        )],
    )
    .unwrap();
    assert_eq!(raw(&all, &ranking, "big"), 10.0);
    assert_eq!(raw(&all, &ranking, "small"), 0.0);
    assert_eq!(raw(&all, &ranking, "bare"), 1.0);
}

// ------------------------------------------------------------ normalization

#[test]
fn test_rank_normalize_ties_share_a_percentile_and_the_maximum_reaches_max() {
    let got = rank_normalize(&[1.0, 1.0, 3.0, 2.0, 5.0]);
    assert_eq!(got[0], got[1], "tied raw values share a percentile");
    assert_eq!(got[0], MAX * 2.0 / 5.0);
    assert!(got[0] < got[3] && got[3] < got[2] && got[2] < got[4]);
    assert_eq!(got[4], MAX);
}

#[test]
fn test_rank_normalize_an_empty_population_ranks_nothing() {
    assert!(rank_normalize(&[]).is_empty());
}

#[test]
fn test_rank_normalize_a_nan_neither_hangs_nor_propagates() {
    let got = rank_normalize(&[1.0, f64::NAN, 3.0]);
    assert_eq!(got.len(), 3);
    assert!(got.iter().all(|p| !p.is_nan()));
}

// ------------------------------------------------------------ diagnostics

#[test]
fn test_no_priority_fn_ranks_every_live_task_max_and_says_it_means_nothing() {
    let all = [
        task("a", TO_DO, &[], 0),
        task("b", "In Progress", &["triage:kind=defect"], 0),
        task("done", "Done", &[], 0),
    ];
    let ranking = ranked(&all, &[]).unwrap();
    for harp in ["a", "b"] {
        assert_eq!(score(&all, &ranking, harp).priority, MAX, "{harp}");
        assert_eq!(raw(&all, &ranking, harp), 0.0, "{harp}");
    }
    assert_eq!(score(&all, &ranking, "done").priority, 0.0);
    assert!(ranking.diagnostics.no_priority_fn);
    assert!(ranking.diagnostics.all_tied);
    assert!(ranking.diagnostics.target_coverage.is_empty());
}

#[test]
fn test_a_declared_priority_fn_clears_no_priority_fn() {
    let all = [task("a", TO_DO, &[], 0)];
    let ranking = ranked(&all, &[(PriorityFn, "widget:score", "1")]).unwrap();
    assert!(!ranking.diagnostics.no_priority_fn);
}

#[test]
fn test_all_tied_needs_more_than_one_live_task_on_one_score() {
    let one = [task("a", TO_DO, &[], 0)];
    let tied = [task("a", TO_DO, &[], 0), task("b", TO_DO, &[], 0)];
    let differ = [
        task("a", TO_DO, &["widget:score=1"], 0),
        task("b", TO_DO, &["widget:score=2"], 0),
    ];
    let formula = [(PriorityFn, "widget:score", "{{widget:score}}")];
    assert!(!ranked(&one, &formula).unwrap().diagnostics.all_tied);
    assert!(ranked(&tied, &formula).unwrap().diagnostics.all_tied);
    assert!(!ranked(&differ, &formula).unwrap().diagnostics.all_tied);
}

#[test]
fn test_scored_tasks_counts_live_tasks_carrying_a_referenced_target() {
    let all = [
        task("scored", TO_DO, &["widget:kind=big"], 0),
        task("unrelated", TO_DO, &["urgent"], 0),
        task("bare", "In Progress", &[], 0),
        task("parked", DEFERRED, &[], 0),
        task("finished", "Done", &["widget:kind=big"], 0),
        task("shelved", "Archived", &[], 0),
    ];
    let ranking = ranked(&all, &[(PriorityFn, "widget:score", "{{widget:kind=big}}")]).unwrap();
    assert_eq!(ranking.diagnostics.scored_tasks, 1);
    assert_eq!(
        ranking.diagnostics.non_terminal_tasks, 4,
        "Deferred is live (it only skips decay); Done and Archived are not"
    );
}

#[test]
fn test_target_coverage_names_a_dead_term_worst_first() {
    let all = [
        task("a", TO_DO, &["widget:score=1"], 0),
        task("b", TO_DO, &["widget:score=2"], 0),
        task("c", "In Progress", &["widget:score=3"], 0),
    ];
    let ranking = ranked(
        &all,
        &[(
            PriorityFn,
            "widget:score",
            "{{widget:score}} + {{widget:never}}",
        )],
    )
    .unwrap();
    assert_eq!(ranking.diagnostics.scored_tasks, 3);
    assert_eq!(
        ranking.diagnostics.target_coverage,
        [
            TargetCoverage {
                target: "widget:never".to_string(),
                tasks: 0
            },
            TargetCoverage {
                target: "widget:score".to_string(),
                tasks: 3
            },
        ]
    );
}

#[test]
fn test_target_coverage_counts_only_live_tasks() {
    let all = [
        task("live", TO_DO, &["widget:score=1"], 0),
        task(
            "finished",
            "Done",
            &["widget:score=2", "widget:historic=9"],
            0,
        ),
        task("shelved", "Archived", &["widget:historic=9"], 0),
    ];
    let ranking = ranked(
        &all,
        &[(
            PriorityFn,
            "widget:score",
            "{{widget:score}} + {{widget:historic}}",
        )],
    )
    .unwrap();
    assert_eq!(
        ranking.diagnostics.target_coverage,
        [
            TargetCoverage {
                target: "widget:historic".to_string(),
                tasks: 0
            },
            TargetCoverage {
                target: "widget:score".to_string(),
                tasks: 1
            },
        ]
    );
}

#[test]
fn test_target_coverage_counts_presence_under_the_bare_target_of_both_facets() {
    let all = [
        task("bare", TO_DO, &["widget:flag"], 0),
        task(
            "valued",
            TO_DO,
            &["widget:flag=yes", "widget:kind=small"],
            0,
        ),
    ];
    let ranking = ranked(
        &all,
        &[
            (
                PriorityFn,
                "widget:score",
                "{{widget:flag=*}} + {{age_factor}}",
            ),
            (DecayFn, "widget:score", "{{widget:kind=big}} + 1"),
        ],
    )
    .unwrap();
    assert_eq!(
        ranking.diagnostics.target_coverage,
        [
            TargetCoverage {
                target: "widget:kind".to_string(),
                tasks: 1
            },
            TargetCoverage {
                target: "widget:flag".to_string(),
                tasks: 2
            },
        ]
    );
}

// ------------------------------------------------------------ refusals

#[test]
fn test_a_malformed_formula_is_refused_naming_its_target() {
    let err = formulas(&[(PriorityFn, "widget:foo", "{{{{ not a valid expr ((")]).unwrap_err();
    assert!(
        matches!(&err, PriorityError::Malformed { facet: PriorityFn, target, .. } if target == "widget:foo")
    );
}

#[test]
fn test_more_than_one_priority_fn_is_ambiguous() {
    let err = formulas(&[
        (PriorityFn, "triage:severity", "{{triage:severity}}"),
        (PriorityFn, "widget:foo", "{{widget:foo}}"),
    ])
    .unwrap_err();
    assert_eq!(
        err,
        PriorityError::Ambiguous {
            facet: PriorityFn,
            targets: vec!["triage:severity".to_string(), "widget:foo".to_string()],
        }
    );
}

#[test]
fn test_more_than_one_decay_fn_is_ambiguous() {
    let err = formulas(&[(DecayFn, "a:b", "1"), (DecayFn, "c:d", "1")]).unwrap_err();
    assert!(matches!(
        err,
        PriorityError::Ambiguous { facet: DecayFn, .. }
    ));
}

#[test]
fn test_every_declared_formula_is_compiled_before_ambiguity_is_judged() {
    let err = formulas(&[(PriorityFn, "a:b", "1"), (PriorityFn, "c:d", "1 +")]).unwrap_err();
    assert!(matches!(err, PriorityError::Malformed { .. }), "{err:?}");
}

#[test]
fn test_an_unknown_builtin_is_refused() {
    let err = formulas(&[(PriorityFn, "widget:foo", "{{widget:foo}} * {{typo}}")]).unwrap_err();
    assert_eq!(
        err,
        PriorityError::UnknownBuiltin {
            facet: PriorityFn,
            target: "widget:foo".to_string(),
            name: "typo".to_string(),
        }
    );
}

#[test]
fn test_decay_fn_may_not_read_its_own_age_factor() {
    let err = formulas(&[(DecayFn, "widget:foo", "{{age_factor}}")]).unwrap_err();
    assert!(matches!(&err, PriorityError::UnknownBuiltin { name, .. } if name == AGE_FACTOR));
}

#[test]
fn test_the_known_builtins_compile() {
    formulas(&[
        (DecayFn, "widget:foo", "1 + {{age_days}}/100"),
        (
            PriorityFn,
            "widget:foo",
            "{{widget:foo}} * {{age_factor}} + {{age_days}}",
        ),
    ])
    .unwrap();
    assert_eq!(super::builtins(DecayFn), [AGE_DAYS]);
    assert_eq!(super::builtins(PriorityFn), [AGE_DAYS, AGE_FACTOR]);
}

#[test]
fn test_an_unknown_name_is_refused_when_evaluated() {
    // expr-lang checks names when it runs, not when it compiles.
    let all = [task("a", TO_DO, &[], 0)];
    let err = ranked(&all, &[(PriorityFn, "widget:foo", "foo + 1")]).unwrap_err();
    assert!(
        matches!(&err, PriorityError::Unevaluable { harp_id, facet: PriorityFn, .. } if harp_id == "a"),
        "{err:?}"
    );
}

#[test]
fn test_a_result_that_is_no_number_is_refused() {
    let all = [task("a", TO_DO, &[], 0)];
    let err = ranked(&all, &[(DecayFn, "widget:foo", "{{age_days}} > 1")]).unwrap_err();
    assert!(
        matches!(&err, PriorityError::NotANumber { harp_id, facet: DecayFn, result, .. } if harp_id == "a" && result == "false"),
        "{err:?}"
    );
}

#[test]
fn test_a_non_finite_score_is_refused() {
    let all = [task("a", TO_DO, &[], 0)];
    let err = ranked(&all, &[(PriorityFn, "widget:foo", "1 / 0")]).unwrap_err();
    assert!(
        matches!(&err, PriorityError::NonFinite { harp_id, .. } if harp_id == "a"),
        "{err:?}"
    );
}

#[test]
fn test_an_unset_creation_time_is_refused() {
    let mut all = [task("a", TO_DO, &[], 0)];
    all[0].created_at = Timestamp::default();
    let err = ranked(&all, &[(DecayFn, "widget:foo", "{{age_days}}")]).unwrap_err();
    assert_eq!(
        err,
        PriorityError::NoCreatedAt {
            harp_id: "a".to_string()
        }
    );
}

#[test]
fn test_a_creation_time_in_the_future_is_age_zero() {
    let all = [task("a", TO_DO, &[], -90)];
    let ranking = ranked(
        &all,
        &[
            (DecayFn, "widget:foo", "1 + {{age_days}}/({{age_days}}+90)"),
            (PriorityFn, "widget:foo", "{{age_factor}} + {{age_days}}"),
        ],
    )
    .unwrap();
    assert_eq!(raw(&all, &ranking, "a"), 1.0);
}

// ------------------------------------------------------------ catalog

#[test]
fn test_every_error_has_a_catalog_message_with_exactly_its_args() {
    let (_, ftl) = super::EN_US;
    let resource = fluent_syntax::parser::parse(ftl).expect("the catalog parses");
    let mut catalog: Vec<(String, Vec<String>)> = resource
        .body
        .iter()
        .filter_map(|entry| match entry {
            fluent_syntax::ast::Entry::Message(m) => Some((m.id.name.to_string(), variables(m))),
            _ => None,
        })
        .collect();
    catalog.sort();
    let mut samples: Vec<(String, Vec<String>)> = super::diagnostic_samples()
        .iter()
        .map(|d| {
            let mut args: Vec<String> = d.args().iter().map(|(n, _)| n.to_string()).collect();
            args.sort();
            (d.code().to_string(), args)
        })
        .collect();
    samples.sort();
    assert_eq!(catalog, samples);
}

#[test]
fn test_each_error_renders_its_english_fallback() {
    for sample in super::diagnostic_samples() {
        assert!(!sample.to_string().is_empty(), "{}", sample.code());
    }
    let unknown = PriorityError::UnknownBuiltin {
        facet: DecayFn,
        target: "a:b".to_string(),
        name: "typo".to_string(),
    };
    assert_eq!(
        unknown.to_string(),
        "decay_fn on a:b reads typo, which is no tag (no ':') and no built-in of decay_fn (those are age_days)"
    );
    assert!(unknown
        .args()
        .iter()
        .any(|(name, arg)| *name == "known" && matches!(arg, Arg::Str(s) if s == "age_days")));
    let err = PriorityError::Ambiguous {
        facet: PriorityFn,
        targets: vec!["a:b".to_string(), "c:d".to_string()],
    };
    assert_eq!(
        err.to_string(),
        "more than one priority_fn is declared (on a:b, c:d): exactly one is allowed"
    );
}

/// The variables `message`'s pattern names, sorted.
fn variables(message: &fluent_syntax::ast::Message<&str>) -> Vec<String> {
    use fluent_syntax::ast::{Expression, InlineExpression, PatternElement};
    let mut names = Vec::new();
    for element in message.value.iter().flat_map(|p| &p.elements) {
        if let PatternElement::Placeable {
            expression: Expression::Inline(InlineExpression::VariableReference { id }),
        } = element
        {
            names.push(id.name.to_string());
        }
    }
    names.sort();
    names.dedup();
    names
}
