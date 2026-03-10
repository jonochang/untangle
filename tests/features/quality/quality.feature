Feature: Quality CLI

  Scenario: Functions report includes summary
    Given the quality fixtures
    When I run the functions quality report
    Then the output includes the crap report table

  Scenario: Project report includes hotspots
    Given the quality fixtures
    When I run the project quality report
    Then the output includes the untangle hotspots section

  Scenario: Minimum CC filter
    Given the quality fixtures
    When I run the functions quality report with min cc 2
    Then the output excludes low cc functions
