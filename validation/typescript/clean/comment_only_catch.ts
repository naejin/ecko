// Expected: exit 0
// Catch with only a comment is intentional, not empty.
function tryParse(json: string): unknown {
    try {
        return JSON.parse(json);
    } catch (e) {
        /* intentionally ignored */
    }
    return null;
}

export { tryParse };
