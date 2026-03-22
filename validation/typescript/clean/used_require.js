// Expected: exit 0
const fs = require("fs");
const data = fs.readFileSync("file.txt");
module.exports = data;
