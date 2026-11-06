# Ported from journeys/j002400_container.feature (rev f36e8dfb2)
# NOT ported: the scenarios tagged @pending: it waits for `ctxloom run` to
#   launch an engine in a container. `container check` and `container build`
#   are cli/container.feature's.

@pending
Feature: The container runtime axis — can an engine actually land in a container here, and did it?

  Some tasks Alice would rather not run against her own machine: an
  untrusted repository, a risky migration, an agent with its prompts turned
  off. She gives that agent the container runtime, so its engine runs in a
  container with a fresh home and only the project mounted, instead of in her
  own environment. Before relying on it, she wants to see it: that the agent
  really ran somewhere her machine is not.

  Background:
    Given an initialized ctxloom project

  # Alice runs the same agent twice, first on her machine and then in a
  # container. The first run shows what her machine looks like; the second
  # ran somewhere else, and its work still reached her project.
  @container-only @image-mock-agent
  Scenario: A containerized run puts the engine somewhere this host is not
    When Alice runs the mock agent with runtime "host"
    Then the engine left its record, and it ran on this host
    When Alice runs the mock agent with runtime "container"
    Then the engine left its record, and it ran inside a container, not on this host
