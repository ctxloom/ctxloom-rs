tasks-application-gave-up = gave up after { $attempts } attempts: { $error }
tasks-application-harps-exhausted = no free task id after { $attempts } tries
tasks-application-vanished = task { $harp_id } was written but cannot be read back
tasks-application-no-project = there is no project here yet, so no task to change
tasks-application-no-go-log = Go taskloom has no task log for project { $project }
tasks-application-unknown-status-class = { $status } is no status class (the classes are { $classes }): '@' is reserved for them, so no status starts with it
tasks-application-unknown-sort = { $sort } is no sort: the only one is priority
tasks-application-priority-no-priority-fn = --sort priority is meaningless here: this project's tag_schema declares no priority_fn, so every task's raw score is 0 and the ranking reflects nothing
tasks-application-priority-all-tied = --sort priority is meaningless here: every active task ties at the same raw priority score (only { $scored } of { $live } active tasks carry a tag any priority_fn/decay_fn formula actually reads) — the ranking reflects nothing
tasks-application-priority-inert = priority_fn/decay_fn reads { $target }, which none of the { $live } active tasks carries — that term is inert and moves no task's rank
tasks-application-priority-thin = { $uncovered } of { $live } active tasks carry no { $target } — they rank as if it were absent, on a formula that reads it
tasks-application-priority-in-project = project { $project }: { $warning }
tasks-application-not-found = no task holds { $count ->
    [one] this harp id
   *[other] these harp ids
}: { $harp_ids } (see `taskloom list`)
tasks-application-ambiguous = { $count ->
    [one] a harp id is
   *[other] harp ids are
} held by more than one project ({ $held }): pass --project <id> instead of --global to choose one
tasks-application-import-displaced = another task holds this harp in Go's log: run Go's `taskloom repair`, then import again
tasks-application-import-harp-taken = a task not imported from this record already holds this harp
tasks-application-import-task-not-imported = its task was not imported
tasks-application-tag-unparseable = the tag { $tag } is not a tag in tagma's grammar: { $detail }
tasks-application-tag-operator-word = the tag { $tag } is a query operator (and, or, not, in any case), so no query could match it
tasks-application-tag-reserved-namespace = the tag { $tag } is in the namespace { $namespace }: tagma's namespace is reserved for the tag schema's declarations
tasks-application-tag-not-in-enum = the tag { $tag }'s value { $value } is not one of { $target }'s declared values: { $members }
tasks-application-tag-not-a-number = the tag { $tag }'s value { $value } is not a finite number, which { $target }'s declared range [{ $min }, { $max }] requires
tasks-application-tag-out-of-range = the tag { $tag }'s value { $value } is outside { $target }'s declared range [{ $min }, { $max }]
tasks-application-tag-not-semver = the tag { $tag }'s value { $value } is not a SemVer 2.0.0 version, which { $target }'s declared type semver requires: { $detail }
tasks-application-schema-unparseable = tag_schema: { $declaration } is not a tag in tagma's grammar: { $detail }
tasks-application-schema-no-namespace = tag_schema: { $declaration } has no namespace: expected tagma.<facet>
tasks-application-schema-not-a-facet = tag_schema: { $declaration }'s namespace { $namespace } is not tagma.<facet> (e.g. tagma.arity)
tasks-application-schema-unknown-facet = tag_schema: { $declaration } names the unknown facet { $facet } (known: { $known })
tasks-application-schema-no-value = tag_schema: { $declaration } has no value
tasks-application-schema-arity-target = tag_schema: { $declaration } targets { $target }, but an arity's target must be namespace:key
tasks-application-schema-enum-empty = tag_schema: { $declaration } lists no enum member in { $value }
tasks-application-schema-range = tag_schema: { $declaration }'s range { $value } is not min,max: two finite numbers, the least first
tasks-application-tag-query = the tag query { $query } is malformed (as postfix: { $postfix }; as infix: { $infix }): { $grammar }
tasks-application-lint-not-in-enum = { $target }={ $value } is not one of the declared enum values: { $members }
tasks-application-lint-not-a-number = { $target }={ $value } does not parse as a finite number
tasks-application-lint-out-of-range = { $target }={ $value } is outside the declared range [{ $min }, { $max }]
tasks-application-lint-scalar = { $target } carries { $count } distinct values ({ $values }) but is declared arity=scalar
tasks-application-lint-formula-enum = { $facet } on { $target } references the placeholder { $placeholder }, but { $value } is not one of { $reference }'s declared enum values: { $members }
tasks-application-prune-no-status = a prune needs at least one status to select by: an empty status set is never read as every status
tasks-application-prune-age-required = a prune needs an age: an offset like 30d, or a date like 2026-01-01
tasks-application-prune-age-not-positive = the age { $age } is not a positive age
tasks-application-prune-age-unreadable = the age { $age } is neither an offset (30d, 12w, 720h) nor a date (2026-01-01)
