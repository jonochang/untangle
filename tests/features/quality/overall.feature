Feature: Overall quality report

  Scenario: Untangle hotspots are visible
    Given the quality fixtures
    When I run the overall quality report
    Then the output includes the untangle hotspots section
