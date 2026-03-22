# Expected: exit 1, check=fixed-wait
import time
import asyncio

def test_with_sleep():
    time.sleep(2)
    assert True

async def test_with_async_sleep():
    await asyncio.sleep(1)
    assert True
