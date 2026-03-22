// Expected: exit 1, check=unreachable-code
function process(x: number): number {
    return x * 2;
    console.log("dead");
}

function throwFirst(): never {
    throw new Error("fail");
    console.log("also dead");
}
