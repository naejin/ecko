// Expected: exit 0
// Em dashes in strings and comments are fine.
// This handles special text — with em dashes
const message = "Hello — world";
const fancy = `Use \u201csmart quotes\u201d here`;
export { message, fancy };
