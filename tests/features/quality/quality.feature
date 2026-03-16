Feature: Quality CLI

  Scenario: Functions report includes summary
    Given the quality fixtures
    When I run the functions quality report
    Then the output includes the crap report table

  Scenario: Project report includes hotspots
    Given the quality fixtures
    When I run the project quality report
    Then the output includes the untangle hotspots section

  Scenario: Unified quality report includes all analysis sections
    Given the quality fixtures
    When I run the unified quality report in json format
    Then the unified quality report output is json
    And the unified quality report includes structural analysis
    And the unified quality report includes function quality
    And the unified quality report includes architecture analysis
    And the unified quality report includes priority actions
    And the unified quality report includes an architecture diagram

  Scenario: Unified quality report text output includes evidence for priority actions
    Given the quality fixtures
    When I run the unified quality report in text format
    Then the unified quality report includes priority evidence
    And the unified quality report includes priority locations
    And the unified quality report includes priority categories
    And the unified quality report includes the architecture summary

  Scenario: Unified quality report without coverage shows N/A coverage
    Given the quality fixtures
    When I run the unified quality report without coverage in text format
    Then the unified quality report shows N/A coverage in text

  Scenario: Unified quality report json uses null coverage and new schema version
    Given the quality fixtures
    When I run the unified quality report without coverage in json format
    Then the unified quality report json uses null coverage
    And the unified quality report json uses schema version 4

  Scenario: Minimum CC filter
    Given the quality fixtures
    When I run the functions quality report with min cc 2
    Then the output excludes low cc functions
