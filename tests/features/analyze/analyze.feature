Feature: Analyze CLI

  Scenario: Analyze a simple module (text)
    Given the analyze fixtures
    When I run analyze in text format
    Then the analyze report includes summary

  Scenario: Analyze with top N hotspots
    Given the analyze fixtures
    When I run analyze with top 5
    Then the analyze report includes hotspots

  Scenario: Analyze JSON output
    Given the analyze fixtures
    When I run analyze in json format
    Then the analyze output is json
