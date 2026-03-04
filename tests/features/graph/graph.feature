Feature: Graph CLI

  Scenario: Graph output is DOT
    Given the analyze fixtures
    When I run graph in dot format
    Then the graph output is dot
