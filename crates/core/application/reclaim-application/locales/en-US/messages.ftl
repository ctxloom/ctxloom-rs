# Messages for the crate's errors. Each id is a code, which begins with the
# crate's name; the variables are its args.

reclaim-application-bad-age = { $flag } { $value } is neither an age (30d, 12w, 720h) nor a date (2026-01-01)
reclaim-application-bad-age-fix = give an offset such as 90d, 12w or 720h, or a date such as 2026-01-01
