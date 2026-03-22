// Expected: exit 0
// Catch that logs before rethrowing is NOT useless.
async function fetchData(url: string) {
    try {
        const response = await fetch(url);
        return response.json();
    } catch (e) {
        console.error('Fetch failed:', e);
        throw e;
    }
}

export { fetchData };
