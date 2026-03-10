Feature: Service Graph CLI

  Scenario: Service graph JSON output
    Given the service graph fixtures
    When I run service-graph in json format
    Then the service-graph output is json
