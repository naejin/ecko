// Expected: exit 0
// Known limitation: placeholder-code only matches `new Error(...)`, not subclasses.
function notReady() {
    throw new TypeError("not implemented yet");
}

export { notReady };
