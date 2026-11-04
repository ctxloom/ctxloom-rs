# Messages for the crate's errors. Each id is an error's code, which begins
# with the crate's name; the variables are its args.

content-fs-list-root = cannot list the bundles directory { $path }: { $ext_err_txt }
content-fs-list-dir = cannot list { $path }: { $ext_err_txt }
content-fs-read-file = cannot read { $path }: { $ext_err_txt }
content-fs-bad-name = "{ $name }" is not the name of a bundle: a name is a path below the bundles directory and does not climb out of it
