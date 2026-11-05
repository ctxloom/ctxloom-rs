# Messages for the crate's errors. Each id is a code, which begins with the
# crate's name; the variables are its args. An id ending -fix is the remedy of the one before.

worktree-git-unadoptable = { $path } already exists but cannot be shown to be this agent's own checkout on { $branch } ({ $why }); it is not adopted, and the run is not moved elsewhere
worktree-git-unadoptable-fix = inspect { $path } by hand: if it is this agent's work, check out { $branch } there; otherwise remove it (git worktree remove) and resume the agent again
worktree-git-unreadable = cannot read { $path }: { $why }
worktree-git-git-timed-out = git { $command } did not finish within { $timeout } and was killed
worktree-git-git-timed-out-fix = check that the repository is reachable and that no other git process holds it (a stale index.lock, a hung hook or a stalled network mount), then run again
