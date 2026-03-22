// Expected: exit 1, check=useless-catch
function load() {
    try {
        return fetchData();
    } catch (e) {
        throw e;
    }
}
