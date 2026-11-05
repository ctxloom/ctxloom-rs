# world: base
# Ported from cli/llm.feature
# NOT ported: every other scenario of the Go file. Ported: the env: block
#   under a label being refused with its key named.
@doc
Feature: llm — the named engine configurations

  Rule: A config carrying an env: block under a label is refused, not ignored

    A silently-dropped key is the worse outcome: a user who wrote `env:` under
    a label believes their variable reached the engine, and nothing tells them
    otherwise. So the command fails naming the key by its path, label included.

    Scenario: An env: block under a label fails the command and names the key
      Given an initialized ctxloom project
      And the project already has the file ".ctxloom/config.yaml":
        """
        schema_version: 8
        llm:
          configs:
            big:
              type: mock
              env:
                OPENAI_API_KEY: sk-acceptance-MUST-NEVER-BE-APPLIED
        """
      When I run "ctxloom llm list"
      Then the command fails
      And the output contains "llm.configs.big.env"
