Feature: Config CLI

  Scenario: Config show defaults
    Given an empty temp project
    When I run config show
    Then the config output shows defaults
