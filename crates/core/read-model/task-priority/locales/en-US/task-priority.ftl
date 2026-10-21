task-priority-malformed = { $facet } on { $target } is not a formula: { $reason } (formula: { $formula })
task-priority-unknown-builtin = { $facet } on { $target } reads { $name }, which is no tag (no ':') and no built-in of { $facet } (those are { $known })
task-priority-ambiguous = more than one { $facet } is declared (on { $targets }): exactly one is allowed
task-priority-unevaluable = { $facet } could not score task { $harp_id }: { $reason } (formula: { $formula })
task-priority-not-a-number = { $facet } gives task { $harp_id } { $result }, which is no number (formula: { $formula })
task-priority-non-finite = priority_fn gives task { $harp_id } the score { $score }, which is no finite number: check its tags for a value that is not one
task-priority-no-created-at = task { $harp_id } has no creation time, so it has no age to rank by
