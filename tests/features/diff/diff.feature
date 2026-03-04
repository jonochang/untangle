Feature: Diff CLI

  Scenario: Diff with identical refs passes
    Given the diff fixtures
    When I run diff with identical refs
    Then the diff verdict is pass
