from pathlib import Path

FIXTURES_DIR: Path = Path(__file__).resolve().parent

with open(FIXTURES_DIR / "crontab.txt", "r") as f:
    CRONTAB: str = f.read()
