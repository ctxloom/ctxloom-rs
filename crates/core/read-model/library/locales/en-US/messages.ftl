# Messages for the crate's errors. Each id is an error's code, which begins
# with the crate's name; the variables are its args. An id ending in -fix is
# the remedy of the error whose code it follows.

library-ref-syntax = malformed bundle reference: { $detail }
library-ref-selector = selector "{ $selector }" must be <kind>/<name>
library-ref-unknown-kind = unknown item kind "{ $kind }" (want fragments|commands|mcp|hooks|skills)

## Profiles. $cause of a syntax error is the YAML reader's own message.

library-profile-syntax = invalid YAML: { $cause }
library-profile-unknown-key =
    profile does not match the profile schema: `/`: additional property `{ $key }` (line { $line }) is not a key a profile declares{ $near ->
        [1] { " — did you mean `" }{ $suggestion }{ "` for `" }{ $key }{ "`?" }
       *[other] { "" }
    } (the keys there are: { $valid }); correct the named keys and values
library-profile-bad-value = profile does not match the profile schema: `/{ $key }`: { $detail }; correct the named keys and values
library-profile-empty-fragment = empty fragment reference in profile fragments list (line { $line }): a fragments entry must name a fragment
library-profile-cross-repo = profile { $profile }: a remote profile may refer only to bundles in its own repository: { $reference }
library-profile-bad-name = invalid profile name "{ $name }": '#' is reserved for bundle refs (<bundle>#profiles/<name>)
library-profile-not-found = profile not found: { $name }
library-profile-circular = circular profile inheritance detected: { $name }
library-profile-depth-exceeded = profile inheritance depth exceeds maximum ({ $max }): possible misconfiguration
library-profile-parent = failed to resolve parent { $parent }: { $cause }
library-profile-unreadable = profile { $name } cannot be read: { $ext_err_txt }

## Bundles.

library-bundle-empty = bundle is empty: { $bytes } bytes parsed to a document declaring no version and no items (fragments, commands, skills, mcp, profiles or hooks) — an empty, truncated, comment-only or `null` file parses as valid YAML but is not a bundle
library-bundle-newer = bundle: schema_version { $generation }: written by a newer format than this binary reads; upgrade the binary (this binary reads up to { $current })
library-bundle-not-integer = bundle: format version cannot be read: schema_version is not an integer
library-bundle-syntax = invalid bundle YAML: { $cause }
library-bundle-unknown-key =
    invalid bundle YAML: unknown key `{ $key }` on line { $line } (at { $site }): a bundle may not declare a key ctxloom does not model — it would be dropped in silence and the bundle would quietly ship less than it declares{ $near ->
        [1] { " — did you mean `" }{ $suggestion }{ "`?" }
       *[other] { "" }
    } (valid keys there: { $valid })
library-bundle-bad-value = invalid bundle YAML: { $path }: { $detail }
library-bundle-no-manifest = bundles: tree bundle "{ $bundle }" has no bundle.yaml, so it carries no version or description and cannot be read as a bundle
library-bundle-inline-and-items = bundles: bundle.yaml of tree bundle "{ $bundle }" still declares { $keys } inline while the tree also holds item files; a half-migrated bundle has two answers for one item and no rule for which wins — finish the migration by removing the inline keys
library-bundle-item = bundles: { $item } in tree bundle "{ $bundle }": { $detail }
library-bundle-unclaimed = content: bundle "{ $bundle }" has a file that no surface type recognises: { $path }
library-bundle-unexplained-meta = content: { $kind } item { $item } carries the metadata file { $path }, but { $kind } items keep no separate metadata file (the .<name>.meta.yaml name is reserved for ctxloom metadata, so the file is refused rather than silently ignored; remove or rename it)
library-bundle-hook-matcher = content: hook { $hook }: matcher "{ $matcher }" names an engine's own tools; narrow the hook with `tool:` (one of shell, file_edit, skill) instead
library-bundle-skill-empty = bundles: skill "{ $skill }" in tree bundle "{ $bundle }": the package has NO files; refusing to read it as an empty skill (an empty skill materializes without complaint and delivers nothing)

## The catalog.

library-catalog-bundle-not-found = bundle not found: { $ask }
library-catalog-ambiguous = bundle name resolves to more than one bundle: "{ $ask }" names more than one bundle: { $candidates }
library-catalog-fragment-not-found = fragment not found: "{ $item }" in bundle "{ $bundle }"
library-catalog-fragment-name-not-found = fragment not found: { $name }
library-catalog-command-not-found = command not found: "{ $item }" in bundle "{ $bundle }"
library-catalog-command-name-not-found = command not found: { $name }
library-catalog-bad-item-ref = bad item reference: "{ $ask }" selects a { $kind }, not a { $wanted }
library-catalog-item-syntax = invalid item reference "{ $ask }": { $cause }
library-bundle-profile = invalid bundle YAML: profile { $name }: { $cause }

## The config.

library-config-syntax = yaml: { $cause }
library-config-not-a-mapping = the config's root must be a mapping
library-profile-not-found-fix = `ctxloom profile list` names every profile you can use
