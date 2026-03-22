// Expected: exit 0
const { readFileSync, writeFileSync } = require("fs");
const data = readFileSync("in.txt");
writeFileSync("out.txt", data);
