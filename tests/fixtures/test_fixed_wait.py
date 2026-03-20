"""Fixture: test with sleep/wait_for_timeout (should trigger fixed-wait)."""
import asyncio
import time


def test_with_sleep():
    time.sleep(2)
    assert True


async def test_with_async_sleep():
    await asyncio.sleep(1)
    assert True


def test_with_wait_for_timeout():
    page = get_page()
    page.wait_for_timeout(5000)
    assert page.title() == "Home"


def test_clean():
    """No waits here — should not trigger."""
    assert 1 + 1 == 2
