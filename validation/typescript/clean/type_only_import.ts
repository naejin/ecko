// Expected: exit 0
// import type should not be flagged as unused.
import type { Config } from './types';

function validate(config: Config): boolean {
    return config !== null;
}

export { validate };
