# world: surfaces
# Ported from cli/util.feature
# NOT ported: nothing; the one scenario is whole.


Feature: util — writing into config files ctxloom does not own

  `ctxloom util config-write` merges a JSON patch into a FOREIGN config file:
  one a user or another tool authored, that ctxloom edits and must hand back
  intact. Everything about it is shaped by that ownership. It backs the file up
  before touching it, it refuses rather than overwrite a file it cannot parse,
  and it re-reads what it wrote to confirm the merge is actually on disk rather
  than trusting a clean exit code.

  Underneath, a write goes through hew: ctxloom records what it applied, and the
  NEXT write reverses that record before applying the new set. That is what lets
  a second write replace ctxloom's own entry without disturbing the user's, and
  it is why the interesting scenario here is not one write but three.

  Rule: a write is reversible, so writing again replaces rather than accumulates

    The record ctxloom keeps of its own application is the whole mechanism. If a
    write cannot re-read the record the previous write left, ctxloom cannot take
    its old entry back out — and it correctly refuses to write at all rather
    than clobber. Correct, and useless: the file silently stops being managed.

    # THREE writes, and each one is load-bearing. A record's reversal describes
    # the DIFFERENCE between two images, so its shape depends on what changed:
    # adding a whole entry yields ops whose values are MAPPINGS, while changing
    # one field of an entry that already exists yields ops whose values are
    # SCALARS and SEQUENCES. Those are different serialization paths, and a
    # suite that only ever adds an entry exercises one of them.
    #
    # Measured 2026-08-16: a defect that broke every write after the first
    # survived a full green acceptance run for exactly this reason. Every
    # scenario in the corpus wrote a foreign config once, or wrote it repeatedly
    # without changing a field, so the record being re-read always held mapping
    # values. The leaf-valued record was never built by any scenario.
    #
    # WRITE TWO IS WHERE THIS EARNED ITS KEEP. It caught a data-corruption
    # defect in hew: Sel.Set is OP-03 upsert, documented "present -> replaced
    # regardless of current value", and it behaved that way for a scalar but on
    # a path holding a SEQUENCE it appended the new sequence as a nested element
    # instead of replacing the node. Reduced to hew alone:
    #
    #   in:   {"s": {"args": ["a", "b"], "cmd": "old"}}
    #   Set /s/args = ["a","b","c"];  Set /s/cmd = "new"
    #   out:  {"s": {"args": ["a", "b", ["a", "b", "c"]], "cmd": "new"}}
    #
    # Through config-write that was `"args": ["mcp", "serve", ["mcp", "serve"]]`.
    # The command's payload verification caught it and exited non-zero — the
    # system working — but the corrupted file was left on disk.
    #
    # Only a caller that recurses to leaves reached it: recordConfigPatch
    # descends into nested maps, while the .mcp.json writer reverses the whole
    # entry out before re-adding, so it never Set an array over an existing one.
    # That is why .mcp.json looked healthy while this did not.
    #
    # Fixed upstream (hew: each format's planInsert/planAdd disambiguated
    # sequence-append from map-add by asking only "does this path resolve to a
    # sequence?", never consulting on_conflict, so an upsert took the append
    # branch). Untagged once ctxloom's pin carried the fix.
    #
    # DO NOT rewrite this to avoid arrays. Every MCP server ctxloom writes has an
    # `args` array; an assertion that dodges the shape would report success while
    # the defect ships.
    Scenario: A third write still replaces ctxloom's entry after one field changed
      Given an initialized ctxloom project
      And the project already has the file "vendor-tool.json":
        """
        {
          "$schema": "https://example.com/vendor.schema.json",
          "servers": {
            "vendor-owned": {
              "command": "vendor-server",
              "args": ["--serve"],
              "env": {"VENDOR_TOKEN": "keep-me-verbatim"}
            }
          }
        }
        """

      # Write one: ctxloom adds its entry beside the vendor's.
      When I run "ctxloom util config-write --file $PROJECT_DIR/vendor-tool.json" with input:
        """
        {"servers": {"ctxloom": {"command": "/first/ctxloom", "args": ["mcp", "serve"]}}}
        """
      Then the command succeeds
      And the file "vendor-tool.json" contains "/first/ctxloom"

      # Write two changes ONE field. The record this leaves describes a
      # leaf-level change, which is the shape write three has to read back.
      When I run "ctxloom util config-write --file $PROJECT_DIR/vendor-tool.json" with input:
        """
        {"servers": {"ctxloom": {"command": "/second/ctxloom", "args": ["mcp", "serve"]}}}
        """
      Then the command succeeds
      And the file "vendor-tool.json" contains "/second/ctxloom"

      # Write three must re-read the leaf-valued record from write two. This is
      # the step that failed: the write was refused and the file stopped being
      # managed, while the command still reported nothing wrong to a caller that
      # only checked whether the file existed.
      When I run "ctxloom util config-write --file $PROJECT_DIR/vendor-tool.json" with input:
        """
        {"servers": {"ctxloom": {"command": "/third/ctxloom", "args": ["mcp", "serve"]}}}
        """
      Then the command succeeds
      And the file "vendor-tool.json" contains "/third/ctxloom"

      # REPLACED, not accumulated: exactly one ctxloom command survives, and it
      # is the newest. Asserting only that the new value is present would pass
      # just as well on a file carrying all three.
      #
      # Counted on the COMMAND VALUE, not on "ctxloom" — that bare substring is
      # legitimately present twice (the server key, and again inside the command
      # path), so counting it asserts a coincidence rather than the claim.
      And the file "vendor-tool.json" does not contain "/first/ctxloom"
      And the file "vendor-tool.json" does not contain "/second/ctxloom"
      And the file "vendor-tool.json" contains "/third/ctxloom" exactly 1 times

      # And the vendor's own content is still byte-for-byte theirs, through all
      # three writes. This is the claim the whole command exists to keep.
      And the file "vendor-tool.json" contains "https://example.com/vendor.schema.json"
      And the file "vendor-tool.json" contains "keep-me-verbatim"
      And the file "vendor-tool.json" contains "vendor-server"
      And the file "vendor-tool.json" contains "--serve"
