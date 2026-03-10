Feature: Analyze Graph CLI

  Scenario: Analyze graph output is DOT
    Given the analyze fixtures
    When I run analyze graph in dot format
    Then the graph output is dot
