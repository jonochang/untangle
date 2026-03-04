Feature: Quality CLI

  Scenario: CRAP report includes summary
    Given the quality fixtures
    When I run the crap quality report
    Then the output includes the crap report table

  Scenario: Overall report includes hotspots
    Given the quality fixtures
    When I run the overall quality report
    Then the output includes the untangle hotspots section

  Scenario: Minimum CC filter
    Given the quality fixtures
    When I run the quality report with min cc 2
    Then the output excludes low cc functions
