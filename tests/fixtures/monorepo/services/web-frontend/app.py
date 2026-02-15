import requests
from services.web_frontend.graphql_client import get_user

# Cross-service REST call to post-api
response = requests.get("/api/v1/posts")

# Cross-service GraphQL call to user-api
user = get_user("123")
