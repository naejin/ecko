// Expected: exit 1, check=debugger-statement
function processData(data: string[]) {
    debugger;
    return data.map(d => d.trim());
}
