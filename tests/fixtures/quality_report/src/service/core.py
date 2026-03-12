from src.api import validators
from src.db import connection


def process(score):
    if validators.valid(score) and connection.available():
        return score * 2
    return score
