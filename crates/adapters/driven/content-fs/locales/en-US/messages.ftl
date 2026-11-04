# Messages for the crate's errors. Each id is an error's code, which begins
# with the crate's name; the variables are its args.

content-fs-list-root = cannot list the bundles directory { $path }: { $ext_err_txt }
content-fs-list-dir = cannot list { $path }: { $ext_err_txt }
content-fs-read-file = cannot read { $path }: { $ext_err_txt }
content-fs-bad-name = "{ $name }" is not the name of a bundle: a name is a path below the bundles directory and does not climb out of it
content-fs-bad-path = "{ $path }" is not a path inside a bundle: it is absolute, climbs out, or goes through a link
content-fs-write-file = cannot write { $path }: { $ext_err_txt }
content-fs-remove-path = cannot remove { $path }: { $ext_err_txt }
content-fs-make-dir = cannot make the directory { $path }: { $ext_err_txt }
