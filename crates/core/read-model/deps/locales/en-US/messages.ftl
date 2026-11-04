# Messages for the crate's errors. Each id is an error's code, which begins
# with the crate's name; the variables are its args. An id ending in -fix is
# the remedy of the error it follows.

deps-unreadable = { $path } cannot be read: { $cause }
deps-unreadable-fix = fix the file by hand, or delete it and let ctxloom write a fresh one
deps-too-new = { $path }: ctxloom { $kind }: schema_version { $found }: written by a newer format than this binary reads; upgrade the binary (this binary reads up to { $current })
deps-too-new-fix = upgrade ctxloom: { $path } was written by a newer one
deps-too-old = { $path }: ctxloom { $kind }: schema_version { $found }: older than the oldest format this binary can migrate (the oldest is { $oldest })
deps-too-old-fix = back up { $path }, then add `schema_version: { $oldest }` to it
deps-bad-shape = { $path }: { $key }: { $detail }
deps-bad-shape-fix = fix the file by hand, or delete it and let ctxloom write a fresh one
deps-forge-type = forge "{ $forge }": unknown type "{ $found }" (want "github" or "git")
deps-forge-type-fix = set the forge's type to github or git in remotes.yaml
deps-unknown-forge = unknown forge "{ $label }": configure it under forges: or use a built-in ("github", "git")
deps-unknown-forge-fix = use github or git, or add the forge under forges: in remotes.yaml
deps-no-repository = remote URL "{ $url }" names no repository: { $detail }
deps-no-repository-fix = write owner/repo, or the repository's full URL
deps-remote-exists = remote already exists: { $name }
deps-remote-exists-fix = choose another name, or `ctxloom remote edit { $name }`
deps-remote-not-found = remote not found: { $name }
deps-remote-not-found-fix = `ctxloom remote list` names the registered remotes
deps-same-repository = remote '{ $existing }' already points to this URL; use the existing remote: `ctxloom remote show { $existing }` lists its bundles
deps-empty-name = new name must not be empty
deps-nothing-to-edit = nothing to edit: pass at least one of --name, --url or --forge
deps-lock-empty = lockfile holds no document (it is empty, or whitespace and comments only): { $path } — this is not the same as no lockfile at all (which is fine)
deps-lock-empty-fix = delete { $path } and re-run `ctxloom deps pull` to rebuild it
deps-lock-key = lockfile entry is not keyed by its bundle identity: { $path }: "{ $key }"
deps-lock-key-fix = delete { $path } and re-run `ctxloom deps pull` to rebuild it
deps-lock-would-erase = refusing to erase lockfile entries: the write would replace { $count } locked entry(ies) in { $path } with none
deps-lock-would-erase-fix = check that the config loaded and the profiles are there; this is a bug in the caller unless you meant to unlock everything
deps-lock-unreadable = refusing to overwrite an unreadable lockfile: { $path }: { $detail }
deps-lock-unreadable-fix = fix the file, or delete it to start a fresh lock (every pin and hold it records will be lost)
