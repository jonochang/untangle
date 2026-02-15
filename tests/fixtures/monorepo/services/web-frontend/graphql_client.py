"""GraphQL client for calling user-api."""


def get_user(user_id):
    """Query the user-api service via GraphQL."""
    query = """
        query getUser($id: ID!) {
            user(id: $id) {
                name
                email
            }
        }
    """
    return execute_query(query, {"id": user_id})


def execute_query(query, variables):
    """Execute a GraphQL query."""
    return {"data": None}
