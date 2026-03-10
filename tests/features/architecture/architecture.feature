Feature: Analyze Architecture CLI

  Scenario: Analyze architecture JSON output
    Given the analyze fixtures
    When I run analyze architecture in json format
    Then the architecture output is json
