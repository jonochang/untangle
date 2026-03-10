Feature: Analyze CLI

  Scenario: Analyze report for a simple module (text)
    Given the analyze fixtures
    When I run analyze report in text format
    Then the analyze report includes summary

  Scenario: Analyze report with top N hotspots
    Given the analyze fixtures
    When I run analyze report with top 5
    Then the analyze report includes hotspots

  Scenario: Analyze report JSON output
    Given the analyze fixtures
    When I run analyze report in json format
    Then the analyze output is json
