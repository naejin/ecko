// Expected: exit 1, check=unreachable-code
// Dead code inside arrow function with block body.
const process = (x: number) => {
    return x * 2;
    console.log("dead");
};

export { process };
