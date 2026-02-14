from src.db import connection
from src.utils import logging

def handle():
    connection.query()
    logging.info("handled")
