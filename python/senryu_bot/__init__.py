"""senryu-bot: Rust-powered Discord senryu detection bot."""

from ._native import run as _run

__all__ = ["run"]
__version__ = "0.1.1"


def run(token: str) -> None:
    """Start the Discord bot.

    `run()` blocks until the bot shuts down.
    """
    if not isinstance(token, str):
        raise TypeError("token must be str")
    token = token.strip()
    if not token:
        raise ValueError("DISCORD_BOT_TOKEN is empty")
    _run(token)
