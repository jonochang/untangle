from src.service import core
from src.db import connection


def handle(payload):
    score = 0
    if payload.get("skip_cache"):
        score += 1
    if payload.get("validate"):
        score += 1
    if payload.get("persist"):
        score += 1
    if payload.get("notify"):
        score += 1
    if connection.available():
        score += 1
    return core.process(score)
