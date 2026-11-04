# Messages for the crate's errors. Each id is an error's code, which begins
# with the crate's name; the variables are its args. A third party's text (the
# OS's) is the arg `ext_err_txt`, verbatim.

editor-process-spawn = cannot run the editor { $program }: { $ext_err_txt }
editor-process-failed = the editor { $program } exited with { $status }
editor-process-temp = cannot prepare the file for the editor: { $ext_err_txt }
editor-process-read = cannot read what the editor left in { $path }: { $ext_err_txt }
editor-process-bad-name = "{ $name }" is not a file name for the editor: it is one name, not a path
