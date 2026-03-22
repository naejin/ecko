// Expected: exit 1, check=no-var
var x = 1;
var name = "hello";
function test() {
    var inner = true;
    return inner;
}
